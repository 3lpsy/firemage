use super::Catalog;
use std::os::unix::fs::symlink;

#[test]
fn catalog_discovers_files_and_rejects_paths_links_and_overwrites() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("kernels");
    let catalog = Catalog::open(&path).unwrap();
    std::fs::write(path.join("vmlinux-6.1"), b"kernel").unwrap();
    let outside = root.path().join("private");
    std::fs::write(&outside, b"private").unwrap();
    symlink(&outside, path.join("symbolic")).unwrap();
    std::fs::hard_link(&outside, path.join("hard")).unwrap();
    std::fs::create_dir(path.join("directory")).unwrap();
    std::fs::write(path.join(".partial"), b"partial").unwrap();
    assert_eq!(
        catalog
            .list()
            .unwrap()
            .iter()
            .map(|k| k.name.as_str())
            .collect::<Vec<_>>(),
        ["vmlinux-6.1"]
    );
    for name in [
        "../private",
        "/private",
        "nested/kernel",
        "",
        ".",
        "..",
        "symbolic",
        "hard",
        "directory",
    ] {
        assert!(catalog.file(name).is_err(), "{name}");
        assert!(catalog.remove(name).is_err(), "{name}");
    }
    assert!(catalog.upload("vmlinux-6.1", b"replacement").is_err());
    assert!(catalog.upload("symbolic", b"replacement").is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), b"private");
    assert_eq!(std::fs::read(path.join("vmlinux-6.1")).unwrap(), b"kernel");
    catalog.upload("uploaded", b"uploaded kernel").unwrap();
    catalog.copy("uploaded", &root.path().join("copy")).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("copy")).unwrap(),
        b"uploaded kernel"
    );
    catalog.remove("uploaded").unwrap();
    assert!(!path.join("uploaded").exists());
    assert!(catalog.upload("empty", b"").is_err());
}

#[test]
fn catalog_rejects_symlink_ancestors_and_keeps_open_directory_after_replacement() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("link")).unwrap();
    assert!(Catalog::open(&root.path().join("link/kernels")).is_err());
    assert!(!outside.path().join("kernels").exists());
    let path = root.path().join("kernels");
    let catalog = Catalog::open(&path).unwrap();
    std::fs::rename(&path, root.path().join("original")).unwrap();
    symlink(outside.path(), &path).unwrap();
    catalog.upload("vmlinux", b"kernel").unwrap();
    assert!(root.path().join("original/vmlinux").exists());
    assert!(!outside.path().join("vmlinux").exists());
}

#[tokio::test]
async fn imports_reject_unverified_and_private_destinations_before_publishing() {
    let root = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(root.path()).unwrap();
    for url in [
        "http://example.com/kernel",
        "https://127.0.0.1/kernel",
        "https://100.64.0.1/kernel",
        "https://user:pass@example.com/kernel",
    ] {
        let request = firemage_wire::KernelImport {
            name: "vmlinux".into(),
            alias: "Linux".into(),
            url: url.into(),
            sha256: "a".repeat(64),
        };
        assert!(super::fetch(&catalog, &request).await.is_err());
    }
    assert!(catalog.list().unwrap().is_empty());
}
