use crate::{create_private, hash_file, open_private, validate_manifest};
use anyhow::{Context, ensure};
use firemage_wire::{SnapshotFile, SnapshotManifest};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::Path,
};

pub fn pack(
    directory: &Path,
    mut manifest: SnapshotManifest,
    output: &Path,
    limit: u64,
) -> anyhow::Result<SnapshotManifest> {
    for (name, file) in &mut manifest.files {
        ensure!(is_component(name), "invalid snapshot component filename");
        let (size_bytes, sha256) = hash_file(&directory.join(name), limit)?;
        *file = SnapshotFile { size_bytes, sha256 };
    }
    validate_manifest(&manifest, limit)?;
    let encoded = serde_json::to_vec(&manifest)?;
    ensure!(
        encoded.len() <= 1024 * 1024,
        "snapshot manifest exceeds 1 MiB"
    );
    let encoder = GzEncoder::new(create_private(output)?, Compression::fast());
    let mut archive = tar::Builder::new(encoder);
    let mut manifest_header = header(encoded.len() as u64);
    archive.append_data(&mut manifest_header, "manifest.json", encoded.as_slice())?;
    for (name, meta) in &manifest.files {
        let file = open_private(&directory.join(name))?;
        ensure!(
            file.metadata()?.len() == meta.size_bytes,
            "snapshot component changed while packaging"
        );
        archive.append_data(&mut header(meta.size_bytes), name, file)?;
    }
    let file = archive.into_inner()?.finish()?;
    file.sync_all()?;
    ensure!(
        file.metadata()?.len() <= limit,
        "snapshot bundle exceeds configured size limit"
    );
    Ok(manifest)
}
fn header(size: u64) -> tar::Header {
    let mut header = tar::Header::new_gnu();
    header.set_size(size);
    header.set_mode(0o600);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    header
}
fn is_component(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && name != "."
        && name != ".."
}

pub fn inspect(archive: &Path, limit: u64) -> anyhow::Result<SnapshotManifest> {
    read_bundle(archive, None, limit)
}
pub fn unpack(archive: &Path, destination: &Path, limit: u64) -> anyhow::Result<SnapshotManifest> {
    ensure!(
        std::fs::symlink_metadata(destination)?.is_dir()
            && std::fs::read_dir(destination)?.next().is_none(),
        "snapshot extraction requires an empty private directory"
    );
    read_bundle(archive, Some(destination), limit)
}
fn read_bundle(
    path: &Path,
    destination: Option<&Path>,
    limit: u64,
) -> anyhow::Result<SnapshotManifest> {
    let file = open_private(path)?;
    ensure!(
        file.metadata()?.len() <= limit,
        "snapshot bundle exceeds configured size limit"
    );
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    let mut entries = archive.entries()?.raw(true);
    let mut first = entries.next().context("snapshot bundle is empty")??;
    ensure!(
        first.header().entry_type().is_file()
            && first.path()?.as_ref() == Path::new("manifest.json")
            && first.size() <= 1024 * 1024,
        "snapshot bundle must start with a regular manifest.json up to 1 MiB"
    );
    let mut encoded = Vec::new();
    first.read_to_end(&mut encoded)?;
    let manifest: SnapshotManifest =
        serde_json::from_slice(&encoded).context("invalid snapshot manifest")?;
    validate_manifest(&manifest, limit)?;
    drop(first);
    let mut seen = BTreeSet::new();
    for entry in entries {
        let mut entry = entry?;
        ensure!(
            entry.header().entry_type().is_file(),
            "snapshot archives cannot contain links or special files"
        );
        let name = entry
            .path()?
            .to_str()
            .context("snapshot component is not UTF-8")?
            .to_owned();
        let metadata = manifest
            .files
            .get(&name)
            .context("snapshot contains an unlisted file")?;
        ensure!(
            seen.insert(name.clone()),
            "snapshot contains a duplicate file"
        );
        ensure!(
            entry.size() == metadata.size_bytes,
            "snapshot file size does not match manifest"
        );
        let mut output = destination
            .map(|dir| create_private(&dir.join(&name)))
            .transpose()?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let count = entry.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
            if let Some(file) = &mut output {
                file.write_all(&buffer[..count])?;
            }
        }
        ensure!(
            format!("{:x}", hash.finalize()) == metadata.sha256,
            "snapshot component hash mismatch: {name}"
        );
        if let Some(file) = &mut output {
            file.sync_all()?;
        }
    }
    ensure!(
        seen.len() == manifest.files.len(),
        "snapshot bundle is missing files"
    );
    Ok(manifest)
}
