use crate::{App, router};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn app(config: firemage_config::Server) -> App {
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    firemage_queries::add_user(
        &db,
        "operator".into(),
        Some(firemage_auth::hash_password("test-password-123").unwrap()),
        true,
        None,
    )
    .await
    .unwrap();
    App::new(firemage_runtime::Runtime::new(db, config)).unwrap()
}
async fn send(
    router: &Router,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Value,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    router
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}
async fn body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
#[tokio::test]
async fn password_cookie_csrf_and_revocation() {
    let app = app(firemage_config::Server {
        public_url: Some("https://console.example.test".into()),
        ..Default::default()
    })
    .await;
    let router = router(app);
    let input = json!({"username":"operator", "password":"test-password-123"});
    let rejected = send(
        &router,
        "POST",
        "/v1/browser/login",
        &[("origin", "https://evil.example")],
        input.clone(),
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
    let response = send(
        &router,
        "POST",
        "/v1/browser/login",
        &[
            ("origin", "https://console.example.test"),
            ("x-forwarded-proto", "http"),
        ],
        input,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.contains("HttpOnly; SameSite=Strict"));
    assert!(cookie.contains("; Secure"));
    let session = body(response).await;
    assert!(session.get("token").is_none());
    let csrf = session["csrf_token"].as_str().unwrap();
    let cookie = cookie.split(';').next().unwrap();
    let response = send(
        &router,
        "GET",
        "/v1/browser/session",
        &[("cookie", cookie)],
        Value::Null,
    )
    .await;
    assert_eq!(body(response).await["user"]["username"], "operator");
    let input = json!({"name":"browser-token", "expires_at":firemage_queries::now()+60});
    for headers in [
        vec![("cookie", cookie)],
        vec![
            ("cookie", cookie),
            ("origin", "https://evil.example"),
            ("x-csrf-token", csrf),
        ],
    ] {
        assert_eq!(
            send(&router, "POST", "/v1/apitokens", &headers, input.clone())
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    let headers = [
        ("cookie", cookie),
        ("origin", "https://console.example.test"),
        ("x-csrf-token", csrf),
    ];
    let response = send(&router, "POST", "/v1/apitokens", &headers, input).await;
    assert_eq!(response.status(), StatusCode::OK);
    let token = body(response).await["token"].as_str().unwrap().to_string();
    assert_eq!(
        send(
            &router,
            "GET",
            "/v1/me",
            &[("authorization", &format!("Bearer {token}"))],
            Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            &router,
            "GET",
            "/v1/me",
            &[("cookie", &format!("firemage_session={token}"))],
            Value::Null
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&router, "POST", "/v1/browser/logout", &headers, Value::Null)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(&router, "GET", "/v1/me", &[("cookie", cookie)], Value::Null)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn pending_state_is_one_use() {
    let state = super::BrowserState::default();
    state.insert("ticket", "nonce".into(), "verifier".into());
    assert!(state.take("unknown").is_none());
    assert_eq!(state.take("ticket").unwrap().nonce, "nonce");
    assert!(state.take("ticket").is_none());
}

#[tokio::test]
async fn oidc_code_pkce_nonce_binding_and_replay() {
    use axum::{
        Json,
        extract::{Form, State},
        routing::{get, post},
    };
    use base64::Engine;
    use sha2::{Digest, Sha256};
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    #[derive(Clone)]
    struct Provider {
        issuer: String,
        nonce: Arc<Mutex<String>>,
        challenge: Arc<Mutex<String>>,
    }
    async fn exchange(
        State(provider): State<Provider>,
        Form(input): Form<HashMap<String, String>>,
    ) -> Json<Value> {
        assert_eq!(input["grant_type"], "authorization_code");
        assert_eq!(input["client_id"], "browser-client");
        assert_eq!(
            input["redirect_uri"],
            "http://127.0.0.1:8080/v1/browser/oidc/callback"
        );
        assert_eq!(
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(Sha256::digest(input["code_verifier"].as_bytes())),
            *provider.challenge.lock().unwrap()
        );
        let nonce = if input["code"] == "bad-nonce" {
            "wrong".into()
        } else {
            provider.nonce.lock().unwrap().clone()
        };
        let now = firemage_queries::now();
        let mut claims = json!({"iss":provider.issuer,"sub":"stable-subject","aud":"browser-client","iat":now,"exp":now+60,"nonce":nonce});
        match input["code"].as_str() {
            "bad-issuer" => claims["iss"] = json!("https://other.example.test"),
            "bad-audience" => claims["aud"] = json!("other-client"),
            "unknown-subject" => claims["sub"] = json!("unlinked-subject"),
            "expired-token" => claims["exp"] = json!(now - 120),
            "stale-token" => claims["iat"] = json!(now - 600),
            "multiple-audiences" => claims["aud"] = json!(["browser-client", "other-client"]),
            _ => {}
        }
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some("test-key".into());
        let token = jsonwebtoken::encode(
            &header,
            &claims,
            &jsonwebtoken::EncodingKey::from_rsa_pem(include_bytes!("test_key.pem")).unwrap(),
        )
        .unwrap();
        Json(json!({"id_token":token}))
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let provider = Provider {
        issuer: issuer.clone(),
        nonce: Default::default(),
        challenge: Default::default(),
    };
    let mock = Router::new()
        .route("/.well-known/openid-configuration", get(|State(p): State<Provider>| async move { Json(json!({"issuer":p.issuer,"authorization_endpoint":format!("{}/authorize",p.issuer),"token_endpoint":format!("{}/token",p.issuer),"jwks_uri":format!("{}/keys",p.issuer)})) }))
        .route("/keys", get(|| async { Json(serde_json::from_str::<Value>(include_str!("test_jwks.json")).unwrap()) }))
        .route("/token", post(exchange)).with_state(provider.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, mock).await.unwrap();
    });
    let app = app(firemage_config::Server {
        public_url: Some("http://127.0.0.1:8080".into()),
        oidc_issuer: Some(issuer.clone()),
        oidc_client_id: Some("browser-client".into()),
        ..Default::default()
    })
    .await;
    firemage_queries::add_user_with_issuer(
        &app.runtime.db,
        "federated".into(),
        None,
        false,
        Some("stable-subject".into()),
        Some(issuer),
    )
    .await
    .unwrap();
    let router = router(app);
    for code in [
        "bad-nonce",
        "bad-issuer",
        "bad-audience",
        "unknown-subject",
        "expired-token",
        "stale-token",
        "multiple-audiences",
        "good-code",
    ] {
        let start = send(&router, "GET", "/v1/browser/oidc/start", &[], Value::Null).await;
        assert_eq!(start.status(), StatusCode::SEE_OTHER);
        let url = reqwest::Url::parse(start.headers()[header::LOCATION].to_str().unwrap()).unwrap();
        let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params["code_challenge_method"], "S256");
        *provider.nonce.lock().unwrap() = params["nonce"].clone();
        *provider.challenge.lock().unwrap() = params["code_challenge"].clone();
        let cookie = start.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap();
        let path = format!(
            "/v1/browser/oidc/callback?state={}&code={code}",
            params["state"]
        );
        let unbound = send(&router, "GET", &path, &[], Value::Null).await;
        assert_eq!(unbound.headers()[header::LOCATION], "/?login_error=oidc");
        let response = send(&router, "GET", &path, &[("cookie", cookie)], Value::Null).await;
        if code != "good-code" {
            assert_eq!(response.headers()[header::LOCATION], "/?login_error=oidc");
        } else {
            assert_eq!(response.headers()[header::LOCATION], "/");
            let cookie = response
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .find(|value| value.to_str().unwrap().starts_with("firemage_session="))
                .unwrap()
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap();
            let session = body(
                send(
                    &router,
                    "GET",
                    "/v1/browser/session",
                    &[("cookie", cookie)],
                    Value::Null,
                )
                .await,
            )
            .await;
            assert_eq!(session["user"]["username"], "federated");
            assert_eq!(session["user"]["admin"], false);
        }
        assert_eq!(
            send(&router, "GET", &path, &[("cookie", cookie)], Value::Null)
                .await
                .headers()[header::LOCATION],
            "/?login_error=oidc"
        );
    }
    task.abort();
}

#[tokio::test]
async fn invalid_browser_requests_retain_security_headers() {
    let router = router(app(firemage_config::Server::default()).await);
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/browser/login")
                .header("origin", "http://127.0.0.1:8080")
                .header("content-type", "application/json")
                .body(Body::from("{"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(response.headers().contains_key("content-security-policy"));
}

#[tokio::test]
async fn oidc_rejects_invalid_custom_ca_before_contacting_provider() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ca.pem");
    std::fs::write(&path, "not a certificate").unwrap();
    let app = app(firemage_config::Server {
        oidc_issuer: Some("https://identity.example.test".into()),
        oidc_client_id: Some("browser-client".into()),
        oidc_ca_cert: Some(path),
        ..Default::default()
    })
    .await;
    let error = crate::oidc::discovery(&app).await.unwrap_err();
    assert!(format!("{error:#}").contains("invalid OIDC CA certificate"));
}
