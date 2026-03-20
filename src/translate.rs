use crate::types::{
    CliMessage, ContentBlock, SseContentBlockDelta, SseContentBlockStart, SseContentBlockStop,
    SseDelta, SseMessageDelta, SseMessageDeltaInner, SseMessageMeta, SseMessageStart,
    SseMessageStop, SseUsage,
};

/// A translated SSE event: (event_type, json_data).
pub type SseEvent = (String, String);

fn event(event_type: &str, data: &impl serde::Serialize) -> SseEvent {
    let json = serde_json::to_string(data).unwrap_or_default();
    (event_type.to_string(), json)
}

/// Translate a CLI NDJSON message into zero or more Anthropic SSE events.
pub fn translate_cli_message(msg: &CliMessage, state: &mut TranslateState) -> Vec<SseEvent> {
    let mut events = Vec::new();

    match msg {
        CliMessage::System { model } => {
            if let Some(m) = model {
                tracing::debug!("CLI system: model={}", m);
                state.model = m.clone();
            }
        }

        CliMessage::Assistant { message } => {
            if !state.started {
                state.started = true;

                let id = message
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("msg_{}", uuid::Uuid::new_v4().simple()));

                if let Some(m) = &message.model {
                    state.model = m.clone();
                }

                let usage = cli_usage_to_sse(message.usage.as_ref());
                state.message_id = id.clone();

                events.push(event(
                    "message_start",
                    &SseMessageStart {
                        r#type: "message_start",
                        message: SseMessageMeta {
                            id,
                            r#type: "message",
                            role: "assistant",
                            model: state.model.clone(),
                            content: vec![],
                            stop_reason: None,
                            usage,
                        },
                    },
                ));
            }

            for block in &message.content {
                let index = state.block_index;

                match block {
                    ContentBlock::Text { text } => {
                        tracing::debug!("Block {}: text ({} chars)", index, text.len());
                        events.push(event(
                            "content_block_start",
                            &SseContentBlockStart {
                                r#type: "content_block_start",
                                index,
                                content_block: ContentBlock::Text {
                                    text: String::new(),
                                },
                            },
                        ));

                        events.push(event(
                            "content_block_delta",
                            &SseContentBlockDelta {
                                r#type: "content_block_delta",
                                index,
                                delta: SseDelta::TextDelta { text: text.clone() },
                            },
                        ));

                        events.push(event(
                            "content_block_stop",
                            &SseContentBlockStop {
                                r#type: "content_block_stop",
                                index,
                            },
                        ));

                        state.block_index += 1;
                    }

                    ContentBlock::ToolUse { id, name, input } => {
                        tracing::debug!("Block {}: tool_use name={} id={}", index, name, id);
                        events.push(event(
                            "content_block_start",
                            &SseContentBlockStart {
                                r#type: "content_block_start",
                                index,
                                content_block: ContentBlock::ToolUse {
                                    id: id.clone(),
                                    name: name.clone(),
                                    input: input.clone(),
                                },
                            },
                        ));

                        events.push(event(
                            "content_block_stop",
                            &SseContentBlockStop {
                                r#type: "content_block_stop",
                                index,
                            },
                        ));

                        state.block_index += 1;
                    }

                    ContentBlock::Thinking { thinking } => {
                        tracing::debug!("Block {}: thinking ({} chars)", index, thinking.len());
                        events.push(event(
                            "content_block_start",
                            &SseContentBlockStart {
                                r#type: "content_block_start",
                                index,
                                content_block: ContentBlock::Thinking {
                                    thinking: String::new(),
                                },
                            },
                        ));

                        events.push(event(
                            "content_block_delta",
                            &SseContentBlockDelta {
                                r#type: "content_block_delta",
                                index,
                                delta: SseDelta::ReasoningDelta {
                                    text: thinking.clone(),
                                },
                            },
                        ));

                        events.push(event(
                            "content_block_stop",
                            &SseContentBlockStop {
                                r#type: "content_block_stop",
                                index,
                            },
                        ));

                        state.block_index += 1;
                    }

                    _ => {}
                }
            }

            if let Some(u) = &message.usage {
                state.output_tokens += u.output_tokens;
            }
        }

        CliMessage::Result { stop_reason, usage } => {
            let reason = stop_reason
                .clone()
                .unwrap_or_else(|| "end_turn".to_string());
            let final_usage = usage
                .as_ref()
                .map(|u| SseUsage {
                    input_tokens: u.input_tokens,
                    output_tokens: u.output_tokens,
                    cache_read_input_tokens: Some(u.cache_read_input_tokens),
                    cache_creation_input_tokens: Some(u.cache_creation_input_tokens),
                })
                .unwrap_or(SseUsage {
                    input_tokens: 0,
                    output_tokens: state.output_tokens,
                    cache_read_input_tokens: None,
                    cache_creation_input_tokens: None,
                });

            events.push(event(
                "message_delta",
                &SseMessageDelta {
                    r#type: "message_delta",
                    delta: SseMessageDeltaInner {
                        stop_reason: reason,
                        stop_sequence: None,
                    },
                    usage: final_usage,
                },
            ));

            events.push(event(
                "message_stop",
                &SseMessageStop {
                    r#type: "message_stop",
                },
            ));
        }

        CliMessage::RateLimitEvent {} => {}
    }

    events
}

fn cli_usage_to_sse(usage: Option<&crate::types::CliUsage>) -> SseUsage {
    usage
        .map(|u| SseUsage {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
            cache_read_input_tokens: Some(u.cache_read_input_tokens),
            cache_creation_input_tokens: Some(u.cache_creation_input_tokens),
        })
        .unwrap_or(SseUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
        })
}

/// Tracks state across a single streaming response.
pub struct TranslateState {
    pub started: bool,
    pub message_id: String,
    pub model: String,
    pub block_index: usize,
    pub output_tokens: u32,
}

impl TranslateState {
    pub fn new() -> Self {
        Self {
            started: false,
            message_id: String::new(),
            model: String::new(),
            block_index: 0,
            output_tokens: 0,
        }
    }
}
