//! Same-origin JSON transport. Authentication stays in HttpOnly cookies.
use serde_json::Value;
pub async fn request(
    method: &str,
    path: &str,
    body: Option<Value>,
    csrf: &str,
) -> Result<Value, String> {
    if !path.starts_with("/v1/") || path.starts_with("//") {
        return Err("Invalid API path".into());
    }
    let method = method.parse().map_err(|_| "Invalid HTTP method")?;
    let mut builder = gloo_net::http::RequestBuilder::new(path)
        .method(method)
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .header("Accept", "application/json");
    if !csrf.is_empty() {
        builder = builder.header("X-CSRF-Token", csrf);
    }
    let request = match body {
        Some(body) => builder.json(&body).map_err(|e| e.to_string())?,
        None => builder.build().map_err(|e| e.to_string())?,
    };
    let response = request.send().await.map_err(|e| e.to_string())?;
    response_value(response, path).await
}

pub async fn upload(path: &str, bytes: &[u8], csrf: &str) -> Result<Value, String> {
    upload_bytes("PUT", path, bytes, csrf).await
}

pub async fn upload_post(path: &str, bytes: &[u8], csrf: &str) -> Result<Value, String> {
    upload_bytes("POST", path, bytes, csrf).await
}

/// Send a browser-backed file without copying its bytes into WASM memory.
pub async fn upload_blob(path: &str, blob: &web_sys::Blob, csrf: &str) -> Result<Value, String> {
    if !path.starts_with("/v1/") || path.starts_with("//") {
        return Err("Invalid API path".into());
    }
    let response = gloo_net::http::RequestBuilder::new(path)
        .method(gloo_net::http::Method::POST)
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .header("Accept", "application/json")
        .header("Content-Type", "application/octet-stream")
        .header("X-CSRF-Token", csrf)
        .body(blob.clone())
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?;
    response_value(response, path).await
}

async fn upload_bytes(method: &str, path: &str, bytes: &[u8], csrf: &str) -> Result<Value, String> {
    if !path.starts_with("/v1/") || path.starts_with("//") {
        return Err("Invalid API path".into());
    }
    let response = gloo_net::http::RequestBuilder::new(path)
        .method(method.parse().map_err(|_| "Invalid HTTP method")?)
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .header("Accept", "application/json")
        .header("Content-Type", "application/octet-stream")
        .header("X-CSRF-Token", csrf)
        .body(js_sys::Uint8Array::from(bytes))
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    response_value(response, path).await
}

async fn response_value(response: gloo_net::http::Response, path: &str) -> Result<Value, String> {
    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if status == 401 && path != "/v1/browser/login" {
        notify_session_expired();
    }
    if !(200..300).contains(&status) {
        let reason = value["error"]
            .as_str()
            .or(value["message"].as_str())
            .unwrap_or("Request failed");
        return Err(format!("{reason} (HTTP {status})"));
    }
    Ok(value)
}
pub async fn get(path: &str) -> Result<Value, String> {
    request("GET", path, None, "").await
}
pub fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
pub fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}
pub fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

pub fn is_oidc_failure() -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .is_some_and(|query| query.contains("login_error=oidc"))
}

pub fn timestamp(value: &Value) -> String {
    let Some(seconds) = value.as_i64() else {
        return value.as_str().unwrap_or_default().to_owned();
    };
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(seconds as f64 * 1000.0))
            .to_iso_string()
            .as_string()
            .unwrap_or_default()
            .replace('T', " ")
            .replace(".000Z", " UTC")
    }
    #[cfg(not(target_arch = "wasm32"))]
    seconds.to_string()
}

pub fn notify_session_expired() {
    if let (Some(window), Ok(event)) = (
        web_sys::window(),
        web_sys::Event::new("firemage-session-expired"),
    ) {
        let _ = window.dispatch_event(&event);
    }
}

pub struct SessionListener(wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>);
impl Drop for SessionListener {
    fn drop(&mut self) {
        use wasm_bindgen::JsCast;
        if let Some(window) = web_sys::window() {
            let _ = window.remove_event_listener_with_callback(
                "firemage-session-expired",
                self.0.as_ref().unchecked_ref(),
            );
        }
    }
}
pub fn on_session_expired(mut callback: impl FnMut() + 'static) -> SessionListener {
    use wasm_bindgen::JsCast;
    let listener =
        wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(move |_| callback());
    if let Some(window) = web_sys::window() {
        let _ = window.add_event_listener_with_callback(
            "firemage-session-expired",
            listener.as_ref().unchecked_ref(),
        );
    }
    SessionListener(listener)
}
