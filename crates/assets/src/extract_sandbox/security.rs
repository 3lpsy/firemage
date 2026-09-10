use std::{
    fs::File,
    io::{Seek, Write},
    os::fd::FromRawFd,
};

pub(super) fn filter() -> anyhow::Result<File> {
    #[cfg(target_arch = "x86_64")]
    let architecture = 0xc000003e;
    #[cfg(target_arch = "aarch64")]
    let architecture = 0xc00000b7;
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    let architecture = 0;
    anyhow::ensure!(
        architecture != 0,
        "isolated extraction supports x86_64 and aarch64 hosts"
    );
    let mut rules = vec![
        libc::sock_filter {
            code: 0x20,
            jt: 0,
            jf: 0,
            k: 4,
        },
        libc::sock_filter {
            code: 0x15,
            jt: 1,
            jf: 0,
            k: architecture,
        },
        libc::sock_filter {
            code: 0x06,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_KILL_PROCESS,
        },
        libc::sock_filter {
            code: 0x20,
            jt: 0,
            jf: 0,
            k: 0,
        },
        libc::sock_filter {
            code: 0x35,
            jt: 0,
            jf: 1,
            k: 0x40000000,
        },
        libc::sock_filter {
            code: 0x06,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_KILL_PROCESS,
        },
    ];
    for syscall in blocked_syscalls() {
        rules.push(libc::sock_filter {
            code: 0x15,
            jt: 0,
            jf: 1,
            k: syscall,
        });
        rules.push(libc::sock_filter {
            code: 0x06,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_ERRNO | libc::EPERM as u32,
        });
    }
    rules.push(libc::sock_filter {
        code: 0x06,
        jt: 0,
        jf: 0,
        k: libc::SECCOMP_RET_ALLOW,
    });
    let fd = unsafe { libc::memfd_create(c"firemage-extract-seccomp".as_ptr(), libc::MFD_CLOEXEC) };
    anyhow::ensure!(
        fd >= 0,
        "cannot create extraction seccomp filter: {}",
        std::io::Error::last_os_error()
    );
    let mut file = unsafe { File::from_raw_fd(fd) };
    let bytes = unsafe {
        std::slice::from_raw_parts(
            rules.as_ptr().cast::<u8>(),
            std::mem::size_of_val(rules.as_slice()),
        )
    };
    file.write_all(bytes)?;
    file.rewind()?;
    Ok(file)
}

fn blocked_syscalls() -> Vec<u32> {
    let mut calls = vec![libc::SYS_clone as u32, libc::SYS_clone3 as u32];
    #[cfg(target_arch = "x86_64")]
    calls.extend([libc::SYS_fork as u32, libc::SYS_vfork as u32]);
    calls
}

// Limits apply before bubblewrap starts; its filter then forbids spawning parser descendants.
pub(super) fn limits(filter: i32) -> std::io::Result<()> {
    for (resource, value) in [
        (libc::RLIMIT_CPU, 15),
        (libc::RLIMIT_AS, 512 * 1024 * 1024),
        (libc::RLIMIT_NOFILE, 64),
        (libc::RLIMIT_FSIZE, 0),
        (libc::RLIMIT_CORE, 0),
    ] {
        let limit = libc::rlimit {
            rlim_cur: value,
            rlim_max: value,
        };
        if unsafe { libc::setrlimit(resource, &limit) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::fcntl(filter, libc::F_SETFD, 0) } == -1
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
