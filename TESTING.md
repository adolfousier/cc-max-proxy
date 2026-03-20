# Testing

## Run Tests

```bash
cargo test --all-features
```

## Test Structure

All tests live in `src/tests/` as dedicated `*_test.rs` files:

```
src/tests/
├── mod.rs
├── types_test.rs        # Request/response deserialization
├── translate_test.rs    # NDJSON → Anthropic SSE translation
└── claude_cli_test.rs   # CLI path resolution, prompt building, model mapping
```

## What's Tested

### Types (`types_test.rs`)
- Anthropic Messages API request deserialization
- Message content parsing (string and block array formats)
- CLI NDJSON message parsing (system, assistant, result, rate_limit_event)
- Stream defaults to `true` when omitted
- Unknown fields in CLI messages are silently skipped

### Translation (`translate_test.rs`)
- System message sets model on state
- First assistant message emits `message_start` + content blocks
- Text content blocks emit `content_block_start` + `content_block_delta` + `content_block_stop`
- Tool use blocks pass through as-is
- Result message emits `message_delta` + `message_stop`
- Rate limit events produce no output
- Block index increments correctly across messages
- Usage data flows from CLI to SSE events

### CLI (`claude_cli_test.rs`)
- Model mapping: opus/haiku/sonnet/default
- Prompt building from system + messages
- System prompt as string and as block array
- CLI path resolution (CLAUDE_PATH env override)

## Manual Testing

### Start the proxy

```bash
cargo run
```

### Streaming request

```bash
curl -X POST http://127.0.0.1:3456/v1/messages \
  -H "x-api-key: dummy" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 100,
    "messages": [{"role": "user", "content": "Say hello"}],
    "stream": true
  }'
```

### Non-streaming request

```bash
curl -X POST http://127.0.0.1:3456/v1/messages \
  -H "x-api-key: dummy" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 100,
    "messages": [{"role": "user", "content": "Say hello"}],
    "stream": false
  }'
```

### Health check

```bash
curl http://127.0.0.1:3456/
```

## CI

Tests run on every push via GitHub Actions. See `.github/workflows/ci.yml`.
