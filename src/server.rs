use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::StreamExt;
use futures::stream;
use tokio::sync::Semaphore;
use tower_http::cors::CorsLayer;

use crate::error::ProxyError;
use crate::translate::{TranslateState, translate_cli_message};
use crate::types::{CliMessage, ContentBlock, MessagesRequest, MessagesResponse, SseUsage};

/// Shared application state holding the CLI path and concurrency limiter.
pub struct AppState {
    pub claude_path: String,
    pub semaphore: Semaphore,
}

/// Build the Axum router with `/v1/messages` and health endpoints.
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/messages", post(handle_messages))
        .route("/", get(handle_health))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn handle_health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "cc-max-proxy-rs",
        "format": "anthropic",
        "endpoints": ["/v1/messages"],
    }))
}

async fn handle_messages(
    State(state): State<Arc<AppState>>,
    Json(request): Json<MessagesRequest>,
) -> Result<Response, ProxyError> {
    tracing::info!(
        "POST /v1/messages — model={}, stream={}, messages={}",
        request.model,
        request.stream,
        request.messages.len()
    );

    let _permit = state
        .semaphore
        .acquire()
        .await
        .map_err(|e| ProxyError::CliError(format!("semaphore closed: {e}")))?;

    let rx = crate::claude_cli::spawn_stream(&state.claude_path, &request).await?;

    let model = request.model.clone();
    if request.stream {
        Ok(stream_response(rx, model).into_response())
    } else {
        Ok(non_stream_response(rx, &model).await?.into_response())
    }
}

fn stream_response(
    rx: tokio::sync::mpsc::Receiver<Result<CliMessage, ProxyError>>,
    model: String,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let event_stream = stream::unfold(
        (rx, TranslateState::new(), model),
        |(mut rx, mut state, model)| async move {
            if state.model.is_empty() {
                state.model.clone_from(&model);
            }

            loop {
                match rx.recv().await {
                    Some(Ok(cli_msg)) => {
                        let sse_events = translate_cli_message(&cli_msg, &mut state);
                        if sse_events.is_empty() {
                            continue;
                        }

                        let axum_events: Vec<Result<Event, std::convert::Infallible>> = sse_events
                            .into_iter()
                            .map(|(event_type, json_data)| {
                                Ok(Event::default().event(event_type).data(json_data))
                            })
                            .collect();

                        return Some((stream::iter(axum_events), (rx, state, model)));
                    }
                    Some(Err(e)) => {
                        let err_json = serde_json::json!({
                            "type": "error",
                            "error": {"type": "api_error", "message": e.to_string()}
                        });
                        let event = Ok(Event::default()
                            .event("error")
                            .data(serde_json::to_string(&err_json).unwrap_or_default()));
                        return Some((stream::iter(vec![event]), (rx, state, model)));
                    }
                    None => return None,
                }
            }
        },
    );

    Sse::new(event_stream.flatten()).keep_alive(KeepAlive::default())
}

async fn non_stream_response(
    mut rx: tokio::sync::mpsc::Receiver<Result<CliMessage, ProxyError>>,
    model: &str,
) -> Result<Json<MessagesResponse>, ProxyError> {
    let mut content_blocks: Vec<ContentBlock> = Vec::new();
    let mut stop_reason = "end_turn".to_string();
    let mut message_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
    let mut final_model = model.to_string();
    let mut usage = SseUsage {
        input_tokens: 0,
        output_tokens: 0,
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
    };

    while let Some(msg_result) = rx.recv().await {
        let msg = msg_result?;
        match msg {
            CliMessage::Assistant { message } => {
                if let Some(id) = message.id {
                    message_id = id;
                }
                if let Some(m) = message.model {
                    final_model = m;
                }
                for block in message.content {
                    content_blocks.push(block);
                }
                if let Some(u) = message.usage {
                    usage.input_tokens = u.input_tokens;
                    usage.output_tokens += u.output_tokens;
                    usage.cache_read_input_tokens = Some(u.cache_read_input_tokens);
                    usage.cache_creation_input_tokens = Some(u.cache_creation_input_tokens);
                }
            }
            CliMessage::Result {
                stop_reason: sr,
                usage: ru,
            } => {
                if let Some(r) = sr {
                    stop_reason = r;
                }
                if let Some(u) = ru {
                    usage.input_tokens = u.input_tokens;
                    usage.output_tokens = u.output_tokens;
                }
            }
            _ => {}
        }
    }

    Ok(Json(MessagesResponse {
        id: message_id,
        r#type: "message",
        role: "assistant",
        model: final_model,
        content: content_blocks,
        stop_reason,
        usage,
    }))
}
