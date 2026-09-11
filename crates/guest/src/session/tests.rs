use super::serve;
use firemage_guest_protocol::{
    ClientMessage, ServerMessage, VERSION, decode_data, encode_data, read_frame, write_frame,
};
use std::{fs::File, io, os::fd::OwnedFd, os::unix::net::UnixStream, thread, time::Duration};

fn connection(command: &[&str]) -> (UnixStream, thread::JoinHandle<io::Result<()>>) {
    let (mut host, guest) = UnixStream::pair().unwrap();
    host.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let guest = File::from(OwnedFd::from(guest));
    let session = thread::spawn(move || serve(guest));
    write_frame(
        &mut host,
        &ClientMessage::Open {
            version: VERSION,
            command: command.iter().map(|value| (*value).into()).collect(),
            rows: 24,
            cols: 80,
        },
    )
    .unwrap();
    (host, session)
}

#[test]
fn pty_captures_stdout_stderr_and_exit_code() {
    let (mut host, session) = connection(&[
        "/bin/sh",
        "-c",
        "test -t 0 && test -t 1 && test -t 2; printf 'stdout'; printf 'stderr' >&2; exit 7",
    ]);
    assert_eq!(
        read_frame::<_, ServerMessage>(&mut host).unwrap(),
        ServerMessage::Ready
    );
    let mut output = Vec::new();
    loop {
        match read_frame::<_, ServerMessage>(&mut host).unwrap() {
            ServerMessage::Output { data } => output.extend(decode_data(&data).unwrap()),
            ServerMessage::Exit { code } => {
                assert_eq!(code, Some(7));
                break;
            }
            message => panic!("unexpected message: {message:?}"),
        }
    }
    assert!(String::from_utf8_lossy(&output).contains("stdoutstderr"));
    session.join().unwrap().unwrap();
}

#[test]
fn input_and_resize_reach_controlling_terminal() {
    let (mut host, session) = connection(&[
        "/bin/sh",
        "-c",
        "read answer; stty size; printf 'answer=%s' \"$answer\"",
    ]);
    assert_eq!(
        read_frame::<_, ServerMessage>(&mut host).unwrap(),
        ServerMessage::Ready
    );
    write_frame(&mut host, &ClientMessage::Resize { rows: 37, cols: 91 }).unwrap();
    write_frame(
        &mut host,
        &ClientMessage::Input {
            data: encode_data(b"hello\n"),
        },
    )
    .unwrap();
    let mut output = Vec::new();
    loop {
        match read_frame::<_, ServerMessage>(&mut host).unwrap() {
            ServerMessage::Output { data } => output.extend(decode_data(&data).unwrap()),
            ServerMessage::Exit { code } => {
                assert_eq!(code, Some(0));
                break;
            }
            message => panic!("unexpected message: {message:?}"),
        }
    }
    let output = String::from_utf8_lossy(&output);
    assert!(output.contains("37 91"), "{output}");
    assert!(output.contains("answer=hello"), "{output}");
    session.join().unwrap().unwrap();
}

