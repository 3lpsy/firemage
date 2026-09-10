use anyhow::{Context, Result, bail, ensure};
use rustix::fs::{Timespec, Timestamps};

pub(super) fn timestamp(value: &[u8]) -> Result<Timestamps> {
    let value = std::str::from_utf8(value).context("OCI mtime is not UTF-8")?;
    let negative = value.starts_with('-');
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (seconds, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    ensure!(
        !seconds.is_empty()
            && seconds.bytes().all(|b| b.is_ascii_digit())
            && fraction.bytes().all(|b| b.is_ascii_digit()),
        "invalid OCI mtime"
    );
    let seconds: i64 = seconds.parse().context("OCI mtime exceeds i64")?;
    ensure!(
        fraction.len() <= 9,
        "OCI mtime exceeds nanosecond precision"
    );
    let nanos: i64 = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i64>()? * 10_i64.pow(9 - fraction.len() as u32)
    };
    let (tv_sec, tv_nsec) = if negative && nanos != 0 {
        (
            seconds
                .checked_neg()
                .and_then(|n| n.checked_sub(1))
                .context("OCI mtime underflow")?,
            1_000_000_000 - nanos,
        )
    } else if negative {
        (-seconds, 0)
    } else {
        (seconds, nanos)
    };
    let time = Timespec { tv_sec, tv_nsec };
    Ok(Timestamps {
        last_access: time,
        last_modification: time,
    })
}

pub(super) fn xattr_name(key: &[u8]) -> Result<Option<&str>> {
    if key.starts_with(b"LIBARCHIVE.xattr.") {
        bail!("OCI LIBARCHIVE encoded xattrs are not supported");
    }
    if key.starts_with(b"SCHILY.acl.") {
        bail!("OCI POSIX ACL metadata is not supported");
    }
    let Some(name) = key.strip_prefix(b"SCHILY.xattr.") else {
        return Ok(None);
    };
    let name = std::str::from_utf8(name).context("OCI xattr name is not UTF-8")?;
    ensure!(
        name.len() <= 255 && !name.contains('\0'),
        "invalid OCI xattr name"
    );
    ensure!(
        (name.starts_with("user.") && name.len() > 5) || name == "security.capability",
        "unsupported OCI xattr namespace: {name}"
    );
    Ok(Some(name))
}

pub(super) fn decimal(value: &[u8]) -> Result<u64> {
    ensure!(
        !value.is_empty() && value.iter().all(u8::is_ascii_digit),
        "invalid OCI numeric PAX value"
    );
    std::str::from_utf8(value)?
        .parse()
        .context("OCI numeric PAX value exceeds u64")
}
