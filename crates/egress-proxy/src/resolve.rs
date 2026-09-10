use anyhow::{Result, bail, ensure};
use ipnet::IpNet;
use std::net::{IpAddr, SocketAddr};

pub(crate) async fn destination(
    host: &str,
    port: u16,
    allowed: &[IpNet],
    public_only: bool,
) -> Result<SocketAddr> {
    let addresses = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::net::lookup_host((host, port)),
    )
    .await??;
    for address in addresses {
        let ip = address.ip();
        if !allowed.is_empty() {
            if allowed.iter().any(|net| net.contains(&ip)) {
                return Ok(address);
            }
        } else if !public_only || is_public(ip) {
            return Ok(address);
        }
    }
    bail!("destination has no policy-approved IP address")
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            !ip.is_private()
                && !ip.is_loopback()
                && !ip.is_link_local()
                && !ip.is_broadcast()
                && !ip.is_documentation()
                && !ip.is_multicast()
                && a != 0
                && a < 240
                && !(a == 100 && (64..128).contains(&b))
                && !(a == 198 && (18..20).contains(&b))
                && !(a == 192 && b == 0)
        }
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            (segments[0] & 0xe000) == 0x2000
                && !(segments[0] == 0x2001 && (segments[1] == 0xdb8 || segments[1] < 0x200))
        }
    }
}

pub(crate) fn ensure_safe_path(path: &str) -> Result<()> {
    ensure!(
        path.starts_with('/') && !path.contains('\\') && !path.bytes().any(|b| b <= 32),
        "invalid request path"
    );
    let lower = path.to_ascii_lowercase();
    ensure!(
        !["%2e", "%2f", "%5c", "%25", "%00"]
            .iter()
            .any(|s| lower.contains(s))
            && !path.split('/').any(|s| s == "." || s == ".."),
        "ambiguous request path"
    );
    Ok(())
}
