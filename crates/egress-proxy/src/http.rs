use crate::{authority, ca::Authority, forwarding};
use bytes::Bytes;
use firemage_egress_policy::{HttpProxy, HttpScheme};
use firemage_request_signing::SecretResolver;
use http_body_util::{BodyExt, Full};
use hyper::{
    Request, Response, StatusCode, body::Incoming, server::conn::http1, service::service_fn,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use std::{convert::Infallible, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_util::sync::CancellationToken;

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub(crate) type Body = http_body_util::combinators::BoxBody<Bytes, BoxError>;

pub(crate) fn response(status: StatusCode, body: impl Into<Bytes>) -> Response<Body> {
    let mut response = Response::new(
        Full::new(body.into())
            .map_err(|never| match never {})
            .boxed(),
    );
    *response.status_mut() = status;
    response
}

pub(crate) struct Context {
    pub _permit: tokio::sync::OwnedSemaphorePermit,
    pub policy: Arc<HttpProxy>,
    pub ca: Arc<Authority>,
    pub cancel: CancellationToken,
    pub tasks: tokio_util::task::TaskTracker,
    pub secrets: Arc<dyn SecretResolver>,
    pub upstream: Option<firemage_egress_policy::UpstreamProxy>,
}

pub(crate) async fn serve(stream: TcpStream, context: Arc<Context>) {
    let cancel = context.cancel.clone();
    let service = service_fn(move |request| handle(request, context.clone()));
    tokio::select! {
        _ = cancel.cancelled() => {},
        _ = http1::Builder::new().timer(TokioTimer::new()).header_read_timeout(std::time::Duration::from_secs(10)).max_buf_size(32768).serve_connection(TokioIo::new(stream),service).with_upgrades() => {}
    }
}

async fn handle(
    mut request: Request<Incoming>,
    context: Arc<Context>,
) -> Result<Response<Body>, Infallible> {
    if request.method() != "CONNECT" {
        return Ok(forward_or_deny(request, context, None).await);
    }
    let checked = (|| -> anyhow::Result<_> {
        anyhow::ensure!(
            request.uri().scheme().is_none() && request.uri().path_and_query().is_none(),
            "invalid CONNECT target"
        );
        let value = request
            .uri()
            .authority()
            .ok_or_else(|| anyhow::anyhow!("CONNECT authority required"))?;
        anyhow::ensure!(value.port_u16().is_some(), "CONNECT requires explicit port");
        let target = authority::parse(value.as_str(), 443)?;
        authority::ensure_host(&request, &target, 443)?;
        anyhow::ensure!(
            !request.headers().contains_key("transfer-encoding")
                && request
                    .headers()
                    .get("content-length")
                    .is_none_or(|v| v == "0"),
            "CONNECT body denied"
        );
        anyhow::ensure!(
            context
                .policy
                .rules
                .iter()
                .any(|r| r.scheme == HttpScheme::Https
                    && r.host.eq_ignore_ascii_case(&target.0)
                    && r.port == target.1),
            "CONNECT destination denied"
        );
        let acceptor = context.ca.acceptor(&target.0)?;
        Ok((target, acceptor))
    })();
    let Ok((target, acceptor)) = checked else {
        return Ok(response(StatusCode::FORBIDDEN, "CONNECT denied\n"));
    };
    let upgrade = hyper::upgrade::on(&mut request);
    context.tasks.clone().spawn(async move {
        let cancel = context.cancel.clone();
        let work = async {
            let stream = TokioIo::new(upgrade.await?);
            let tls =
                tokio::time::timeout(std::time::Duration::from_secs(15), acceptor.accept(stream))
                    .await??;
            if tls
                .get_ref()
                .1
                .server_name()
                .is_some_and(|name| !name.eq_ignore_ascii_case(&target.0))
            {
                anyhow::bail!("TLS SNI mismatch");
            }
            serve_intercepted(tls, context, target).await;
            Ok::<_, anyhow::Error>(())
        };
        tokio::select! { _ = cancel.cancelled() => {}, _ = work => {} }
    });
    Ok(response(StatusCode::OK, Bytes::new()))
}

async fn serve_intercepted<I: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    stream: I,
    context: Arc<Context>,
    target: (String, u16),
) {
    let service = service_fn(move |request| {
        let context = context.clone();
        let target = target.clone();
        async move { Ok::<_, Infallible>(forward_or_deny(request, context, Some(target)).await) }
    });
    let _ = http1::Builder::new()
        .timer(TokioTimer::new())
        .header_read_timeout(std::time::Duration::from_secs(10))
        .max_buf_size(32768)
        .serve_connection(TokioIo::new(stream), service)
        .await;
}

async fn forward_or_deny(
    request: Request<Incoming>,
    context: Arc<Context>,
    target: Option<(String, u16)>,
) -> Response<Body> {
    match forwarding::forward(request, context, target).await {
        Ok(response) => response,
        Err(_) => response(
            StatusCode::FORBIDDEN,
            "Request denied or upstream unavailable\n",
        ),
    }
}
