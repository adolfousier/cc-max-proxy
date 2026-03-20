use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Proxy error types, returned as Anthropic-format error JSON.
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("failed to spawn claude CLI: {0}")]
    CliSpawn(#[from] std::io::Error),

    #[error(
        "claude CLI not found at '{0}' — install via: npm install -g @anthropic-ai/claude-code"
    )]
    CliNotFound(String),

    #[error("claude CLI exited with error: {0}")]
    CliError(String),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, error_type) = match &self {
            ProxyError::CliNotFound(_) => (StatusCode::SERVICE_UNAVAILABLE, "api_error"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "api_error"),
        };

        let body = serde_json::json!({
            "type": "error",
            "error": {
                "type": error_type,
                "message": self.to_string(),
            }
        });

        (status, axum::Json(body)).into_response()
    }
}
