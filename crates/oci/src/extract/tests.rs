use super::{ExtractionBudget, apply_layer, finalize, has_file};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};

const TAR: &str = "application/vnd.oci.image.layer.v1.tar";

struct Item<'a> {
    path: &'a str,
    kind: u8,
    target: Option<&'a str>,
    data: &'a [u8],
    mode: u32,
}

fn file<'a>(path: &'a str, data: &'a [u8]) -> Item<'a> {
    Item {
        path,
        kind: b'0',
        target: None,
        data,
        mode: 0o644,
    }
}

fn link<'a>(path: &'a str, target: &'a str, kind: u8) -> Item<'a> {
    Item {
        path,
        target: Some(target),
        kind,
        data: b"",
        mode: 0o777,
    }
}

fn tar(items: &[Item<'_>]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for item in items {
        let mut header = tar::Header::new_gnu();
        header.set_uid(rustix::process::geteuid().as_raw().into());
        header.set_gid(rustix::process::getegid().as_raw().into());
        header.set_mode(item.mode);
        header.set_entry_type(tar::EntryType::new(item.kind));
        header.set_size(item.data.len() as u64);
        header.set_mtime(0);
        // Raw names exercise parser validation even when tar's builder rejects them.
        header.as_mut_bytes()[..item.path.len()].copy_from_slice(item.path.as_bytes());
        if let Some(target) = item.target {
            header.set_link_name(target).unwrap();
        }
        header.set_cksum();
        builder.append(&header, item.data).unwrap();
    }
    builder.into_inner().unwrap()
}

fn apply(root: &Path, bytes: &[u8], budget: &mut ExtractionBudget) -> anyhow::Result<()> {
    let mut layer = tempfile::NamedTempFile::new()?;
    layer.write_all(bytes)?;
    apply_layer(
        root,
        layer.path(),
        TAR,
        &format!("sha256:{:x}", Sha256::digest(bytes)),
        budget,
    )
}

fn budget() -> ExtractionBudget {
    ExtractionBudget::new(4 * 1024 * 1024, 1024)
}

#[test]
fn whiteouts_remove_only_lower_content_independent_of_entry_order() {
    let root = private_root();
    let mut budget = budget();
    apply(
        root.path(),
        &tar(&[
            file("d/old", b"old"),
            file("replace", b"old"),
            file("keep", b"keep"),
        ]),
        &mut budget,
    )
    .unwrap();
    apply(
        root.path(),
        &tar(&[
            file("d/new", b"new"),
            file("replace", b"new"),
            file("d/.wh..wh..opq", b""),
            file(".wh.replace", b""),
        ]),
        &mut budget,
    )
    .unwrap();
    assert!(!root.path().join("d/old").exists());
    assert_eq!(fs::read(root.path().join("d/new")).unwrap(), b"new");
    assert_eq!(fs::read(root.path().join("replace")).unwrap(), b"new");
    assert_eq!(fs::read(root.path().join("keep")).unwrap(), b"keep");
    assert!(!root.path().join("d/.wh..wh..opq").exists());
}

#[test]
fn merged_usr_and_absolute_guest_links_are_confined() {
    let root = private_root();
    let outside = private_root();
    fs::write(outside.path().join("sentinel"), b"host").unwrap();
    let target = outside.path().to_str().unwrap();
    let mut budget = budget();
    apply(
        root.path(),
        &tar(&[
            link("bin", "/usr/bin", b'2'),
            file("bin/sh", b"guest"),
            link("outside", target, b'2'),
            file("outside/sentinel", b"guest"),
        ]),
        &mut budget,
    )
    .unwrap();
    assert_eq!(fs::read(root.path().join("usr/bin/sh")).unwrap(), b"guest");
    assert!(has_file(root.path(), "/bin/sh").unwrap());
    assert_eq!(fs::read(outside.path().join("sentinel")).unwrap(), b"host");
    assert!(!has_file(root.path(), "/etc/passwd").unwrap());
}

#[test]
fn whiteouts_through_guest_links_cannot_delete_host_files() {
    let root = private_root();
    let outside = private_root();
    fs::write(outside.path().join("sentinel"), b"host").unwrap();
    let mut budget = budget();
    apply(
        root.path(),
        &tar(&[link("alias", outside.path().to_str().unwrap(), b'2')]),
        &mut budget,
    )
    .unwrap();
    apply(
        root.path(),
        &tar(&[
            file("alias/.wh.sentinel", b""),
            file("alias/.wh..wh..opq", b""),
        ]),
        &mut budget,
    )
    .unwrap();
    assert_eq!(fs::read(outside.path().join("sentinel")).unwrap(), b"host");
}

