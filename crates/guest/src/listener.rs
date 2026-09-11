use std::{
    fs::File,
    io, mem,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const MAX_SESSIONS: usize = 8;

pub fn serve(port: u32) -> io::Result<()> {
    // Custom init may ignore SIGCHLD; retain exited children until session cleanup.
    if unsafe { libc::signal(libc::SIGCHLD, libc::SIG_DFL) } == libc::SIG_ERR {
        return Err(io::Error::last_os_error());
    }
    let probe = unsafe { libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0) };
    if probe < 0 {
        return Err(io::Error::other(
            "guest kernel must support pidfd_open (Linux 5.3 or newer)",
        ));
    }
    drop(unsafe { OwnedFd::from_raw_fd(probe as i32) });
    // CLOEXEC prevents shells from inheriting the listening socket.
    let fd = unsafe { libc::socket(libc::AF_VSOCK, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let listener = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut address: libc::sockaddr_vm = unsafe { mem::zeroed() };
    address.svm_family = libc::AF_VSOCK as _;
    address.svm_cid = libc::VMADDR_CID_ANY;
    address.svm_port = port;
    if unsafe {
        libc::bind(
            listener.as_raw_fd(),
            (&address as *const libc::sockaddr_vm).cast(),
            mem::size_of_val(&address) as _,
        )
    } < 0
        || unsafe { libc::listen(listener.as_raw_fd(), MAX_SESSIONS as _) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    let sessions = Arc::new(AtomicUsize::new(0));
    loop {
        let mut peer: libc::sockaddr_vm = unsafe { mem::zeroed() };
        let mut length = mem::size_of_val(&peer) as libc::socklen_t;
        let fd = unsafe {
            libc::accept4(
                listener.as_raw_fd(),
                (&mut peer as *mut libc::sockaddr_vm).cast(),
                &mut length,
                libc::SOCK_CLOEXEC,
            )
        };
        if fd < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        let socket = unsafe { File::from_raw_fd(fd) };
        if !is_host_peer(peer.svm_family, peer.svm_cid)
            || sessions.load(Ordering::Acquire) >= MAX_SESSIONS
        {
            continue;
        }
        sessions.fetch_add(1, Ordering::AcqRel);
        let active = sessions.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("web-shell".into())
            .spawn(move || {
                let _guard = SessionCount(active);
                if let Err(error) = crate::session::serve(socket) {
                    eprintln!("web shell session: {error}");
                }
            })
        {
            sessions.fetch_sub(1, Ordering::AcqRel);
            eprintln!("web shell thread: {error}");
        }
    }
}

fn is_host_peer(family: libc::sa_family_t, cid: u32) -> bool {
    family == libc::AF_VSOCK as libc::sa_family_t && cid == libc::VMADDR_CID_HOST
}

struct SessionCount(Arc<AtomicUsize>);
impl Drop for SessionCount {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
#[path = "listener_tests.rs"]
mod tests;
