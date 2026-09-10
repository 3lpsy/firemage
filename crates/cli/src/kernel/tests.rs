use clap::Parser;

use super::Command;

#[test]
fn alias_requires_a_value_or_explicit_clear() {
    assert!(crate::args::Cli::try_parse_from(["firemage", "kernel", "alias", "vmlinux"]).is_err());
    assert!(
        crate::args::Cli::try_parse_from([
            "firemage", "kernel", "alias", "vmlinux", "stable", "--clear"
        ])
        .is_err()
    );
    let parsed =
        crate::args::Cli::try_parse_from(["firemage", "kernel", "alias", "vmlinux", "--clear"])
            .unwrap();
    assert!(matches!(
        parsed.command,
        crate::args::Command::Kernel(Command::Alias {
            alias: None,
            clear: true,
            ..
        })
    ));
}

#[test]
fn upload_rejects_empty_and_oversized_files_before_reading() {
    let path = std::env::temp_dir().join(format!("firemage-kernel-upload-{}", std::process::id()));
    let file = std::fs::File::create_new(&path).unwrap();
    assert!(crate::upload::read(&path, 1024, true).unwrap().is_empty());
    assert!(super::command::read_upload(&path).is_err());
    file.set_len(firemage_wire::KERNEL_MAX_BYTES + 1).unwrap();
    assert!(super::command::read_upload(&path).is_err());
    std::fs::write(&path, [0x7f, b'E', b'L', b'F']).unwrap();
    assert_eq!(
        super::command::read_upload(&path).unwrap(),
        [0x7f, b'E', b'L', b'F']
    );
    std::fs::remove_file(path).unwrap();
}
