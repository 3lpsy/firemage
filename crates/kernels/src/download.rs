use crate::Catalog;
use firemage_wire::{KERNEL_MAX_BYTES, KernelImport};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

pub async fn fetch(
    catalog: &Catalog,
    input: &KernelImport,
) -> anyhow::Result<tempfile::NamedTempFile> {
    firemage_wire::ensure_kernel_name(&input.name)?;
    anyhow::ensure!(
        input.sha256.len() == 64 && input.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "kernel download requires a SHA-256 digest"
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(300),
        fetch_inner(catalog, input),
    )
    .await
    .map_err(|_| anyhow::anyhow!("kernel download exceeded 300 seconds"))?
}
async fn fetch_inner(
    catalog: &Catalog,
    input: &KernelImport,
) -> anyhow::Result<tempfile::NamedTempFile> {
    anyhow::ensure!(input.url.len() <= 4096, "kernel URL is too long");
    let mut url = reqwest::Url::parse(&input.url)?;
    ensure_url(&url)?;
    let mut redirects = 0;
    let mut response = loop {
        let client = pinned_client(&url).await?;
        let response = client.get(url.clone()).send().await?;
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or_else(|| anyhow::anyhow!("kernel redirect is missing Location"))?
                .to_str()?;
            url = redirect_target(&url, location, redirects)?;
            redirects += 1;
            continue;
        }
        anyhow::ensure!(
            response.status().is_success(),
            "kernel download failed with HTTP {}",
            response.status()
        );
        break response;
    };
    anyhow::ensure!(
        response
            .content_length()
            .is_none_or(|size| size <= KERNEL_MAX_BYTES),
        "kernel exceeds 128 MiB"
    );
    let temporary = catalog.temporary()?;
    let mut file = tokio::fs::File::from_std(temporary.as_file().try_clone()?);
    let mut hash = Sha256::new();
    let mut size = 0u64;
    while let Some(chunk) = response.chunk().await? {
        size += chunk.len() as u64;
        anyhow::ensure!(size <= KERNEL_MAX_BYTES, "kernel exceeds 128 MiB");
        hash.update(&chunk);
        file.write_all(&chunk).await?;
    }
    anyhow::ensure!(
        hex::encode(hash.finalize()).eq_ignore_ascii_case(&input.sha256),
        "kernel SHA-256 does not match"
    );
    file.sync_all().await?;
    drop(file);
    Ok(temporary)
}
// Recheck each redirect before resolving it, including HTTPS downgrade and literal IP targets.
pub(crate) fn redirect_target(
    base: &reqwest::Url,
    location: &str,
    followed: usize,
) -> anyhow::Result<reqwest::Url> {
    anyhow::ensure!(followed < 5, "kernel download exceeded five redirects");
    anyhow::ensure!(location.len() <= 4096, "kernel redirect URL is too long");
    let target = base.join(location)?;
    ensure_url(&target)?;
    Ok(target)
}
fn ensure_url(url: &reqwest::Url) -> anyhow::Result<()> {
    anyhow::ensure!(url.as_str().len() <= 4096, "kernel URL is too long");
    anyhow::ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "kernel URL must use HTTPS without credentials or fragment"
    );
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("kernel URL requires a host"))?;
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
        anyhow::ensure!(
            is_public(ip),
            "kernel downloads require a public destination"
        );
    }
    Ok(())
}
pub(crate) fn ensure_addresses(addresses: &[std::net::SocketAddr]) -> anyhow::Result<()> {
    anyhow::ensure!(
        !addresses.is_empty() && addresses.iter().all(|address| is_public(address.ip())),
        "kernel downloads require a public destination"
    );
    Ok(())
}
async fn pinned_client(url: &reqwest::Url) -> anyhow::Result<reqwest::Client> {
    ensure_url(url)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("kernel URL requires a host"))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses: Vec<_> =
        if let Ok(ip) = host.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
            vec![std::net::SocketAddr::new(ip, port)]
        } else {
            tokio::time::timeout(
                std::time::Duration::from_secs(15),
                tokio::net::lookup_host((host, port)),
            )
            .await??
            .collect()
        };
    ensure_addresses(&addresses)?;
    Ok(reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(300))
        .resolve_to_addrs(host, &addresses)
        .build()?)
}
fn is_public(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_multicast()
                || a == 0
                || a >= 240
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (b == 18 || b == 19))
                || (a == 192 && b == 0))
        }
        std::net::IpAddr::V6(ip) => {
            let segments = ip.segments();
            // Only native public unicast; exclude mapped, NAT64, transition and documentation ranges.
            segments[0] & 0xe000 == 0x2000
                && segments[0] != 0x2002
                && !(segments[0] == 0x2001 && (segments[1] < 0x200 || segments[1] == 0xdb8))
                && segments[0] != 0x3fff
        }
    }
}
