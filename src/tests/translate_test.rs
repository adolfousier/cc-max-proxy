use crate::translate::{TranslateState, translate_cli_message};
use crate::types::{CliAssistantMessage, CliMessage, CliUsage, ContentBlock};

fn make_state() -> TranslateState {
    TranslateState::new()
}

fn text_assistant(text: &str) -> CliMessage {
    CliMessage::Assistant {
        message: CliAssistantMessage {
            id: Some("msg_test".to_string()),
            model: Some("claude-sonnet-4-6".to_string()),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            usage: Some(CliUsage {
                input_tokens: 10,
                output_tokens: 5,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            }),
        },
    }
}

#[test]
fn system_message_sets_model() {
    let mut state = make_state();
    let msg = CliMessage::System {
        model: Some("claude-opus-4-6".to_string()),
    };

    let events = translate_cli_message(&msg, &mut state);
    assert!(events.is_empty());
    assert_eq!(state.model, "claude-opus-4-6");
}

#[test]
fn first_assistant_emits_message_start() {
    let mut state = make_state();
    let msg = text_assistant("Hello!");

    let events = translate_cli_message(&msg, &mut state);
    assert!(state.started);

    // message_start + content_block_start + content_block_delta = 3
    // (no content_block_stop yet — block is still the last/open block)
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].0, "message_start");
    assert!(events[0].1.contains("message_start"));
    assert!(events[0].1.contains("claude-sonnet-4-6"));
}

#[test]
fn text_block_emits_start_and_delta() {
    let mut state = make_state();
    state.started = true;
    state.model = "claude-sonnet-4-6".to_string();

    let msg = text_assistant("World!");
    let events = translate_cli_message(&msg, &mut state);

    // content_block_start + content_block_delta = 2 (block stays open as last block)
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, "content_block_start");
    assert_eq!(events[1].0, "content_block_delta");
    assert!(events[1].1.contains("World!"));
}

#[test]
fn incremental_text_only_sends_diff() {
    let mut state = make_state();
    state.started = true;
    state.model = "claude-sonnet-4-6".to_string();

    // First partial: "Hel"
    let msg1 = text_assistant("Hel");
    let events1 = translate_cli_message(&msg1, &mut state);
    assert_eq!(events1.len(), 2); // start + delta("Hel")
    assert!(events1[1].1.contains("Hel"));

    // Second partial: "Hello!" (same block, more text)
    let msg2 = text_assistant("Hello!");
    let events2 = translate_cli_message(&msg2, &mut state);
    assert_eq!(events2.len(), 1); // only delta("lo!")
    assert_eq!(events2[0].0, "content_block_delta");
    assert!(events2[0].1.contains("lo!"));
    assert!(!events2[0].1.contains("Hel")); // no duplicate
}

#[test]
fn tool_use_block_passes_through() {
    let mut state = make_state();
    state.started = true;

    let msg = CliMessage::Assistant {
        message: CliAssistantMessage {
            id: Some("msg_test".to_string()),
            model: None,
            content: vec![ContentBlock::ToolUse {
                id: "tool_1".to_string(),
                name: "get_weather".to_string(),
                input: serde_json::json!({"city": "SF"}),
            }],
            usage: None,
        },
    };

    let events = translate_cli_message(&msg, &mut state);
    // content_block_start + content_block_delta (empty text) — block stays open
    // Actually tool_use has no text, so just start
    assert!(events.len() >= 1);
    assert_eq!(events[0].0, "content_block_start");
    assert!(events[0].1.contains("get_weather"));
    assert!(events[0].1.contains("tool_1"));
}

#[test]
fn result_closes_open_block_and_emits_stop() {
    let mut state = make_state();
    state.started = true;
    state.current_block_started = true; // simulate an open block

    let msg = CliMessage::Result {
        stop_reason: Some("end_turn".to_string()),
        usage: Some(CliUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 10,
            cache_creation_input_tokens: 5,
        }),
    };

    let events = translate_cli_message(&msg, &mut state);
    // content_block_stop (close open block) + message_delta + message_stop = 3
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].0, "content_block_stop");
    assert_eq!(events[1].0, "message_delta");
    assert!(events[1].1.contains("end_turn"));
    assert!(events[1].1.contains("100")); // input_tokens
    assert_eq!(events[2].0, "message_stop");
}

