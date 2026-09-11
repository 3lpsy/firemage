use crate::{
    authority,
    http::{Body, Context},
    resolve, transport,
};
use anyhow::{Result, ensure};
use firemage_egress_policy::HttpScheme;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{Request, Response, body::Incoming, header::HeaderMap};
use hyper_util::rt::TokioIo;
use std::sync::Arc;

pub(crate) async fn forward(
    request: Request<Incoming>,
    context: Arc<Context>,
    target: Option<(String, u16)>,
) -> Result<Response<Body>> {
    ensure!(
        matches!(
            request.method().as_str(),
            "GET" | "HEAD" | "POST" | "PUT" | "DELETE" | "OPTIONS" | "PATCH"
        ),
        "method denied"
    );
    ensure!(
        !request.headers().contains_key("upgrade"),
        "protocol upgrades are denied"
    );
    let scheme = if target.is_some() {
        HttpScheme::Https
    } else {
        HttpScheme::Http
    };
    let default_port = if scheme == HttpScheme::Https { 443 } else { 80 };
    let authority = if let Some(target) = target {
        if let Some(value) = request.uri().authority() {
            ensure!(
                authority::parse(value.as_str(), 443)? == target,
                "TLS authority mismatch"
            );
        }
        ensure!(
            request.uri().scheme_str().is_none_or(|s| s == "https"),
            "TLS scheme mismatch"
        );
        target
    } else {
        ensure!(
            request.uri().scheme_str() == Some("http"),
            "absolute HTTP URI required"
        );
        authority::parse(
            request
                .uri()
                .authority()
                .ok_or_else(|| anyhow::anyhow!("request authority required"))?
                .as_str(),
            80,
        )?
    };
    authority::ensure_host(&request, &authority, default_port)?;
    let path = request.uri().path();
    resolve::ensure_safe_path(path)?;
    let rule = context
        .policy
        .rules
        .iter()
        .find(|rule| {
            rule.is_match(
                scheme,
                &authority.0,
                authority.1,
                request.method().as_str(),
                path,
            )
        })
        .ok_or_else(|| anyhow::anyhow!("request denied by VM policy"))?;
    let address = resolve::destination(&authority.0, authority.1, &rule.allowed_ips, true).await?;
    let host = if authority.0.contains(':') {
        format!("[{}]", authority.0)
    } else {
        authority.0.clone()
    };
    let url = format!(
        "{}://{}:{}{}",
        scheme.as_str(),
        host,
        authority.1,
        request
            .uri()
            .path_and_query()
            .map(|p| p.as_str())
            .unwrap_or("/")
    );
    let (parts, body) = request.into_parts();
    let body = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        Limited::new(body, 16 * 1024 * 1024).collect(),
    )
    .await?
    .map_err(|_| anyhow::anyhow!("request body exceeds limit or is invalid"))?
    .to_bytes();
    let mut headers = parts.headers;
    clean_headers(&mut headers);
    headers.insert("host", format!("{host}:{}", authority.1).parse()?);
    firemage_request_signing::apply(
        context.secrets.as_ref(),
        &rule.headers,
        rule.signing.as_ref(),
        parts.method.as_str(),
        &url,
        &mut headers,
        &body,
    )
    .await?;
    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        transport::connect(address, context.upstream.as_ref(), context.secrets.as_ref()),
    )
    .await??;
    if scheme == HttpScheme::Https {
        stream = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            transport::tls(
                stream,
                &authority.0,
                context.policy.upstream_ca_pem.as_deref(),
            ),
        )
        .await??;
    }
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;
    let cancel = context.cancel.clone();
    context.tasks.spawn(async move {
        tokio::select! {_ = cancel.cancelled()=>{},_ = connection=>{}}
    });
    let uri = parts
        .uri
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/");
    let mut outgoing = Request::builder()
        .method(parts.method)
        .uri(uri)
        .body(Full::new(body))?;
    *outgoing.headers_mut() = headers;
    let upstream = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        sender.send_request(outgoing),
    )
    .await??;
    let (mut parts, body) = upstream.into_parts();
    clean_headers(&mut parts.headers);
    let output = Response::from_parts(
        parts,
        body.map_err(|e| -> crate::http::BoxError { Box::new(e) })
            .boxed(),
    );
    Ok(output)
}

pub(crate) fn clean_headers(headers: &mut HeaderMap) {
    let nominated: Vec<String> = headers
        .get_all("connection")
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|v| v.split(',').map(|s| s.trim().to_owned()))
        .collect();
    for name in nominated {
        headers.remove(name);
    }
    for name in [
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "proxy-connection",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
        "content-length",
    ] {
        headers.remove(name);
    }
}
