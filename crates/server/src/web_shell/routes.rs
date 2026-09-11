use super::tickets::Ticket;
use crate::{
    App,
    error::{Error, Result},
    identity::Identity,
};
use axum::{
    Json,
    extract::{Path, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use firemage_wire::WebShellSession;

pub(crate) async fn create(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<WebShellSession>> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    crate::browser::ensure_origin(&app, &headers)?;
    let row = crate::resources::owned_vm(&app, &identity, &id).await?;
    app.runtime.ensure_web_shell(&row.owner_id, &id).await?;
    let token = app.shell.insert(Ticket {
        owner: identity.user.id,
        credential: identity.credential.id,
        vm: id.clone(),
        created: std::time::Instant::now(),
    })?;
    Ok(Json(WebShellSession {
        url: format!("/v1/vms/{id}/shell/ws"),
        ticket: token,
    }))
}
pub(crate) async fn connect(
    identity: Identity,
    State(app): State<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response> {
    identity.ensure_admin()?;
    identity.ensure_session()?;
    crate::browser::ensure_origin(&app, &headers)?;
    crate::ensure!(
        crate::browser::cookie(&headers, crate::browser::SESSION_COOKIE).is_some()
            && !headers.contains_key("authorization"),
        "Web Shell requires a browser session"
    );
    let token = protocol_ticket(&headers)?;
    let ticket = app.shell.take(token).ok_or_else(|| {
        Error(
            StatusCode::FORBIDDEN,
            "invalid or expired shell ticket".into(),
        )
    })?;
    if !ticket.is_bound_to(&identity.user.id, &identity.credential.id, &id) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "shell ticket does not match this session or VM".into(),
        ));
    }
    let row = crate::resources::owned_vm(&app, &identity, &id).await?;
    app.runtime.ensure_web_shell(&row.owner_id, &id).await?;
    let permit = app
        .shell
        .sessions
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            Error(
                StatusCode::TOO_MANY_REQUESTS,
                "too many active shell sessions".into(),
            )
        })?;
    Ok(ws
        .protocols(["firemage-shell"])
        .max_message_size(firemage_guest_protocol::MAX_FRAME_BYTES)
        .max_frame_size(firemage_guest_protocol::MAX_FRAME_BYTES)
        .on_upgrade(move |socket| super::relay::run(socket, app, row, identity.credential, permit)))
}

pub(super) fn protocol_ticket(headers: &HeaderMap) -> Result<&str> {
    let mut protocols = headers
        .get_all("sec-websocket-protocol")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim);
    let protocol = protocols.next();
    let token = protocols.next();
    if protocol != Some("firemage-shell")
        || protocols.next().is_some()
        || !token.is_some_and(|token| {
            token.len() == 73
                && token.starts_with("fm_shell_")
                && token[9..].bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "shell connection protocols are invalid".into(),
        ));
    }
    Ok(token.expect("validated token"))
}
