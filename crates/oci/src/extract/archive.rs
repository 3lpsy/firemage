use super::metadata::ExtractionBudget;
use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

pub(super) fn spool(
    path: &Path,
    media_type: &str,
    expected_diff_id: &str,
    budget: &mut ExtractionBudget,
) -> Result<File> {
    let source = File::open(path)?;
    let mut reader: Box<dyn Read> = match media_type {
        "application/vnd.oci.image.layer.v1.tar"
        | "application/vnd.docker.image.rootfs.diff.tar" => Box::new(source),
        "application/vnd.oci.image.layer.v1.tar+gzip"
        | "application/vnd.docker.image.rootfs.diff.tar.gzip" => {
            Box::new(flate2::read::MultiGzDecoder::new(source))
        }
        "application/vnd.oci.image.layer.v1.tar+zstd" => {
            let mut decoder = zstd::stream::read::Decoder::new(source)?;
            decoder.window_log_max(27)?;
            Box::new(decoder)
        }
        _ => bail!("unsupported OCI layer media type: {media_type}"),
    };
    let mut output = tempfile::tempfile_in(
        path.parent()
            .context("OCI archive has no parent directory")?,
    )
    .context("create private OCI decompression spool")?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).context("decompress OCI layer")?;
        if read == 0 {
            break;
        }
        budget.bytes = budget
            .bytes
            .checked_add(read as u64)
            .context("OCI decompression size overflow")?;
        ensure!(
            budget.bytes <= budget.max_bytes,
            "OCI decompressed byte limit exceeded"
        );
        hash.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
    }
    ensure!(
        format!("sha256:{:x}", hash.finalize()) == expected_diff_id,
        "OCI layer uncompressed diff ID mismatch"
    );
    output.rewind()?;
    preflight(&mut output, budget)?;
    Ok(output)
}

fn preflight(file: &mut File, budget: &mut ExtractionBudget) -> Result<()> {
    let length = file.metadata()?.len();
    let mut archive = tar::Archive::new(&mut *file);
    let mut extensions = 0u64;
    for item in archive.entries_with_seek()?.raw(true) {
        let entry = item.context("inspect OCI tar header")?;
        budget.entries = budget
            .entries
            .checked_add(1)
            .context("OCI entry count overflow")?;
        ensure!(
            budget.entries <= budget.max_entries,
            "OCI archive entry limit exceeded"
        );
        let kind = entry.header().entry_type();
        ensure!(
            !kind.is_gnu_sparse(),
            "OCI sparse archives are not supported"
        );
        let end = entry
            .raw_file_position()
            .checked_add(entry.size())
            .context("OCI tar offset overflow")?;
        ensure!(end <= length, "OCI tar entry extends past archive end");
        if kind.is_gnu_longname()
            || kind.is_gnu_longlink()
            || kind.is_pax_local_extensions()
            || kind.is_pax_global_extensions()
        {
            ensure!(
                entry.size() <= 64 * 1024,
                "OCI tar extension exceeds 64 KiB"
            );
            extensions += entry.size();
            ensure!(
                extensions <= 16 * 1024 * 1024,
                "OCI tar extensions exceed 16 MiB per layer"
            );
        }
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(())
}
