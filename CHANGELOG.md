# Changelog

All notable changes to cc-max-proxy will be documented in this file.

## [0.1.1] - 2026-03-21

### Fixed

- Forward CLI `stream_event` SSE events directly instead of re-translating accumulated `assistant` messages. Eliminates content duplication caused by `--include-partial-messages` and ensures streams complete with proper `message_stop` events.
- Emit `reasoning_delta` (not `thinking_delta`) for extended thinking blocks, matching the Anthropic SSE spec and fixing deserialization in downstream clients.
- Include `tool_result`, `tool_use`, and `thinking` content blocks in `build_prompt()` — previously only `Text` blocks were extracted, causing the CLI to see empty messages and hallucinate "you sent a blank message".
- Skip empty messages in prompt construction to prevent phantom user turns.

### Changed

- Added `StreamEvent` variant to `CliMessage` enum for parsing CLI's `{"type":"stream_event","event":{...}}` wrappers.
- Incremental diffing fallback for environments without `stream_event` support — tracks `completed_blocks` and `current_block_chars` to avoid re-emitting content.
- `TranslateState` now tracks `streaming_via_events` and `got_stop` flags to prevent duplicate events when both `stream_event` and `assistant`/`result` messages arrive.

## [0.1.0] - 2026-03-20

Initial release.

### Features

- Transparent drop-in proxy for `api.anthropic.com` — tools just point their base URL at `localhost:3456`.
- Routes requests through the local `claude` CLI using your Claude Max subscription.
- Full Anthropic Messages API compatibility: streaming SSE, non-streaming JSON, tool use, extended thinking.
- Incremental streaming with content block diffing for real-time token delivery.
- Extended thinking support — `thinking` content blocks streamed as `reasoning_delta` events.
- Prompt piped via stdin (not CLI args) to prevent exposure in `ps aux`.
- Session isolation — each request runs in a temp directory with `--no-session-persistence`.
- `X-Working-Dir` header support for proxy-aware clients to set the CLI's working directory.
- `--debug` flag for debug-level logging.
- Configurable concurrency via `MAX_CONCURRENT` env var (default: 1).
- Auto-detection of `claude` binary location.
- 28 unit tests covering types, translation, and CLI modules.
- CI/CD: GitHub Actions for fmt/clippy/tests + release workflow with crates.io publish and 5-platform binary builds (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64).
