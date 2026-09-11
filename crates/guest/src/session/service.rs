use super::{
    framing::{self, MAX_PENDING},
    process::Shell,
};
use firemage_guest_protocol::{
    ClientMessage, ServerMessage, decode_data, encode_data, validate_open, validate_size,
};
use std::{
    fs::File,
    io::{self, Read},
    os::fd::AsRawFd,
    time::{Duration, Instant},
};

pub fn serve(mut socket: File) -> io::Result<()> {
    framing::nonblocking(&socket)?;
    let result = run(&mut socket);
    if let Err(error) = &result {
        let mut pending = Vec::new();
        let _ = framing::enqueue(
            &mut pending,
            &ServerMessage::Error {
                message: error.to_string(),
            },
        );
        let _ = framing::flush(&mut socket, &mut pending);
    }
    result
}

fn run(socket: &mut File) -> io::Result<()> {
    let mut incoming = Vec::new();
    let mut outgoing = Vec::new();
    let mut input = Vec::new();
    let mut shell: Option<Shell> = None;
    let started = Instant::now();
    let mut progress = started;
    let mut exit: Option<(Option<i32>, Instant)> = None;
    let mut finishing = false;
    let mut pty_closed = false;
    loop {
        if shell.is_none() && started.elapsed() > Duration::from_secs(10) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "guest handshake timed out",
            ));
        }
        if !outgoing.is_empty() && progress.elapsed() > Duration::from_secs(10) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "terminal client is too slow",
            ));
        }
        let mut poll = [
            libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLIN
                    | if outgoing.is_empty() {
                        0
                    } else {
                        libc::POLLOUT
                    },
                revents: 0,
            },
            libc::pollfd {
                fd: shell
                    .as_ref()
                    .filter(|_| !pty_closed)
                    .map_or(-1, |value| value.master.as_raw_fd()),
                events: if finishing { 0 } else { libc::POLLIN }
                    | if input.is_empty() { 0 } else { libc::POLLOUT },
                revents: 0,
            },
        ];
        if unsafe { libc::poll(poll.as_mut_ptr(), poll.len() as _, 100) } < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if poll[0].revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
            return Ok(());
        }
        if poll[0].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            if !framing::receive(socket, &mut incoming)? {
                return Ok(());
            }
            while let Some(message) = framing::next_message(&mut incoming)? {
                match message {
                    ClientMessage::Open {
                        version,
                        command,
                        rows,
                        cols,
                    } if shell.is_none() => {
                        validate_open(version, &command, rows, cols)?;
                        let value = Shell::start(&command, rows, cols)?;
                        framing::nonblocking(&value.master)?;
                        shell = Some(value);
                        framing::enqueue(&mut outgoing, &ServerMessage::Ready)?;
                    }
                    ClientMessage::Input { data } if shell.is_some() && !finishing => {
                        input.extend(decode_data(&data)?);
                        if input.len() > MAX_PENDING {
                            return Err(io::Error::other("too much pending terminal input"));
                        }
                    }
                    ClientMessage::Resize { rows, cols } if shell.is_some() && !finishing => {
                        validate_size(rows, cols)?;
                        shell.as_ref().unwrap().resize(rows, cols)?;
                    }
                    ClientMessage::Close => return Ok(()),
                    _ => {
                        return Err(io::Error::other(
                            "expected a single Open followed by Input, Resize, or Close",
                        ));
                    }
                }
            }
        }
        if let Some(shell) = shell.as_mut() {
            if !finishing && poll[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                let mut buffer = [0; 8192];
                match shell.master.read(&mut buffer) {
                    Ok(0) => pty_closed = true,
                    Ok(count) => framing::enqueue(
                        &mut outgoing,
                        &ServerMessage::Output {
                            data: encode_data(&buffer[..count]),
                        },
                    )?,
                    Err(error) if error.raw_os_error() == Some(libc::EIO) => pty_closed = true,
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(error) => return Err(error),
                }
            }
            if poll[1].revents & libc::POLLOUT != 0 {
                framing::flush(&mut shell.master, &mut input)?;
            }
            if exit.is_none()
                && let Some(code) = shell.exit_code()?
            {
                exit = Some((code, Instant::now()));
            }
            if !finishing
                && exit
                    .is_some_and(|(_, at)| pty_closed || at.elapsed() >= Duration::from_millis(200))
            {
                framing::enqueue(
                    &mut outgoing,
                    &ServerMessage::Exit {
                        code: exit.unwrap().0,
                    },
                )?;
                finishing = true;
                input.clear();
            }
        }
        let previous = outgoing.len();
        framing::flush(socket, &mut outgoing)?;
        if outgoing.len() < previous || outgoing.is_empty() {
            progress = Instant::now();
        }
        if finishing && outgoing.is_empty() {
            return Ok(());
        }
    }
}
