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
///
/// With `--include-partial-messages`, each `assistant` line contains the FULL
/// accumulated content so far. We diff against what we already emitted to avoid
/// sending duplicate blocks/text to the client.
pub fn translate_cli_message(msg: &CliMessage, state: &mut TranslateState) -> Vec<SseEvent> {
    let mut events = Vec::new();

    match msg {
        CliMessage::System { model } => {
            if let Some(m) = model {
                tracing::debug!("CLI system: model={}", m);
                state.model = m.clone();
            }
        }

        CliMessage::StreamEvent { event } => {
            // The CLI wraps real Anthropic SSE events in stream_event.
            // Extract the inner event type and forward the JSON directly.
            if let Some(event_type) = event.get("type").and_then(|t| t.as_str()) {
                let json = serde_json::to_string(event).unwrap_or_default();
                tracing::debug!("StreamEvent → {}", event_type);

                state.streaming_via_events = true;

                // Track state from forwarded events
                if event_type == "message_start" {
                    state.started = true;
                }
                if event_type == "message_stop" {
                    state.got_stop = true;
                }

                events.push((event_type.to_string(), json));
            }
        }

        CliMessage::Assistant { message } => {
            // With stream_event support, assistant messages are redundant
            // (they contain accumulated content we already streamed).
            // Only use them as fallback if no stream_events arrived.
            if state.streaming_via_events {
                if let Some(u) = &message.usage {
                    state.output_tokens = u.output_tokens;
                }
                return events;
            }

            // Fallback: emit message_start once
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

            let num_blocks = message.content.len();

            for (i, block) in message.content.iter().enumerate() {
                // Blocks before the last are "complete" — their content won't change.
                // The last block may still be growing (partial streaming).
                let is_last = i == num_blocks - 1;

                if i < state.completed_blocks {
                    // Already fully emitted (start + deltas + stop). Skip.
                    continue;
                }

                if i > state.completed_blocks {
                    // A new block appeared after the current one — the previous
                    // current block is now complete. Close it first.
                    if state.current_block_started {
                        events.push(event(
                            "content_block_stop",
                            &SseContentBlockStop {
                                r#type: "content_block_stop",
                                index: state.completed_blocks,
                            },
                        ));
                        state.completed_blocks += 1;
                        state.current_block_chars = 0;
                        state.current_block_started = false;
                    }

                    // If there are multiple new blocks, emit intermediate ones fully
                    while state.completed_blocks < i {
                        let intermediate = &message.content[state.completed_blocks];
                        emit_full_block(intermediate, state.completed_blocks, &mut events);
                        state.completed_blocks += 1;
                    }
                }

                // Now i == state.completed_blocks (or we're at the current partial block)
                let full_text = block_text(block);

                if !state.current_block_started {
                    // Start this block
                    events.push(event(
                        "content_block_start",
                        &SseContentBlockStart {
                            r#type: "content_block_start",
                            index: i,
                            content_block: empty_block(block),
                        },
                    ));
                    state.current_block_started = true;
                    state.current_block_chars = 0;
                }

                // Emit delta for new characters only
                if full_text.len() > state.current_block_chars {
                    let new_text = &full_text[state.current_block_chars..];
                    if !new_text.is_empty() {
                        events.push(event(
                            "content_block_delta",
                            &SseContentBlockDelta {
                                r#type: "content_block_delta",
                                index: i,
                                delta: block_delta(block, new_text),
                            },
                        ));
                        state.current_block_chars = full_text.len();
                    }
                }

                // If this is NOT the last block, it's complete — close it
                if !is_last {
                    events.push(event(
                        "content_block_stop",
                        &SseContentBlockStop {
                            r#type: "content_block_stop",
                            index: i,
                        },
                    ));
                    state.completed_blocks += 1;
                    state.current_block_chars = 0;
                    state.current_block_started = false;
                }
            }

            if let Some(u) = &message.usage {
                state.output_tokens = u.output_tokens; // replace, not accumulate
            }
        }

        CliMessage::Result { stop_reason, usage } => {
            // If stream_event already sent message_delta + message_stop, skip.
            if state.got_stop {
                tracing::debug!("Result received but stream_event already sent message_stop");
                return events;
            }

            // Close any open block (fallback path)
            if state.current_block_started {
                events.push(event(
                    "content_block_stop",
                    &SseContentBlockStop {
                        r#type: "content_block_stop",
                        index: state.completed_blocks,
                    },
                ));
                state.completed_blocks += 1;
                state.current_block_started = false;
            }

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

            tracing::debug!(
                "Result: stop_reason={}, output_tokens={}",
                reason,
                final_usage.output_tokens
            );

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

/// Extract text content from a block.
fn block_text(block: &ContentBlock) -> &str {
    match block {
        ContentBlock::Text { text } => text.as_str(),
        ContentBlock::Thinking { thinking } => thinking.as_str(),
        _ => "",
    }
}

/// Create an empty version of a block for content_block_start.
fn empty_block(block: &ContentBlock) -> ContentBlock {
    match block {
        ContentBlock::Text { .. } => ContentBlock::Text {
            text: String::new(),
        },
        ContentBlock::Thinking { .. } => ContentBlock::Thinking {
            thinking: String::new(),
        },
        ContentBlock::ToolUse { id, name, .. } => ContentBlock::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: serde_json::Value::Object(Default::default()),
        },
        other => other.clone(),
    }
}

/// Create the appropriate delta for a block type.
fn block_delta(block: &ContentBlock, new_text: &str) -> SseDelta {
    match block {
        ContentBlock::Thinking { .. } => SseDelta::ReasoningDelta {
            text: new_text.to_string(),
        },
        _ => SseDelta::TextDelta {
            text: new_text.to_string(),
        },
    }
}

/// Emit a complete block (start + delta + stop) in one shot.
fn emit_full_block(block: &ContentBlock, index: usize, events: &mut Vec<SseEvent>) {
    match block {
        ContentBlock::Text { text } => {
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
        }
        ContentBlock::Thinking { thinking } => {
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
        }
        ContentBlock::ToolUse { id, name, input } => {
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
        }
        _ => {}
    }
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
///
/// Handles incremental diffing for `--include-partial-messages` where each
/// `assistant` NDJSON line contains the full accumulated content.
pub struct TranslateState {
    pub started: bool,
    pub message_id: String,
    pub model: String,
    /// Number of blocks fully emitted (start + stop sent).
    pub completed_blocks: usize,
    /// Whether we've sent content_block_start for the current (incomplete) block.
    pub current_block_started: bool,
    /// Number of characters already sent for the current block.
    pub current_block_chars: usize,
    pub output_tokens: u32,
    /// Whether we're receiving real-time stream_event messages from CLI.
    pub streaming_via_events: bool,
    /// Whether message_stop was already forwarded via stream_event.
    pub got_stop: bool,
}

impl TranslateState {
    pub fn new() -> Self {
        Self {
            started: false,
            message_id: String::new(),
            model: String::new(),
            completed_blocks: 0,
            current_block_started: false,
            current_block_chars: 0,
            output_tokens: 0,
            streaming_via_events: false,
            got_stop: false,
        }
    }
}
