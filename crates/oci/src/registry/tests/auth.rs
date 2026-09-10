use super::fixtures::{Server, digest};
use crate::RegistryCredentials;
use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn basic_credentials_are_only_sent_after_challenge() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = seen.clone();
    let server = Server::start(Router::new().fallback(get(move |headers: HeaderMap| {
        let captured = captured.clone();
        async move {
            let auth = headers
                .get("authorization")
                .map(|v| v.to_str().unwrap().to_owned());
            captured.lock().unwrap().push(auth.clone());
            if auth.as_deref() == Some("Basic cm9ib3Q6c2VjcmV0") {
                (StatusCode::OK, "ok").into_response()
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    [("www-authenticate", "Basic realm=\"registry\"")],
                    "",
                )
                    .into_response()
            }
        }
    })))
    .await;
    let mut registry = server.registry(
        &digest(b"ok"),
        RegistryCredentials::Basic {
            username: "robot".into(),
            password: "secret".into(),
        },
    );
    registry
        .get(registry.origin.join("v2/").unwrap(), false)
        .await
        .unwrap();
    assert_eq!(
        *seen.lock().unwrap(),
        vec![None, Some("Basic cm9ib3Q6c2VjcmV0".into())]
    );
}

#[tokio::test]
async fn token_exchange_limits_scope_and_uses_the_trusted_realm() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let capture = seen.clone();
    let token_server = Server::start(Router::new().fallback(get(
        move |headers: HeaderMap, axum::extract::RawQuery(query): axum::extract::RawQuery| {
            let capture = capture.clone();
            async move {
                capture.lock().unwrap().push((
                    headers
                        .get("authorization")
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    query.unwrap(),
                ));
                axum::Json(serde_json::json!({"access_token":"pull-token"}))
            }
        },
    )))
    .await;
    let realm = format!("{}/token", token_server.origin);
    let challenge =
        format!("Bearer realm=\"{realm}\",service=\"registry\",scope=\"repository:other:push\"");
    let server = Server::start(Router::new().fallback(get(move |headers: HeaderMap| {
        let challenge = challenge.clone();
        async move {
            if headers
                .get("authorization")
                .is_some_and(|v| v == "Bearer pull-token")
            {
                (StatusCode::OK, "ok").into_response()
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    [("www-authenticate", challenge)],
                    "",
                )
                    .into_response()
            }
        }
    })))
    .await;
    let mut options = crate::RegistryOptions {
        credentials: RegistryCredentials::Basic {
            username: "robot".into(),
            password: "secret".into(),
        },
        ca_pem: Some(format!("{}{}", server.pem, token_server.pem)),
        token_realm: None,
    };
    let image = server.image(&digest(b"ok"));
    let mut registry = super::super::Registry::new(&image, &options).unwrap();
    let error = registry
        .get(registry.origin.join("v2/").unwrap(), false)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("another origin"));
    assert!(seen.lock().unwrap().is_empty());
    options.token_realm = Some(realm);
    let mut registry = super::super::Registry::new(&image, &options).unwrap();
    registry
        .get(registry.origin.join("v2/").unwrap(), false)
        .await
        .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, "Basic cm9ib3Q6c2VjcmV0");
    let params: std::collections::HashMap<_, _> = url::form_urlencoded::parse(seen[0].1.as_bytes())
        .into_owned()
        .collect();
    assert_eq!(params["scope"], "repository:test/image:pull");
    assert_eq!(params["service"], "registry");
}

#[tokio::test]
async fn cdn_redirects_do_not_receive_registry_credentials() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let capture = seen.clone();
    let cdn = Server::start(Router::new().fallback(get(move |headers: HeaderMap| {
        let capture = capture.clone();
        async move {
            capture
                .lock()
                .unwrap()
                .push(headers.contains_key("authorization"));
            "layer"
        }
    })))
    .await;
    let target = format!("{}/signed?signature=secret", cdn.origin);
    let server = Server::start(Router::new().fallback(get(move || {
        let target = target.clone();
        async move { (StatusCode::TEMPORARY_REDIRECT, [("location", target)]) }
    })))
    .await;
    let mut registry = super::super::Registry::new(
        &server.image(&digest(b"layer")),
        &crate::RegistryOptions {
            credentials: RegistryCredentials::Bearer {
                token: "registry-secret".into(),
            },
            ca_pem: Some(format!("{}{}", server.pem, cdn.pem)),
            ..Default::default()
        },
    )
    .unwrap();
    let url = registry.origin.join("v2/").unwrap();
    assert!(registry.get(url.clone(), false).await.is_err());
    let response = registry.get(url, true).await.unwrap();
    assert_eq!(response.text().await.unwrap(), "layer");
    assert_eq!(*seen.lock().unwrap(), vec![false]);
}

#[tokio::test]
async fn authentication_error_bodies_never_escape() {
    let server = Server::start(Router::new().fallback(get(|| async {
        (StatusCode::FORBIDDEN, "sensitive-reflected-token")
    })))
    .await;
    let mut registry = server.registry(&digest(b"x"), RegistryCredentials::Anonymous);
    let error = registry
        .get(registry.origin.clone(), false)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("403"));
    assert!(!error.contains("sensitive"));
}