#[test]
fn traversal_special_entries_and_unsafe_links_are_rejected() {
    for item in [
        file("../escape", b"bad"),
        file("/absolute", b"bad"),
        link("escape", "../outside", b'2'),
        link("hard", "/etc/passwd", b'1'),
        file(".wh...", b""),
        Item {
            kind: b'6',
            ..file("fifo", b"")
        },
        Item {
            kind: b'3',
            ..file("device", b"")
        },
        Item {
            kind: b'S',
            ..file("sparse", b"")
        },
    ] {
        let root = private_root();
        assert!(apply(root.path(), &tar(&[item]), &mut budget()).is_err());
    }
    let root = private_root();
    assert!(
        apply(
            root.path(),
            &tar(&[link("symlink", "file", b'2'), link("hard", "symlink", b'1')]),
            &mut budget()
        )
        .is_err()
    );
}

#[test]
fn forward_hardlinks_preserve_inodes_and_replacement_preserves_siblings() {
    let root = private_root();
    let mut budget = budget();
    apply(
        root.path(),
        &tar(&[
            link("alias", "original", b'1'),
            Item {
                mode: 0o751,
                ..file("original", b"old")
            },
        ]),
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        fs::metadata(root.path().join("alias")).unwrap().ino(),
        fs::metadata(root.path().join("original")).unwrap().ino()
    );
    apply(root.path(), &tar(&[file("original", b"new")]), &mut budget).unwrap();
    assert_eq!(fs::read(root.path().join("alias")).unwrap(), b"old");
    finalize(root.path(), &budget).unwrap();
    assert_eq!(
        fs::metadata(root.path().join("alias")).unwrap().mode() & 0o7777,
        0o751
    );
    assert_eq!(
        fs::metadata(root.path().join("original")).unwrap().mode() & 0o7777,
        0o644
    );
}

#[test]
fn metadata_is_deferred_and_root_stays_private() {
    let root = private_root();
    let mut budget = budget();
    apply(
        root.path(),
        &tar(&[
            Item {
                kind: b'5',
                mode: 0o755,
                ..file(".", b"")
            },
            Item {
                mode: 0o4751,
                ..file("program", b"program")
            },
        ]),
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        fs::metadata(root.path()).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(root.path().join("program")).unwrap().mode() & 0o7777,
        0o600
    );
    finalize(root.path(), &budget).unwrap();
    let metadata = fs::metadata(root.path().join("program")).unwrap();
    assert_eq!(metadata.mode() & 0o7777, 0o4751);
    assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
}

#[test]
fn decompression_entries_and_extension_allocations_are_bounded() {
    let root = private_root();
    let bytes = tar(&[file("a", b"a"), file("b", b"b")]);
    assert!(
        apply(root.path(), &bytes, &mut ExtractionBudget::new(100, 100))
            .unwrap_err()
            .to_string()
            .contains("byte limit")
    );
    assert!(
        apply(root.path(), &bytes, &mut ExtractionBudget::new(100_000, 1))
            .unwrap_err()
            .to_string()
            .contains("entry limit")
    );
    let oversized = vec![b'a'; 65537];
    let bytes = tar(&[Item {
        kind: b'L',
        ..file("longname", &oversized)
    }]);
    assert!(
        apply(root.path(), &bytes, &mut budget())
            .unwrap_err()
            .to_string()
            .contains("64 KiB")
    );
}

#[test]
fn raw_gzip_and_zstd_use_the_same_verified_content() {
    let bytes = tar(&[file("content", b"verified")]);
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&bytes).unwrap();
    let compressed = [
        (TAR, bytes.clone()),
        (
            "application/vnd.oci.image.layer.v1.tar+gzip",
            gzip.finish().unwrap(),
        ),
        (
            "application/vnd.oci.image.layer.v1.tar+zstd",
            zstd::stream::encode_all(bytes.as_slice(), 1).unwrap(),
        ),
    ];
    for (media_type, content) in compressed {
        let root = private_root();
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&content).unwrap();
        let digest = format!("sha256:{:x}", Sha256::digest(&bytes));
        apply_layer(root.path(), file.path(), media_type, &digest, &mut budget()).unwrap();
        assert_eq!(fs::read(root.path().join("content")).unwrap(), b"verified");
        assert!(
            apply_layer(
                root.path(),
                file.path(),
                media_type,
                "sha256:incorrect",
                &mut budget()
            )
            .unwrap_err()
            .to_string()
            .contains("diff ID mismatch")
        );
    }
}

