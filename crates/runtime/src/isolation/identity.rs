use crate::Runtime;
use anyhow::Context;
use std::os::unix::fs::OpenOptionsExt;
impl Runtime {
    pub(crate) fn jail_uid(&self, id: &str) -> anyhow::Result<u32> {
        let base = self.config.jailer_uid_base.context(
            "jailed VMs require an explicitly reserved jailer_uid_base and jailer_uid_count",
        )?;
        let count = self
            .config
            .jailer_uid_count
            .context("missing jailer_uid_count")?;
        self.config.validate()?;
        ensure_reserved_range(base, count)?;
        let directory = self.config.data_dir().join("jailer-identities");
        std::fs::create_dir_all(&directory)?;
        super::ensure_trusted_path(&directory)?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(directory.join(".lock"))?;
        lock.lock()?;
        let record = directory.join(id);
        if record.exists() {
            let uid: u32 = std::fs::read_to_string(record)?.trim().parse()?;
            anyhow::ensure!(
                (base..base + count).contains(&uid),
                "persisted VM UID falls outside configured reserved range"
            );
            return Ok(uid);
        }
        let mut used = std::collections::HashSet::new();
        for item in std::fs::read_dir(&directory)? {
            let item = item?;
            if item.file_name() != ".lock" {
                used.insert(
                    std::fs::read_to_string(item.path())?
                        .trim()
                        .parse::<u32>()?,
                );
            }
        }
        let uid = (base..base + count)
            .find(|uid| !used.contains(uid))
            .context("reserved jailer identity range is exhausted")?;
        firemage_config::write_private(&record, uid.to_string().as_bytes())?;
        Ok(uid)
    }
}
fn ensure_reserved_range(base: u32, count: u32) -> anyhow::Result<()> {
    for (name, field) in [("/etc/passwd", 2), ("/etc/group", 2)] {
        for line in std::fs::read_to_string(name)?.lines() {
            if let Some(value) = line
                .split(':')
                .nth(field)
                .and_then(|v| v.parse::<u32>().ok())
            {
                anyhow::ensure!(
                    !(base..base + count).contains(&value),
                    "jailer identity range overlaps {name}"
                );
            }
        }
    }
    for name in ["/etc/subuid", "/etc/subgid"] {
        let text = match std::fs::read_to_string(name) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let parts: Vec<_> = line.split(':').collect();
            anyhow::ensure!(parts.len() == 3, "invalid subordinate identity file {name}");
            let start: u64 = parts[1].parse()?;
            let length: u64 = parts[2].parse()?;
            anyhow::ensure!(
                u64::from(base) >= start + length || u64::from(base) + u64::from(count) <= start,
                "jailer identity range overlaps {name}"
            );
        }
    }
    Ok(())
}
