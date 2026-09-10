use super::command::build;
use std::path::{Path, PathBuf};

#[test]
fn parser_gets_only_readonly_files_and_required_isolation() {
    let binary = Path::new("/usr/sbin/debugfs");
    let disk = Path::new("/private/vm/rootfs.ext4");
    let libraries = [
        PathBuf::from("/lib64/ld-linux-x86-64.so.2"),
        PathBuf::from("/lib64/libc.so.6"),
    ];
    let command = build(Path::new("/usr/bin/bwrap"), binary, &libraries, disk, 7);
    let command = command.as_std();
    let args: Vec<_> = command
        .get_args()
        .map(|value| value.to_str().unwrap())
        .collect();
    for required in [
        "--unshare-user",
        "--unshare-pid",
        "--unshare-net",
        "--unshare-ipc",
        "--unshare-uts",
        "--unshare-cgroup",
        "--disable-userns",
        "--die-with-parent",
        "--clearenv",
    ] {
        assert!(args.contains(&required));
    }
    assert!(!args.iter().any(|value| value.ends_with("-try")));
    for pair in [
        ["--uid", "65534"],
        ["--gid", "65534"],
        ["--cap-drop", "ALL"],
        ["--remount-ro", "/"],
        ["--seccomp", "7"],
    ] {
        assert!(args.windows(2).any(|window| window == pair));
    }
    let binds: Vec<_> = args
        .windows(3)
        .filter(|window| window[0] == "--ro-bind")
        .map(|window| (window[1], window[2]))
        .collect();
    assert_eq!(
        binds,
        vec![
            ("/usr/sbin/debugfs", "/debugfs"),
            ("/lib64/ld-linux-x86-64.so.2", "/lib64/ld-linux-x86-64.so.2"),
            ("/lib64/libc.so.6", "/lib64/libc.so.6"),
            ("/private/vm/rootfs.ext4", "/input/rootfs.ext4")
        ]
    );
    assert!(!args.contains(&"--bind") && !args.contains(&"--dev") && !args.contains(&"--proc"));
    assert_eq!(command.get_envs().count(), 0);
}

#[test]
fn process_filter_rejects_compatibility_syscalls_and_process_creation() {
    use std::io::{Read, Seek};
    let mut file = super::security::filter().unwrap();
    file.rewind().unwrap();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    let instructions: Vec<_> = bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| {
            (
                u16::from_ne_bytes(chunk[0..2].try_into().unwrap()),
                chunk[2],
                chunk[3],
                u32::from_ne_bytes(chunk[4..8].try_into().unwrap()),
            )
        })
        .collect();
    let evaluate = |architecture: u32, syscall: u32| {
        let mut index = 0;
        let mut value = 0;
        loop {
            let (code, yes, no, operand) = instructions[index];
            match code {
                0x20 => value = if operand == 4 { architecture } else { syscall },
                0x15 => index += if value == operand { yes } else { no } as usize,
                0x35 => index += if value >= operand { yes } else { no } as usize,
                0x06 => break operand,
                _ => panic!("unexpected BPF instruction"),
            }
            index += 1;
        }
    };
    #[cfg(target_arch = "x86_64")]
    let architecture = 0xc000003e;
    #[cfg(target_arch = "aarch64")]
    let architecture = 0xc00000b7;
    assert_eq!(
        evaluate(architecture, libc::SYS_read as u32),
        libc::SECCOMP_RET_ALLOW
    );
    for syscall in [libc::SYS_clone, libc::SYS_clone3] {
        assert_eq!(
            evaluate(architecture, syscall as u32),
            libc::SECCOMP_RET_ERRNO | libc::EPERM as u32
        );
    }
    assert_eq!(
        evaluate(0, libc::SYS_read as u32),
        libc::SECCOMP_RET_KILL_PROCESS
    );
    assert_eq!(
        evaluate(architecture, 0x40000000),
        libc::SECCOMP_RET_KILL_PROCESS
    );
}