fn pax(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    builder
        .append_pax_extensions(entries.iter().copied())
        .unwrap();
    let content = tar(&[file("content", b"data")]);
    let mut entry_archive = tar::Archive::new(content.as_slice());
    let entry = entry_archive.entries().unwrap().next().unwrap().unwrap();
    builder.append(entry.header(), b"data".as_slice()).unwrap();
    builder.into_inner().unwrap()
}

#[test]
fn pax_mtime_and_user_xattrs_survive_finalization() {
    let root = private_root();
    let mut budget = budget();
    apply(
        root.path(),
        &pax(&[
            ("mtime", b"1234567890.123456789"),
            ("SCHILY.xattr.user.example", b"value"),
        ]),
        &mut budget,
    )
    .unwrap();
    finalize(root.path(), &budget).unwrap();
    let file = fs::File::open(root.path().join("content")).unwrap();
    let metadata = file.metadata().unwrap();
    assert_eq!(metadata.mtime(), 1234567890);
    assert_eq!(metadata.mtime_nsec(), 123456789);
    let mut value = [0u8; 16];
    let length = rustix::fs::fgetxattr(&file, "user.example", &mut value).unwrap();
    assert_eq!(&value[..length], b"value");
}

#[test]
fn unsupported_metadata_duplicate_paths_and_link_cycles_fail() {
    let root = private_root();
    assert!(
        apply(
            root.path(),
            &pax(&[("SCHILY.xattr.security.selinux", b"label")]),
            &mut budget()
        )
        .unwrap_err()
        .to_string()
        .contains("namespace")
    );
    assert!(
        apply(
            root.path(),
            &tar(&[file("same", b"a"), file("./same", b"b")]),
            &mut budget()
        )
        .unwrap_err()
        .to_string()
        .contains("duplicate")
    );
    assert!(
        apply(
            root.path(),
            &tar(&[link("a", "b", b'1'), link("b", "a", b'1')]),
            &mut budget()
        )
        .unwrap_err()
        .to_string()
        .contains("cyclic")
    );
    let root = private_root();
    assert!(
        apply(
            root.path(),
            &tar(&[
                link("a", "b", b'2'),
                link("b", "a", b'2'),
                file("a/file", b"bad")
            ]),
            &mut budget()
        )
        .unwrap_err()
        .to_string()
        .contains("40 links")
    );
}

fn private_root() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    root
}

#[test]
fn reverse_hardlink_chains_have_a_bounded_resolution_cost() {
    let names: Vec<_> = (0..43).map(|i| format!("file{i:03}")).collect();
    let mut items: Vec<_> = names
        .windows(2)
        .map(|pair| link(&pair[0], &pair[1], b'1'))
        .collect();
    items.push(file(names.last().unwrap(), b"data"));
    let root = private_root();
    assert!(
        apply(root.path(), &tar(&items), &mut budget())
            .unwrap_err()
            .to_string()
            .contains("40 links")
    );
}

#[test]
fn late_directory_headers_keep_pending_child_links() {
    let root = private_root();
    let mut budget = budget();
    apply(
        root.path(),
        &tar(&[
            link("dir/alias", "original", b'1'),
            Item {
                kind: b'5',
                mode: 0o755,
                ..file("dir", b"")
            },
            file("original", b"data"),
        ]),
        &mut budget,
    )
    .unwrap();
    assert_eq!(fs::read(root.path().join("dir/alias")).unwrap(), b"data");
    assert_eq!(
        fs::metadata(root.path().join("dir/alias")).unwrap().ino(),
        fs::metadata(root.path().join("original")).unwrap().ino()
    );
}

#[test]
fn invalid_pax_ownership_cannot_silently_fall_back_to_tar_header() {
    let root = private_root();
    assert!(
        apply(root.path(), &pax(&[("uid", b"-1")]), &mut budget())
            .unwrap_err()
            .to_string()
            .contains("numeric PAX")
    );
}

#[test]
fn implicit_directories_count_toward_the_physical_node_limit() {
    let root = private_root();
    let error = apply(
        root.path(),
        &tar(&[file("a/b/c/file", b"data")]),
        &mut ExtractionBudget::new(100_000, 3),
    )
    .unwrap_err();
    assert!(error.to_string().contains("staging node limit"));
    assert!(!root.path().join("a/b/c/file").exists());
}