#[test]
fn disconnect_kills_shell_and_its_foreground_job() {
    let (mut host, session) = connection(&[
        "/bin/sh",
        "-m",
        "-c",
        "sleep 60 & job=$!; printf '%s %s\\n' $$ \"$job\"; wait",
    ]);
    assert_eq!(
        read_frame::<_, ServerMessage>(&mut host).unwrap(),
        ServerMessage::Ready
    );
    let mut output = String::new();
    while !output.contains('\n') {
        let ServerMessage::Output { data } = read_frame::<_, ServerMessage>(&mut host).unwrap()
        else {
            panic!("expected shell and job PIDs");
        };
        output.push_str(&String::from_utf8(decode_data(&data).unwrap()).unwrap());
    }
    let pids: Vec<i32> = output
        .split_whitespace()
        .map(|pid| pid.parse().unwrap())
        .collect();
    let [pid, job] = pids[..] else {
        panic!("expected two PIDs: {output}");
    };
    assert_eq!(unsafe { libc::getsid(job) }, pid);
    assert_ne!(
        unsafe { libc::getpgid(job) },
        pid,
        "job must exercise session-member cleanup"
    );
    drop(host);
    session.join().unwrap().unwrap();
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while unsafe { libc::getsid(job) } == pid {
        let stat = std::fs::read_to_string(format!("/proc/{job}/stat")).unwrap_or_default();
        if stat
            .rsplit_once(')')
            .is_none_or(|(_, fields)| fields.split_whitespace().next() == Some("Z"))
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "shell job survived disconnect"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn malformed_open_cannot_start_a_session() {
    let (mut host, guest) = UnixStream::pair().unwrap();
    host.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let session = thread::spawn(move || serve(File::from(OwnedFd::from(guest))));
    write_frame(
        &mut host,
        &ClientMessage::Open {
            version: VERSION + 1,
            command: vec!["/bin/sh".into()],
            rows: 24,
            cols: 80,
        },
    )
    .unwrap();
    assert!(matches!(
        read_frame::<_, ServerMessage>(&mut host).unwrap(),
        ServerMessage::Error { .. }
    ));
    assert!(session.join().unwrap().is_err());
}

#[test]
fn incremental_frames_reject_invalid_length_before_allocation() {
    let mut bytes = Vec::new();
    write_frame(
        &mut bytes,
        &ClientMessage::Resize {
            rows: 30,
            cols: 100,
        },
    )
    .unwrap();
    let mut received = bytes[..2].to_vec();
    assert!(
        super::framing::next_message(&mut received)
            .unwrap()
            .is_none()
    );
    received.extend_from_slice(&bytes[2..bytes.len() - 1]);
    assert!(
        super::framing::next_message(&mut received)
            .unwrap()
            .is_none()
    );
    received.push(*bytes.last().unwrap());
    assert_eq!(
        super::framing::next_message(&mut received).unwrap(),
        Some(ClientMessage::Resize {
            rows: 30,
            cols: 100
        })
    );
    assert!(received.is_empty());
    received.extend_from_slice(&u32::MAX.to_be_bytes());
    assert!(super::framing::next_message(&mut received).is_err());
}

#[test]
fn control_c_interrupts_foreground_process() {
    let (mut host, session) = connection(&[
        "/bin/sh",
        "-c",
        "trap 'printf interrupted; exit 0' INT; /bin/sh -c 'printf ready; exec sleep 60'",
    ]);
    assert_eq!(
        read_frame::<_, ServerMessage>(&mut host).unwrap(),
        ServerMessage::Ready
    );
    let mut output = String::new();
    while !output.contains("ready") {
        let ServerMessage::Output { data } = read_frame::<_, ServerMessage>(&mut host).unwrap()
        else {
            panic!("expected ready output");
        };
        output.push_str(&String::from_utf8_lossy(&decode_data(&data).unwrap()));
    }
    write_frame(
        &mut host,
        &ClientMessage::Input {
            data: encode_data(&[3]),
        },
    )
    .unwrap();
    loop {
        match read_frame::<_, ServerMessage>(&mut host)
            .unwrap_or_else(|error| panic!("{error}: output={output:?}"))
        {
            ServerMessage::Output { data } => {
                output.push_str(&String::from_utf8_lossy(&decode_data(&data).unwrap()))
            }
            ServerMessage::Exit { code } => {
                assert_eq!(code, Some(0));
                break;
            }
            message => panic!("unexpected message: {message:?}"),
        }
    }
    assert!(output.contains("interrupted"), "{output}");
    session.join().unwrap().unwrap();
}

#[test]
fn exited_shell_pid_remains_reserved_until_cleanup() {
    let shell =
        super::process::Shell::start(&["/bin/sh".into(), "-c".into(), "exit 17".into()], 24, 80)
            .unwrap();
    let pid = shell.child.id() as libc::pid_t;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(code) = shell.exit_code().unwrap() {
            assert_eq!(code, Some(17));
            break;
        }
        assert!(std::time::Instant::now() < deadline, "shell did not exit");
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        0,
        "shell PID was reaped before cleanup"
    );
    assert_eq!(shell.exit_code().unwrap(), Some(Some(17)));
    drop(shell);
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "shell was not reaped during cleanup"
    );
}
