use super::*;

#[test]
fn basename_and_first_free_suffix_are_stable() {
    let mut used = HashSet::new();
    assert_eq!(available_alias("/catalog/vmlinux", &used), "vmlinux");
    used.extend(["vmlinux".into(), "vmlinux-1".into(), "vmlinux-3".into()]);
    assert_eq!(available_alias("vmlinux", &used), "vmlinux-2");
}

#[test]
fn suffixes_fit_the_alias_limit_at_utf8_boundaries() {
    let filename = "é".repeat(100);
    let base = available_alias(&filename, &HashSet::new());
    assert_eq!(base.len(), 128);
    let used = HashSet::from([base]);
    let next = available_alias(&filename, &used);
    assert_eq!(next.len(), 128);
    assert!(next.ends_with("-1"));
    firemage_wire::ensure_asset_alias(&next).unwrap();
}

#[test]
fn timestamp_follows_the_2048th_suffix_and_avoids_timestamp_collisions() {
    let now = "2026-09-11T12:34:56.123456789Z"
        .parse::<chrono::DateTime<chrono::Utc>>()
        .unwrap();
    let mut used = HashSet::from(["kernel".into()]);
    used.extend((1..2048).map(|number| format!("kernel-{number}")));
    assert_eq!(available_alias_at("kernel", &used, now), "kernel-2048");
    used.insert("kernel-2048".into());
    assert_eq!(
        available_alias_at("kernel", &used, now),
        "20260911T123456.123456789Z"
    );
    used.insert("20260911T123456.123456789Z".into());
    assert_eq!(
        available_alias_at("kernel", &used, now),
        "20260911T123456.123456790Z"
    );
}
