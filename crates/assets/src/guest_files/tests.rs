use firemage_wire::GuestFileKind;

#[test]
fn directory_preserves_spaces_unicode_and_handles_control_names_without_commands() {
    let output = "/2/040755/0/0/.//\n/2/040755/0/0/..//\n/12/100640/1000/1001/report final.json/21/\n/13/040750/0/0/資料//\n/14/120777/0/0/link/10/\n/15/010600/0/0/fifo/0/\n/16/100600/-1/-1/a\nb/2/\n\n";
    let entries = super::parse::directory(output.as_bytes()).unwrap();
    assert_eq!(entries.len(), 5);
    assert_eq!(entries[0].name, "資料");
    assert_eq!(entries[0].kind, GuestFileKind::Directory);
    assert_eq!(entries[0].size_bytes, None);
    let report = entries.iter().find(|file| file.inode == 12).unwrap();
    assert_eq!(report.name, "report final.json");
    assert_eq!(report.mode, 0o640);
    assert_eq!(report.uid, 1000);
    assert_eq!(report.gid, 1001);
    assert_eq!(report.size_bytes, Some(21));
    assert_eq!(
        entries.iter().find(|f| f.inode == 14).unwrap().kind,
        GuestFileKind::Symlink
    );
    assert_eq!(
        entries.iter().find(|f| f.inode == 15).unwrap().kind,
        GuestFileKind::Special
    );
    let control = entries.iter().find(|f| f.inode == 16).unwrap();
    assert_eq!(control.name, "a\\nb");
    assert_eq!(control.uid, u32::MAX);
    for bad in [
        "junk",
        "/12/100600/0/0/a/1",
        "/12/100600/0/0/a/1/extra\n",
        "/x/100600/0/0/a/1/\n",
    ] {
        assert!(super::parse::directory(bad.as_bytes()).is_err(), "{bad}");
    }
    assert!(super::parse::directory("/12/100600/0/0/a/1/\n".repeat(4097).as_bytes()).is_err());
}

#[test]
fn inode_type_rejects_symlinks_special_files_and_mismatched_inode() {
    let stat = b"Inode: 12   Type: regular    Mode:  0644   Flags: 0x80000\nGeneration: 0\n";
    super::parse::ensure_type(stat, 12, "regular").unwrap();
    assert!(super::parse::ensure_type(stat, 13, "regular").is_err());
    assert!(super::parse::ensure_type(stat, 12, "directory").is_err());
    for kind in ["symlink", "character special", "FIFO", "socket", "bad type"] {
        assert!(
            super::parse::ensure_type(
                format!("Inode: 12 Type: {kind} Mode: 0777").as_bytes(),
                12,
                "regular"
            )
            .is_err()
        );
    }
    assert!(firemage_wire::ensure_guest_inode(0).is_err());
    assert!(firemage_wire::ensure_guest_inode(1).is_err());
}

#[tokio::test]
async fn reader_preserves_binary_and_rejects_overflow_and_parser_diagnostics() {
    let mut command = tokio::process::Command::new("/bin/sh");
    command.args([
        "-c",
        "printf '\\000\\377hello'; printf 'debugfs 1.47.0\\n' >&2",
    ]);
    let mut output = Vec::new();
    assert_eq!(
        super::process::run(&mut command, &mut output, 7)
            .await
            .unwrap(),
        7
    );
    assert_eq!(output, b"\0\xffhello");
    for (script, limit) in [
        ("printf 12345", 4),
        ("printf 'File not found\\n' >&2", 100),
        ("exit 1", 100),
    ] {
        let mut command = tokio::process::Command::new("/bin/sh");
        command.args(["-c", script]);
        assert!(
            super::process::run(&mut command, &mut Vec::new(), limit)
                .await
                .is_err()
        );
    }
}
