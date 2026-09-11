use anyhow::Context;
use firemage_wire::{GuestFileEntry, GuestFileKind};

pub(super) fn directory(mut bytes: &[u8]) -> anyhow::Result<Vec<GuestFileEntry>> {
    let mut entries = Vec::new();
    while !bytes.is_empty() {
        if bytes[0] == b'\n' {
            bytes = &bytes[1..];
            continue;
        }
        anyhow::ensure!(bytes[0] == b'/', "invalid guest directory record");
        bytes = &bytes[1..];
        let mut fields = Vec::with_capacity(6);
        for _ in 0..6 {
            let end = bytes
                .iter()
                .position(|byte| *byte == b'/')
                .context("incomplete guest directory record")?;
            fields.push(&bytes[..end]);
            bytes = &bytes[end + 1..];
        }
        anyhow::ensure!(
            bytes.first() == Some(&b'\n'),
            "invalid guest directory separator"
        );
        bytes = &bytes[1..];
        let inode: u32 = std::str::from_utf8(fields[0])?.parse()?;
        if inode == 0 {
            continue;
        }
        let mode = u32::from_str_radix(std::str::from_utf8(fields[1])?, 8)?;
        anyhow::ensure!(mode <= 0o177777, "invalid guest file mode");
        let kind = match mode & 0o170000 {
            0o100000 => GuestFileKind::File,
            0o040000 => GuestFileKind::Directory,
            0o120000 => GuestFileKind::Symlink,
            _ => GuestFileKind::Special,
        };
        let name = fields[4];
        anyhow::ensure!(
            !name.is_empty() && name.len() <= 255 && !name.contains(&0),
            "invalid guest filename"
        );
        if name == b"." || name == b".." {
            continue;
        }
        let name = String::from_utf8_lossy(name)
            .chars()
            .flat_map(|ch| {
                if ch.is_control() {
                    ch.escape_default().collect::<Vec<_>>()
                } else {
                    vec![ch]
                }
            })
            .collect();
        let size_bytes = if fields[5].is_empty() && kind == GuestFileKind::Directory {
            None
        } else {
            Some(std::str::from_utf8(fields[5])?.parse()?)
        };
        anyhow::ensure!(entries.len() < 4096, "guest directory exceeds 4096 entries");
        entries.push(GuestFileEntry {
            inode,
            name,
            kind,
            size_bytes,
            uid: owner(fields[2])?,
            gid: owner(fields[3])?,
            mode: mode & 0o7777,
        });
    }
    entries.sort_by(|a, b| {
        (a.kind != GuestFileKind::Directory, &a.name)
            .cmp(&(b.kind != GuestFileKind::Directory, &b.name))
    });
    Ok(entries)
}

fn owner(bytes: &[u8]) -> anyhow::Result<u32> {
    let text = std::str::from_utf8(bytes)?;
    if text.starts_with('-') {
        Ok(text.parse::<i32>()? as u32)
    } else {
        Ok(text.parse()?)
    }
}

pub(super) fn ensure_type(bytes: &[u8], inode: u32, kind: &str) -> anyhow::Result<()> {
    let text = std::str::from_utf8(bytes)?;
    let mut fields = text.lines().next().unwrap_or_default().split_whitespace();
    anyhow::ensure!(
        fields.next() == Some("Inode:")
            && fields.next().and_then(|value| value.parse::<u32>().ok()) == Some(inode)
            && fields.next() == Some("Type:")
            && fields.next() == Some(kind),
        "guest inode is not a {kind}"
    );
    Ok(())
}
