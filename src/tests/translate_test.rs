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

    // message_start + content_block_start + content_block_delta + content_block_stop = 4
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].0, "message_start");
    assert!(events[0].1.contains("message_start"));
    assert!(events[0].1.contains("claude-sonnet-4-6"));
}

#[test]
fn text_block_emits_start_delta_stop() {
    let mut state = make_state();
    state.started = true; // skip message_start
    state.model = "claude-sonnet-4-6".to_string();

    let msg = text_assistant("World!");
    let events = translate_cli_message(&msg, &mut state);

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].0, "content_block_start");
    assert_eq!(events[1].0, "content_block_delta");
    assert!(events[1].1.contains("World!"));
    assert_eq!(events[2].0, "content_block_stop");
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
    // content_block_start + content_block_stop = 2
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, "content_block_start");
    assert!(events[0].1.contains("get_weather"));
    assert!(events[0].1.contains("tool_1"));
    assert_eq!(events[1].0, "content_block_stop");
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
    assert!(events[0].1.contains("100")); // input_tokens
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
fn block_index_increments_across_messages() {
    let mut state = make_state();
    state.started = true;

    let msg1 = text_assistant("First");
    translate_cli_message(&msg1, &mut state);
    assert_eq!(state.block_index, 1);

    let msg2 = text_assistant("Second");
    let events = translate_cli_message(&msg2, &mut state);
    assert_eq!(state.block_index, 2);

    // Check that second block uses index 1
    assert!(events[0].1.contains("\"index\":1"));
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
