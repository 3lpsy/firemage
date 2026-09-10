use anyhow::{Result, ensure};
use hyper::{Request, body::Incoming, header::HOST, http::uri::Authority};

pub(crate) fn parse(value: &str, default_port: u16) -> Result<(String, u16)> {
    ensure!(
        !value.contains('@') && !value.contains('%') && !value.ends_with('.'),
        "invalid authority"
    );
    let authority: Authority = value.parse()?;
    let suffix = if value.starts_with('[') {
        value.split_once(']').map(|(_, suffix)| suffix)
    } else {
        value.find(':').map(|index| &value[index..])
    };
    let port = match suffix.filter(|suffix| !suffix.is_empty()) {
        Some(suffix) => suffix
            .strip_prefix(':')
            .ok_or_else(|| anyhow::anyhow!("invalid authority port"))?
            .parse::<u16>()?,
        None => default_port,
    };
    ensure!(port > 0, "invalid authority port");
    let host = authority
        .host()
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();
    ensure!(!host.is_empty() && !host.ends_with('.'), "invalid host");
    Ok((host, port))
}

pub(crate) fn ensure_host(
    request: &Request<Incoming>,
    expected: &(String, u16),
    default_port: u16,
) -> Result<()> {
    ensure!(
        request.headers().get_all(HOST).iter().count() == 1,
        "exactly one Host header is required"
    );
    let host = request
        .headers()
        .get(HOST)
        .ok_or_else(|| anyhow::anyhow!("Host header required"))?
        .to_str()?;
    ensure!(
        &parse(host, default_port)? == expected,
        "Host does not match request authority"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn invalid_explicit_ports_never_use_defaults() {
        for authority in [
            "example.com:65536",
            "example.com:abc",
            "example.com:",
            "example.com:0",
            "user@example.com",
            "example.com.:443",
        ] {
            assert!(super::parse(authority, 80).is_err(), "{authority}");
        }
        assert_eq!(
            super::parse("example.com:443", 80).unwrap(),
            ("example.com".into(), 443)
        );
        assert_eq!(
            super::parse("[::1]:8080", 80).unwrap(),
            ("::1".into(), 8080)
        );
    }
}
