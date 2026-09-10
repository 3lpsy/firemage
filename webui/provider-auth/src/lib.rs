//! Browser session state. No bearer credentials are persisted by the UI.
use dioxus::prelude::*;
use firemage_webui_provider_api::{get, request};
use serde_json::{Value, json};
#[derive(Clone, Copy)]
pub struct Auth {
    pub session: Signal<Value>,
    pub ready: Signal<bool>,
    pub error: Signal<String>,
}
impl Auth {
    pub fn is_admin(self) -> bool {
        self.session.read()["user"]["admin"]
            .as_bool()
            .unwrap_or(false)
    }
    pub fn is_logged_in(self) -> bool {
        self.session.read()["user"].is_object()
    }
    pub fn csrf(self) -> String {
        self.session.read()["csrf_token"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }
    pub async fn login(mut self, username: String, password: String) -> Result<(), String> {
        let session = request(
            "POST",
            "/v1/browser/login",
            Some(json!({ "username" : username, "password" : password })),
            "",
        )
        .await?;
        self.session.set(session);
        Ok(())
    }
    pub async fn logout(mut self) -> Result<(), String> {
        let session = request("POST", "/v1/browser/logout", Some(json!({})), &self.csrf()).await?;
        self.session.set(session);
        Ok(())
    }
}
pub fn use_auth() -> Auth {
    use_context()
}
pub fn use_auth_provider() -> Auth {
    let mut auth = Auth {
        session: use_signal(|| Value::Null),
        ready: use_signal(|| false),
        error: use_signal(String::new),
    };
    use_context_provider(|| auth);
    let _listener = use_hook(move || {
        std::rc::Rc::new(firemage_webui_provider_api::on_session_expired(move || {
            let mut session = auth.session.peek().clone();
            session["user"] = Value::Null;
            session["csrf_token"] = Value::Null;
            session["session_expired"] = Value::Bool(true);
            auth.session.set(session);
        }))
    });
    use_future(move || async move {
        match get("/v1/browser/session").await {
            Ok(session) => auth.session.set(session),
            Err(error) => auth.error.set(error),
        }
        auth.ready.set(true);
        loop {
            gloo_timers::future::TimeoutFuture::new(60_000).await;
            if let Ok(mut session) = get("/v1/browser/session").await {
                if auth.is_logged_in() && session["user"].is_null() {
                    session["session_expired"] = Value::Bool(true);
                }
                auth.session.set(session);
            }
        }
    });
    auth
}

pub use firemage_webui_provider_api::is_oidc_failure;
