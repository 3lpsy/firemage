use crate::App;
use axum::{
    Router,
    routing::{get, post},
};

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/browser/login", post(super::handlers::login))
        .route("/v1/browser/session", get(super::handlers::session))
        .route("/v1/browser/logout", post(super::handlers::logout))
        .route("/v1/browser/oidc/start", get(super::oidc::start))
        .route("/v1/browser/oidc/callback", get(super::oidc::callback))
}
