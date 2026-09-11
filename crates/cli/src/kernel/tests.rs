use clap::Parser;

use super::Command;

#[test]
fn aliases_are_required_for_catalog_mutations() {
    for arguments in [
        vec!["firemage", "kernel", "alias", "vmlinux"],
        vec!["firemage", "kernel", "alias", "vmlinux", "--clear"],
        vec!["firemage", "kernel", "upload", "vmlinux"],
        vec![
            "firemage",
            "kernel",
            "download",
            "vmlinux",
            "https://example.com/kernel",
            "--sha256",
            "hash",
        ],
    ] {
        assert!(crate::args::Cli::try_parse_from(arguments).is_err());
    }
    assert!(
        crate::args::Cli::try_parse_from([
            "firemage",
            "kernel",
            "download",
            "vmlinux",
            "https://example.com/kernel",
            "--alias",
            "stable"
        ])
        .is_ok()
    );
    let parsed =
        crate::args::Cli::try_parse_from(["firemage", "kernel", "alias", "vmlinux", "stable"])
            .unwrap();
    assert!(
        matches!(parsed.command, crate::args::Command::Kernel(Command::Alias { alias, .. }) if alias == "stable")
    );
    assert!(
        crate::args::Cli::try_parse_from([
            "firemage", "kernel", "upload", "vmlinux", "--alias", "stable"
        ])
        .is_ok()
    );
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
