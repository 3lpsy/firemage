use std::collections::HashSet;

pub(crate) fn available_alias(filename: &str, used: &HashSet<String>) -> String {
    available_alias_at(filename, used, chrono::Utc::now())
}

fn available_alias_at(
    filename: &str,
    used: &HashSet<String>,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    let basename = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let basename = basename.trim();
    let basename = if basename.is_empty() {
        "file"
    } else {
        basename
    };
    let candidate = truncate(basename, 128).trim_end();
    if !used.contains(candidate) {
        return candidate.into();
    }
    for number in 1..=2048 {
        let suffix = format!("-{number}");
        let candidate = format!("{}{suffix}", truncate(basename, 128 - suffix.len()));
        if !used.contains(&candidate) {
            return candidate;
        }
    }
    let mut timestamp = now;
    loop {
        let candidate = timestamp.format("%Y%m%dT%H%M%S%.9fZ").to_string();
        if !used.contains(&candidate) {
            return candidate;
        }
        timestamp += chrono::Duration::nanoseconds(1);
    }
}

fn truncate(value: &str, maximum: usize) -> &str {
    let mut end = value.len().min(maximum);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(test)]
#[path = "catalog_alias/tests.rs"]
mod tests;
