use super::{
    relay::validate_input,
    tickets::{ShellState, Ticket},
};
use firemage_guest_protocol::ClientMessage;
use std::time::{Duration, Instant};

#[test]
fn tickets_are_single_use_and_expire() {
    let state = ShellState::default();
    let ticket = |created| Ticket {
        owner: "owner".into(),
        credential: "session".into(),
        vm: "vm".into(),
        created,
    };
    let binding = ticket(Instant::now());
    assert!(binding.is_bound_to("owner", "session", "vm"));
    assert!(!binding.is_bound_to("owner", "another-session", "vm"));
    assert!(!binding.is_bound_to("another-owner", "session", "vm"));
    assert!(!binding.is_bound_to("owner", "session", "another-vm"));
    let token = state.insert(binding).unwrap();
    assert_eq!(state.take(&token).unwrap().credential, "session");
    assert!(state.take(&token).is_none());
    let token = state
        .insert(ticket(Instant::now() - Duration::from_secs(31)))
        .unwrap();
    assert!(state.take(&token).is_none());
    assert!(state.take("invalid").is_none());
}
#[test]
fn browser_cannot_override_command_or_send_unbounded_input() {
    assert!(
        validate_input(&ClientMessage::Open {
            version: 1,
            command: vec!["/bin/sh".into()],
            rows: 24,
            cols: 80
        })
        .is_err()
    );
    assert!(validate_input(&ClientMessage::Resize { rows: 0, cols: 80 }).is_err());
    assert!(
        validate_input(&ClientMessage::Input {
            data: "not base64".into()
        })
        .is_err()
    );
    assert!(
        validate_input(&ClientMessage::Input {
            data: firemage_guest_protocol::encode_data(b"hello\n")
        })
        .is_ok()
    );
    assert!(
        validate_input(&ClientMessage::Resize {
            rows: 40,
            cols: 120
        })
        .is_ok()
    );
}

#[tokio::test]
async fn creation_requires_browser_admin_csrf_ownership_and_enabled_running_vm() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let owner = firemage_queries::add_user(&db, "owner".into(), None, true, None)
        .await
        .unwrap();
    let stranger = firemage_queries::add_user(&db, "stranger".into(), None, true, None)
        .await
        .unwrap();
    let viewer = firemage_queries::add_user(&db, "viewer".into(), None, false, None)
        .await
        .unwrap();
    let config = firemage_config::Server {
        public_url: Some("https://console.example.test".into()),
        ..Default::default()
    };
    let app = crate::App::new(firemage_runtime::Runtime::new(db.clone(), config)).unwrap();
    let spec: firemage_wire::VmSpec =
        serde_json::from_value(serde_json::json!({"name":"test", "web_terminal":{}})).unwrap();
    let vm = firemage_queries::insert_vm(
        &db,
        &owner.id,
        "test",
        serde_json::to_string(&spec).unwrap(),
        "/tmp/absent-shell-api.sock".into(),
    )
    .await
    .unwrap();
    let token = firemage_auth::new_token("session");
    // Session storage is exercised directly so these checks do not depend on password login.
    firemage_queries::insert_credential(
        &db,
        &owner.id,
        firemage_auth::token_hash(&token),
        "session",
        "test",
        firemage_queries::now() + 3600,
    )
    .await
    .unwrap();
    let router = crate::router(app);
    let uri = format!("/v1/vms/{}/shell/sessions", vm.id);
    let request = |token: Option<&str>, csrf: bool| {
        let mut builder = Request::builder()
            .method("POST")
            .uri(&uri)
            .header("origin", "https://console.example.test");
        if let Some(token) = token {
            builder = builder.header("cookie", format!("firemage_session={token}"));
            if csrf {
                builder = builder.header(
                    "x-csrf-token",
                    firemage_auth::token_hash(&format!("firemage-browser-csrf:{token}")),
                );
            }
        }
        builder.body(Body::empty()).unwrap()
    };
    assert_eq!(
        router
            .clone()
            .oneshot(request(None, false))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(Some(&token), false))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = router
        .clone()
        .oneshot(request(Some(&token), true))
        .await
        .unwrap();
    assert!(!response.status().is_success());
    assert!(
        String::from_utf8(to_bytes(response.into_body(), 4096).await.unwrap().to_vec())
            .unwrap()
            .contains("running VM")
    );
    let vm = firemage_queries::set_vm_state(&db, vm, "running", None, None)
        .await
        .unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(request(Some(&token), true))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    for user in [stranger, viewer] {
        let token = firemage_auth::new_token("session");
        firemage_queries::insert_credential(
            &db,
            &user.id,
            firemage_auth::token_hash(&token),
            "session",
            "test",
            firemage_queries::now() + 3600,
        )
        .await
        .unwrap();
        assert_eq!(
            router
                .clone()
                .oneshot(request(Some(&token), true))
                .await
                .unwrap()
                .status()
                .is_success(),
            user.admin
        );
    }
    let mut disabled = spec;
    disabled.web_terminal = None;
    firemage_queries::update_vm_spec(
        &db,
        vm,
        "test".into(),
        serde_json::to_string(&disabled).unwrap(),
    )
    .await
    .unwrap();
    let response = router.oneshot(request(Some(&token), true)).await.unwrap();
    assert!(!response.status().is_success());
}

#[test]
fn ticket_protocol_header_is_unambiguous_and_bounded() {
    let token = firemage_auth::new_token("shell");
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "sec-websocket-protocol",
        format!("firemage-shell, {token}").parse().unwrap(),
    );
    assert_eq!(
        super::routes::protocol_ticket(&headers).unwrap_or_else(|error| panic!("{}", error.1)),
        token
    );
    for value in [
        format!("firemage-shell, {token}, extra"),
        format!("{token}, firemage-shell"),
        "firemage-shell, invalid".into(),
    ] {
        headers.insert("sec-websocket-protocol", value.parse().unwrap());
        assert!(super::routes::protocol_ticket(&headers).is_err());
    }
}
