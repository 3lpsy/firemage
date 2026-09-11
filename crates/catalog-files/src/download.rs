use crate::Directory;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

pub struct Download {
    pub file: tempfile::NamedTempFile,
    pub size_bytes: u64,
    pub sha256: String,
}

impl Directory {
    pub async fn download(&self, url: &str, sha256: Option<&str>) -> anyhow::Result<Download> {
        if let Some(hash) = sha256 {
            anyhow::ensure!(
                hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid SHA-256 digest"
            );
        }
        tokio::time::timeout(
            std::time::Duration::from_secs(300),
            fetch_inner(self, url, sha256),
        )
        .await
        .map_err(|_| anyhow::anyhow!("catalog download exceeded 300 seconds"))?
    }
}

async fn fetch_inner(
    catalog: &Directory,
    source: &str,
    sha256: Option<&str>,
) -> anyhow::Result<Download> {
    anyhow::ensure!(source.len() <= 4096, "catalog URL is too long");
    let mut url = reqwest::Url::parse(source)?;
    ensure_url(&url)?;
    let mut redirects = 0;
    let response = loop {
        let client = pinned_client(&url).await?;
        let response = client.get(url.clone()).send().await?;
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or_else(|| anyhow::anyhow!("catalog redirect is missing Location"))?
                .to_str()?;
            url = redirect_target(&url, location, redirects)?;
            redirects += 1;
            continue;
        }
        anyhow::ensure!(
            response.status().is_success(),
            "catalog download failed with HTTP {}",
            response.status()
        );
        break response;
    };
    save_response(catalog, response, sha256).await
}

pub(crate) async fn save_response(
    catalog: &Directory,
    mut response: reqwest::Response,
    sha256: Option<&str>,
) -> anyhow::Result<Download> {
    let maximum = catalog.maximum();
    anyhow::ensure!(
        response.content_length().is_none_or(|size| size <= maximum),
        "download exceeds {maximum} bytes"
    );
    let temporary = catalog.temporary()?;
    let mut file = tokio::fs::File::from_std(temporary.as_file().try_clone()?);
    let mut hash = Sha256::new();
    let mut size = 0u64;
    while let Some(chunk) = response.chunk().await? {
        size = size
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| anyhow::anyhow!("download size overflow"))?;
        anyhow::ensure!(size <= maximum, "download exceeds {maximum} bytes");
        hash.update(&chunk);
        file.write_all(&chunk).await?;
    }
    let actual = hex::encode(hash.finalize());
    anyhow::ensure!(
        sha256.is_none_or(|expected| actual.eq_ignore_ascii_case(expected)),
        "download SHA-256 does not match"
    );
    file.sync_all().await?;
    drop(file);
    Ok(Download {
        file: temporary,
        size_bytes: size,
        sha256: actual,
    })
}
// Recheck each redirect before resolving it, including HTTPS downgrade and literal IP targets.
pub(crate) fn redirect_target(
    base: &reqwest::Url,
    location: &str,
    followed: usize,
) -> anyhow::Result<reqwest::Url> {
    anyhow::ensure!(followed < 5, "catalog download exceeded five redirects");
    anyhow::ensure!(location.len() <= 4096, "catalog redirect URL is too long");
    let target = base.join(location)?;
    ensure_url(&target)?;
    Ok(target)
}
fn ensure_url(url: &reqwest::Url) -> anyhow::Result<()> {
    anyhow::ensure!(url.as_str().len() <= 4096, "catalog URL is too long");
    anyhow::ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "catalog URL must use HTTPS without credentials or fragment"
    );
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("catalog URL requires a host"))?;
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
        anyhow::ensure!(
            is_public(ip),
            "catalog downloads require a public destination"
        );
    }
    Ok(())
}
pub(crate) fn ensure_addresses(addresses: &[std::net::SocketAddr]) -> anyhow::Result<()> {
    anyhow::ensure!(
        !addresses.is_empty() && addresses.iter().all(|address| is_public(address.ip())),
        "catalog downloads require a public destination"
    );
    Ok(())
}
async fn pinned_client(url: &reqwest::Url) -> anyhow::Result<reqwest::Client> {
    ensure_url(url)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("catalog URL requires a host"))?;
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
