use crate::App;
use axum::{Router, routing::get};

pub fn routes() -> Router<App> {
    Router::new()
        .route(
            "/v1/egress/policies",
            get(super::policies::list).post(super::policies::create),
        )
        .route(
            "/v1/egress/policies/{id}",
            get(super::policies::get)
                .put(super::policies::update)
                .delete(super::policies::delete),
        )
        .route(
            "/v1/egress/proxies",
            get(super::proxies::list).post(super::proxies::create),
        )
        .route(
            "/v1/egress/proxies/{id}",
            get(super::proxies::get)
                .put(super::proxies::update)
                .delete(super::proxies::delete),
        )
        .route(
            "/v1/vms/{id}/egress-policy",
            axum::routing::put(super::policies::assign),
        )
}
