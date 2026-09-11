use std::{
    fs::File,
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::process::CommandExt,
    },
    process::{Child, Command, Stdio},
    sync::Mutex,
};

static SPAWN_LOCK: Mutex<()> = Mutex::new(());

pub struct Shell {
    pub master: File,
    pub child: Child,
}

impl Shell {
    pub fn start(command: &[String], rows: u16, cols: u16) -> io::Result<Self> {
        // openpty cannot set CLOEXEC atomically; serialize allocation with every shell spawn.
        let _spawn = SPAWN_LOCK
            .lock()
            .map_err(|_| io::Error::other("shell spawn lock poisoned"))?;
        let mut master = -1;
        let mut slave = -1;
        let size = window_size(rows, cols);
        if unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        let master = unsafe { File::from_raw_fd(master) };
        let slave = unsafe { File::from_raw_fd(slave) };
        for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
            if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        let default = vec!["/bin/sh".to_owned(), "-i".to_owned()];
        let command = if command.is_empty() {
            &default
        } else {
            command
        };
        let mut process = Command::new(&command[0]);
        process
            .args(&command[1..])
            .env("TERM", "xterm-256color")
            .stdin(Stdio::from(slave.try_clone()?))
            .stdout(Stdio::from(slave.try_clone()?))
            .stderr(Stdio::from(slave));
        let parent = std::process::id() as libc::pid_t;
        // Only async-signal-safe calls may run between fork and exec.
        unsafe {
            process.pre_exec(move || {
                for signal in [
                    libc::SIGINT,
                    libc::SIGQUIT,
                    libc::SIGTERM,
                    libc::SIGHUP,
                    libc::SIGPIPE,
                    libc::SIGTSTP,
                    libc::SIGTTIN,
                    libc::SIGTTOU,
                ] {
                    libc::signal(signal, libc::SIG_DFL);
                }
                let mut mask = std::mem::zeroed();
                libc::sigemptyset(&mut mask);
                libc::sigprocmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut());
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) < 0
                    || libc::getppid() != parent
                    || libc::setsid() < 0
                    || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0
                {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = process.spawn()?;
        Ok(Self { master, child })
    }

    /// Keep the exited leader unreaped so its PID cannot be reused before cleanup.
    pub fn exit_code(&self) -> io::Result<Option<Option<i32>>> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        if unsafe {
            libc::waitid(
                libc::P_PID,
                self.child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } < 0
        {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                return Ok(None);
            }
            return Err(error);
        }
        if unsafe { info.si_pid() } == 0 {
            return Ok(None);
        }
        Ok(Some(
            (info.si_code == libc::CLD_EXITED).then(|| unsafe { info.si_status() }),
        ))
    }

    pub fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        if unsafe {
            libc::ioctl(
                self.master.as_raw_fd(),
                libc::TIOCSWINSZ as _,
                &window_size(rows, cols),
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        let pid = self.child.id() as i32;
        // Interactive jobs may have their own process group inside this session.
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let Some(candidate) = entry
                    .file_name()
                    .to_str()
                    .and_then(|name| name.parse::<i32>().ok())
                else {
                    continue;
                };
                if candidate > 0 {
                    kill_session_member(candidate, pid);
                }
            }
        }
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// Pin the task before checking its session so PID reuse cannot redirect the signal.
fn kill_session_member(candidate: libc::pid_t, session: libc::pid_t) {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, candidate, 0) };
    if fd < 0 {
        return;
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    if unsafe { libc::getsid(candidate) } == session {
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            );
        }
    }
}

fn window_size(rows: u16, cols: u16) -> libc::winsize {
    libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    }
}
