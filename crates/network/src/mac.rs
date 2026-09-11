use anyhow::Context;

pub async fn current_tap_mac(id: &str) -> anyhow::Result<String> {
    let (tap, _) = crate::identifiers(id)?;
    let mac = tokio::fs::read_to_string(format!("/sys/class/net/{tap}/address")).await?;
    validated_mac(mac.trim())
}

pub async fn set_tap_mac(id: &str, mac: &str) -> anyhow::Result<()> {
    let (tap, _) = crate::identifiers(id)?;
    let mac = validated_mac(mac)?;
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new("ip")
            .args(["link", "set", "dev", &tap, "address", &mac])
            .kill_on_drop(true)
            .output(),
    )
    .await??;
    anyhow::ensure!(
        output.status.success(),
        "cannot restore gateway MAC: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    anyhow::ensure!(
        current_tap_mac(id).await? == mac,
        "gateway MAC was not restored"
    );
    Ok(())
}

fn validated_mac(mac: &str) -> anyhow::Result<String> {
    let pieces: Vec<_> = mac.split(':').collect();
    anyhow::ensure!(
        pieces.len() == 6 && pieces.iter().all(|value| value.len() == 2),
        "invalid gateway MAC"
    );
    let bytes = pieces
        .into_iter()
        .map(|value| u8::from_str_radix(value, 16))
        .collect::<Result<Vec<_>, _>>()
        .context("invalid gateway MAC")?;
    anyhow::ensure!(
        bytes[0] & 1 == 0 && bytes.iter().any(|byte| *byte != 0),
        "gateway MAC must be nonzero and unicast"
    );
    Ok(mac.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    #[test]
    fn gateway_mac_rejects_multicast_zero_and_command_fragments() {
        assert_eq!(
            super::validated_mac("02:AA:00:11:22:33").unwrap(),
            "02:aa:00:11:22:33"
        );
        for mac in [
            "00:00:00:00:00:00",
            "ff:ff:ff:ff:ff:ff",
            "01:00:00:00:00:00",
            "02:00:00:00:00:zz",
            "02:00:00:00:00:00 dev eth0",
            "2:00:00:00:00:00",
        ] {
            assert!(super::validated_mac(mac).is_err(), "{mac}");
        }
    }
}