#[test]
fn result_emits_message_delta_and_stop() {
    let mut state = make_state();
    state.started = true;

    let msg = CliMessage::Result {
        stop_reason: Some("end_turn".to_string()),
        usage: Some(CliUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 10,
            cache_creation_input_tokens: 5,
        }),
    };

    let events = translate_cli_message(&msg, &mut state);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, "message_delta");
    assert!(events[0].1.contains("end_turn"));
    assert_eq!(events[1].0, "message_stop");
}

#[test]
fn rate_limit_event_produces_no_output() {
    let mut state = make_state();
    let msg = CliMessage::RateLimitEvent {};
    let events = translate_cli_message(&msg, &mut state);
    assert!(events.is_empty());
}

#[test]
fn thinking_then_text_incremental() {
    let mut state = make_state();
    state.started = true;
    state.model = "claude-opus-4-6".to_string();

    // Partial 1: thinking block only
    let msg1 = CliMessage::Assistant {
        message: CliAssistantMessage {
            id: Some("msg_test".to_string()),
            model: None,
            content: vec![ContentBlock::Thinking {
                thinking: "Let me think...".to_string(),
            }],
            usage: None,
        },
    };
    let events1 = translate_cli_message(&msg1, &mut state);
    assert_eq!(events1.len(), 2); // start + delta
    assert_eq!(events1[0].0, "content_block_start");
    assert_eq!(events1[1].0, "content_block_delta");
    assert!(events1[1].1.contains("Let me think..."));

    // Partial 2: thinking complete + text starts
    let msg2 = CliMessage::Assistant {
        message: CliAssistantMessage {
            id: Some("msg_test".to_string()),
            model: None,
            content: vec![
                ContentBlock::Thinking {
                    thinking: "Let me think...".to_string(),
                },
                ContentBlock::Text {
                    text: "Hi!".to_string(),
                },
            ],
            usage: None,
        },
    };
    let events2 = translate_cli_message(&msg2, &mut state);
    // thinking block stop + text block start + text delta = 3
    // (no extra thinking delta since text hasn't changed)
    assert_eq!(events2.len(), 3);
    assert_eq!(events2[0].0, "content_block_stop"); // close thinking
    assert_eq!(events2[1].0, "content_block_start"); // open text
    assert_eq!(events2[2].0, "content_block_delta"); // text delta
    assert!(events2[2].1.contains("Hi!"));

    // Result closes the text block
    let msg3 = CliMessage::Result {
        stop_reason: Some("end_turn".to_string()),
        usage: None,
    };
    let events3 = translate_cli_message(&msg3, &mut state);
    assert_eq!(events3.len(), 3); // block_stop + message_delta + message_stop
    assert_eq!(events3[0].0, "content_block_stop");
    assert_eq!(events3[1].0, "message_delta");
    assert_eq!(events3[2].0, "message_stop");
}

#[test]
fn usage_flows_from_cli_to_sse() {
    let mut state = make_state();
    let msg = CliMessage::Result {
        stop_reason: Some("end_turn".to_string()),
        usage: Some(CliUsage {
            input_tokens: 200,
            output_tokens: 75,
            cache_read_input_tokens: 30,
            cache_creation_input_tokens: 15,
        }),
    };

    let events = translate_cli_message(&msg, &mut state);
    let delta_json = &events[0].1;
    assert!(delta_json.contains("\"input_tokens\":200"));
    assert!(delta_json.contains("\"output_tokens\":75"));
    assert!(delta_json.contains("\"cache_read_input_tokens\":30"));
    assert!(delta_json.contains("\"cache_creation_input_tokens\":15"));
}

#[test]
fn result_without_usage_uses_accumulated_output_tokens() {
    let mut state = make_state();
    state.output_tokens = 42;

    let msg = CliMessage::Result {
        stop_reason: None,
        usage: None,
    };

    let events = translate_cli_message(&msg, &mut state);
    let delta_json = &events[0].1;
    assert!(delta_json.contains("\"output_tokens\":42"));
    // stop_reason defaults to end_turn
    assert!(delta_json.contains("end_turn"));
}
