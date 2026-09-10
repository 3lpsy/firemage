use std::{io::Read, path::Path};

use anyhow::Context;

pub fn read(path: &Path, maximum: u64, is_empty_allowed: bool) -> anyhow::Result<Vec<u8>> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let metadata = file.metadata()?;
    anyhow::ensure!(metadata.is_file(), "upload must be a regular file");
    ensure_size(metadata.len(), maximum, is_empty_allowed)?;
    let mut bytes = Vec::new();
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    ensure_size(bytes.len() as u64, maximum, is_empty_allowed)?;
    Ok(bytes)
}

fn ensure_size(size: u64, maximum: u64, is_empty_allowed: bool) -> anyhow::Result<()> {
    anyhow::ensure!(
        size <= maximum && (size > 0 || is_empty_allowed),
        "upload must contain {}-{maximum} bytes",
        u64::from(!is_empty_allowed)
    );
    Ok(())
}
