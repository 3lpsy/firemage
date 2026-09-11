use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
pub struct Error(pub StatusCode, pub String);
impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        Self(
            if error.is::<firemage_config::RevisionConflict>()
                || error.is::<firemage_queries::CatalogRevisionConflict>()
                || error.is::<firemage_queries::CatalogInUse>()
            {
                StatusCode::CONFLICT
            } else if error.is::<firemage_queries::NotFound>() {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_REQUEST
            },
            error.to_string(),
        )
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({"error":self.1}))).into_response()
    }
}
pub type Result<T> = std::result::Result<T, Error>;

// Keep extractor rejections and unknown routes consistent with API errors.
pub async fn json_errors(response: Response) -> Response {
    if !response.status().is_client_error() && !response.status().is_server_error()
        || response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .is_some_and(|value| {
                value
                    .to_str()
                    .is_ok_and(|value| value.starts_with("application/json"))
            })
    {
        return response;
    }
    let (parts, body) = response.into_parts();
    let status = parts.status;
    let bytes = axum::body::to_bytes(body, 65536).await.unwrap_or_default();
    let message = if bytes.is_empty() {
        status.canonical_reason().unwrap_or("request failed").into()
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    let mut result = Error(status, message).into_response();
    for (name, value) in &parts.headers {
        if name != axum::http::header::CONTENT_TYPE && name != axum::http::header::CONTENT_LENGTH {
            result.headers_mut().append(name, value.clone());
        }
    }
    result
}
