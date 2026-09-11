use super::*;
use firemage_wire::SnapshotManifest;
use serde_json::json;

fn fixture(dir: &std::path::Path) -> SnapshotManifest {
    std::fs::write(dir.join("state.bin"), b"state").unwrap();
    std::fs::write(dir.join("kernel"), b"kernel").unwrap();
    std::fs::write(dir.join("rootfs.ext4"), b"disk").unwrap();
    create_private(&dir.join("memory.bin"))
        .unwrap()
        .set_len(64 * 1024 * 1024)
        .unwrap();
    serde_json::from_value(json!({"version":1,"source_vm_name":"review","architecture":"x86_64","firecracker_version":"1.16.1","spec":{"name":"review","memory_mib":64},"network":null,"gateway_mac":null,"files":{
        "state.bin":{"size_bytes":0,"sha256":""},"memory.bin":{"size_bytes":0,"sha256":""},"kernel":{"size_bytes":0,"sha256":""},"rootfs.ext4":{"size_bytes":0,"sha256":""}
    }})).unwrap()
}
#[test]
fn bundles_preserve_files_and_reject_tampering_and_expansion_over_limit() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::create_dir(&source).unwrap();
    let manifest = fixture(&source);
    let archive = dir.path().join("snapshot.fmsnap");
    let limit = 128 * 1024 * 1024;
    let packed = pack(&source, manifest, &archive, limit).unwrap();
    assert_eq!(
        inspect(&archive, limit).unwrap().files["rootfs.ext4"].sha256,
        packed.files["rootfs.ext4"].sha256
    );
    assert!(inspect(&archive, 32 * 1024 * 1024).is_err());
    let extracted = dir.path().join("extracted");
    std::fs::create_dir(&extracted).unwrap();
    unpack(&archive, &extracted, limit).unwrap();
    assert_eq!(
        std::fs::read(extracted.join("rootfs.ext4")).unwrap(),
        b"disk"
    );
    assert_eq!(
        std::fs::metadata(extracted.join("memory.bin"))
            .unwrap()
            .len(),
        64 * 1024 * 1024
    );
    assert!(unpack(&archive, &extracted, limit).is_err());
    std::fs::write(&archive, b"invalid archive").unwrap();
    assert!(inspect(&archive, limit).is_err());
}
#[test]
fn manifests_disallow_host_paths_missing_files_and_unjailed_state() {
    let dir = tempfile::tempdir().unwrap();
    let m = fixture(dir.path());
    let archive = dir.path().join("snapshot.fmsnap");
    let mut m = pack(dir.path(), m, &archive, 128 * 1024 * 1024).unwrap();
    m.files
        .insert("../escape".into(), m.files["kernel"].clone());
    assert!(validate_manifest(&m, u64::MAX).is_err());
    m.files.remove("../escape");
    m.files.remove("rootfs.ext4");
    assert!(validate_manifest(&m, u64::MAX).is_err());
}

#[test]
fn archives_reject_links_duplicate_members_extensions_and_wrong_hashes() {
    let directory = tempfile::tempdir().unwrap();
    let initial = fixture(directory.path());
    let manifest = pack(
        directory.path(),
        initial,
        &directory.path().join("good.fmsnap"),
        128 * 1024 * 1024,
    )
    .unwrap();
    for failure in [
        "symlink",
        "hardlink",
        "duplicate",
        "extension",
        "hash",
        "extra",
    ] {
        let path = directory.path().join(format!("{failure}.fmsnap"));
        let writer = flate2::write::GzEncoder::new(
            create_private(&path).unwrap(),
            flate2::Compression::fast(),
        );
        let mut archive = tar::Builder::new(writer);
        let encoded = serde_json::to_vec(&manifest).unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o600);
        header.set_size(encoded.len() as u64);
        archive
            .append_data(&mut header, "manifest.json", encoded.as_slice())
            .unwrap();
        let mut entry = tar::Header::new_gnu();
        entry.set_mode(0o600);
        let mut name = "state.bin";
        let content = if failure == "hash" {
            b"wrong".as_slice()
        } else {
            b"state".as_slice()
        };
        entry.set_entry_type(match failure {
            "symlink" => tar::EntryType::Symlink,
            "hardlink" => tar::EntryType::Link,
            "extension" => tar::EntryType::GNULongName,
            _ => tar::EntryType::Regular,
        });
        if matches!(failure, "symlink" | "hardlink") {
            entry.set_size(0);
            entry.set_link_name("/etc/passwd").unwrap();
            archive
                .append_data(&mut entry, name, std::io::empty())
                .unwrap();
        } else {
            if failure == "extra" {
                name = "unlisted";
            }
            entry.set_size(content.len() as u64);
            archive.append_data(&mut entry, name, content).unwrap();
            if failure == "duplicate" {
                archive.append_data(&mut entry, name, content).unwrap();
            }
        }
        archive.into_inner().unwrap().finish().unwrap();
        let error = inspect(&path, 128 * 1024 * 1024).unwrap_err().to_string();
        assert!(
            !error.contains("missing files"),
            "{failure} should fail on the invalid member: {error}"
        );
        let destination = tempfile::tempdir().unwrap();
        assert!(unpack(&path, destination.path(), 128 * 1024 * 1024).is_err());
        assert!(!destination.path().join("kernel").is_symlink());
    }
}
