use super::provider::{Attempt, ProviderState};
use axum::{
    Form, Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use base64::Engine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub(super) async fn discovery(State(p): State<ProviderState>) -> Json<Value> {
    Json(
        json!({"issuer":p.issuer,"authorization_endpoint":format!("{}/authorize",p.issuer),"token_endpoint":format!("{}/token",p.issuer),"jwks_uri":format!("{}/keys",p.issuer)}),
    )
}
pub(super) async fn keys() -> Json<Value> {
    Json(
        serde_json::from_str(include_str!("../../fixtures/signing-jwks.json"))
            .expect("fixture JWKS"),
    )
}
pub(super) async fn authorize(
    State(p): State<ProviderState>,
    Query(input): Query<HashMap<String, String>>,
) -> Response {
    if input.get("client_id").map(String::as_str) != Some("browser-client")
        || input.get("redirect_uri") != Some(&p.callback)
        || input.get("code_challenge_method").map(String::as_str) != Some("S256")
        || input.get("response_type").map(String::as_str) != Some("code")
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let (Some(nonce), Some(challenge), Some(state)) = (
        input.get("nonce"),
        input.get("code_challenge"),
        input.get("state"),
    ) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let code = firemage_auth::new_token("code");
    p.attempts.lock().expect("provider state").insert(
        code.clone(),
        Attempt {
            nonce: nonce.clone(),
            challenge: challenge.clone(),
            state: state.clone(),
            subject: None,
        },
    );
    Html(format!(r#"<!doctype html><html><head><title>Fixture identity provider</title></head><body><h1>Fixture identity provider</h1><form method="post" action="/authorize"><input type="hidden" name="code" value="{code}"><label>Username<input id="provider-username" name="username" required></label><label>Password<input id="provider-password" type="password" name="password" required></label><button type="submit">Sign in to provider</button></form></body></html>"#)).into_response()
}
pub(super) async fn login(
    State(p): State<ProviderState>,
    Form(input): Form<HashMap<String, String>>,
) -> Response {
    if input.get("password").map(String::as_str) != Some(crate::PASSWORD) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let mut attempts = p.attempts.lock().expect("provider state");
    let Some(code) = input.get("code") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(attempt) = attempts.get_mut(code) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    attempt.subject = Some(
        if input.get("username").map(String::as_str) == Some("federated") {
            "linked-subject"
        } else {
            "unlinked-subject"
        }
        .into(),
    );
    let mut callback = reqwest::Url::parse(&p.callback).expect("fixture callback");
    callback
        .query_pairs_mut()
        .append_pair("code", code)
        .append_pair("state", &attempt.state);
    *p.last_callback.lock().expect("provider callback") = Some(callback.to_string());
    Redirect::to(callback.as_str()).into_response()
}
pub(super) async fn token(
    State(p): State<ProviderState>,
    headers: HeaderMap,
    Form(input): Form<HashMap<String, String>>,
) -> Response {
    if input.get("grant_type").map(String::as_str) != Some("authorization_code")
        || input.get("redirect_uri") != Some(&p.callback)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let authenticated = if let Some(secret) = &p.client_secret {
        let expected = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("browser-client:{secret}"))
        );
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            == Some(expected.as_str())
            && !input.contains_key("client_id")
    } else {
        input.get("client_id").map(String::as_str) == Some("browser-client")
    };
    if !authenticated {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let (Some(code), Some(verifier)) = (input.get("code"), input.get("code_verifier")) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(attempt) = p.attempts.lock().expect("provider state").remove(code) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(subject) = attempt.subject else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
        != attempt.challenge
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let now = firemage_queries::now();
    let claims = json!({"iss":p.issuer,"sub":subject,"aud":"browser-client","iat":now,"exp":now+60,"nonce":attempt.nonce});
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("test-key".into());
    let key =
        jsonwebtoken::EncodingKey::from_rsa_pem(include_bytes!("../../fixtures/signing-key.pem"))
            .expect("fixture signing key");
    Json(json!({"id_token":jsonwebtoken::encode(&header,&claims,&key).expect("fixture token")}))
        .into_response()
}
