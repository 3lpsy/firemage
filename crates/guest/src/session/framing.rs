use firemage_guest_protocol::{ClientMessage, MAX_FRAME_BYTES, ServerMessage, write_frame};
use std::{
    fs::File,
    io::{self, Read, Write},
    os::fd::AsRawFd,
};

pub const MAX_PENDING: usize = 262_144;

pub fn nonblocking(file: &File) -> io::Result<()> {
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn enqueue(pending: &mut Vec<u8>, message: &ServerMessage) -> io::Result<()> {
    if pending.len() > MAX_PENDING {
        return Err(io::Error::other("terminal client is too slow"));
    }
    write_frame(pending, message)
}

pub fn flush(file: &mut File, pending: &mut Vec<u8>) -> io::Result<()> {
    if pending.is_empty() {
        return Ok(());
    }
    match file.write(pending) {
        Ok(0) => Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "terminal disconnected",
        )),
        Ok(count) => {
            pending.drain(..count);
            Ok(())
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub fn receive(file: &mut File, pending: &mut Vec<u8>) -> io::Result<bool> {
    let mut data = [0; 8192];
    match file.read(&mut data) {
        Ok(0) => Ok(false),
        Ok(count) => {
            pending.extend_from_slice(&data[..count]);
            if pending.len() > MAX_FRAME_BYTES + 4 + data.len() {
                return Err(io::Error::other("guest frame too large"));
            }
            Ok(true)
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

pub fn next_message(pending: &mut Vec<u8>) -> io::Result<Option<ClientMessage>> {
    if pending.len() < 4 {
        return Ok(None);
    }
    let size = u32::from_be_bytes(pending[..4].try_into().unwrap()) as usize;
    if size == 0 || size > MAX_FRAME_BYTES {
        return Err(io::Error::other("invalid guest frame length"));
    }
    if pending.len() < size + 4 {
        return Ok(None);
    }
    let message = serde_json::from_slice(&pending[4..size + 4]).map_err(io::Error::other)?;
    pending.drain(..size + 4);
    Ok(Some(message))
}
