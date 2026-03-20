# cc-max-proxy

[![Crates.io](https://img.shields.io/crates/v/cc-max-proxy.svg)](https://crates.io/crates/cc-max-proxy)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange.svg)](https://www.rust-lang.org/)

**Drop-in replacement for `api.anthropic.com` that routes requests through the `claude` CLI on your machine.**

No SDK, no API key, no per-token costs. If `claude` works in your terminal, the proxy works. Any tool that speaks the Anthropic Messages API can use your Max subscription — the proxy just translates the wire format.

> Inspired by [opencode-claude-max-proxy](https://github.com/rynfar/opencode-claude-max-proxy) (TypeScript/Bun). Standalone Rust rewrite — single binary, zero runtime dependencies.

---

## How It Works

```
Your Tool ──POST /v1/messages──▶ Proxy (localhost:3456) ──▶ claude CLI ──▶ Claude Max
                                        │
                                  Anthropic SSE ◀──── NDJSON stdout
```

The proxy accepts Anthropic Messages API requests, spawns the `claude` CLI already authenticated on your machine, and streams back standard Anthropic SSE events. Your tool never knows the difference.

## Quick Start

### Prerequisites

- **Claude Code CLI** installed and authenticated:
  ```bash
  npm install -g @anthropic-ai/claude-code
  claude login
  ```
- **Rust toolchain** (1.85+):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

### Install from crates.io

```bash
cargo install cc-max-proxy
cc-max-proxy
```

### Or build from source

```bash
git clone https://github.com/adolfousier/cc-max-proxy-rs
cd cc-max-proxy-rs
cargo build --release
./target/release/cc-max-proxy
```

```
cc-max-proxy listening on http://127.0.0.1:3456
Set ANTHROPIC_BASE_URL=http://127.0.0.1:3456 in your tool
```

### Connect Your Tool

Just change your tool's Anthropic base URL to point at the proxy. Nothing else needs to change — your existing API key config stays as-is (the proxy ignores it).

```bash
# Environment variable (works with most tools)
ANTHROPIC_BASE_URL=http://127.0.0.1:3456 your-tool

# Or in your tool's config file
# base_url = "http://127.0.0.1:3456"
```

### Test It

```bash
curl -X POST http://127.0.0.1:3456/v1/messages \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 100,
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }'
```

## Why CLI, Not SDK?

Other proxies call the Anthropic API directly — requiring an API key. This proxy spawns the `claude` CLI binary already on your machine. No API key, no SDK, no direct API calls. Just the same CLI you already use in your terminal.

```
SDK approach:   Tool → proxy → Anthropic API (needs API key)
CLI approach:   Tool → proxy → claude CLI (already logged in)
```

## Compatibility

Works with any tool that speaks the Anthropic Messages API:

| Tool | Config |
|------|--------|
| [OpenCrabs](https://github.com/adolfousier/opencrabs) | `base_url = "http://127.0.0.1:3456"` in `[providers.anthropic]` |
| [aider](https://aider.chat) | `ANTHROPIC_BASE_URL=http://127.0.0.1:3456 aider` |
| [continue.dev](https://continue.dev) | Set `apiBase` in config |
| [Cursor](https://cursor.sh) | Set base URL in Anthropic provider settings |
| Custom apps | Point Anthropic SDK `base_url` at proxy |

## Configuration

All configuration via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3456` | Listen port |
| `HOST` | `127.0.0.1` | Listen address |
| `CLAUDE_PATH` | auto-detect | Path to `claude` binary |
| `MAX_CONCURRENT` | `1` | Max parallel CLI spawns |

Use `--debug` flag for verbose logging:

```bash
cc-max-proxy --debug
```

## Features

- **CLI-based** — uses the official `claude` binary, no API key needed, no ToS concerns
- **Single binary** — no Node.js, no Bun, no runtime dependencies
- **Transparent** — standard Anthropic Messages API, streaming SSE
- **Zero config on client** — just set base URL, accepts any API key
- **Model mapping** — `*opus*` → opus, `*haiku*` → haiku, default → sonnet
- **Streaming + non-streaming** — both `stream: true` and `stream: false` supported
- **Secure** — prompts piped via stdin (not visible in `ps aux`)
- **Working directory aware** — `X-Working-Dir` header sets the CLI's cwd for project-scoped sessions
- **Request serialization** — configurable semaphore prevents CLI spawn conflicts
- **Health endpoint** — `GET /` returns proxy status

## Architecture

```
src/
├── main.rs          # Entry point, CLI args (--debug), server startup
├── server.rs        # Axum router, /v1/messages handler, SSE streaming
├── claude_cli.rs    # Spawn claude CLI, read NDJSON stream, env isolation
├── translate.rs     # CLI NDJSON → Anthropic SSE event translation
├── types.rs         # Request/response/CLI message types
├── error.rs         # Typed errors (thiserror)
└── tests/           # Unit tests (types, translation, CLI logic)
```

The proxy translates between two formats:

**Claude CLI** outputs NDJSON (one JSON object per line):
```json
{"type":"system","model":"claude-sonnet-4-6","session_id":"..."}
{"type":"assistant","message":{"content":[{"type":"text","text":"Hello!"}],...}}
{"type":"result","stop_reason":"end_turn","usage":{...}}
```

**Anthropic API** expects SSE events:
```
event: message_start
data: {"type":"message_start","message":{"id":"msg_...","model":"claude-sonnet-4-6",...}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello!"}}

event: message_stop
data: {"type":"message_stop"}
```

## Headers

| Header | Description |
|--------|-------------|
| `x-api-key` | Accepted but ignored |
| `Authorization` | Accepted but ignored |
| `X-Working-Dir` | Sets the CLI's working directory for the request (e.g., your project root) |

## Limitations

- **Concurrency**: Claude CLI may not handle parallel spawns well — default `MAX_CONCURRENT=1` serializes requests
- **Latency**: Extra hop through CLI process spawn adds ~1-2s to first response
- **Rate limits**: Subject to your Claude Max subscription limits
- **No tool execution**: The proxy passes tool_use blocks through but does not execute them — your client handles tool execution

## FAQ

**Why not just use an API key?**
API keys cost per token. Max is flat-rate. This lets any tool use your Max subscription.

**Why Rust instead of TypeScript?**
Single binary. No `node_modules`, no runtime. Download, run, done.

**Why CLI instead of SDK?**
The CLI is already logged in on your machine. No API key needed, no SDK, no direct API calls.

**How do I connect my tool?**
Set base URL to `http://127.0.0.1:3456`. The proxy ignores any API key your tool sends.

**What about rate limits?**
Same as your Max subscription. The proxy adds nothing on top.

**Is my data sent somewhere?**
No. Everything is local. Tool → proxy → `claude` CLI → Anthropic. Same path as using the CLI directly.

**Can I run multiple proxies?**
Yes. Different ports: `PORT=3457 cc-max-proxy`.

**Does this work with Teams/Enterprise?**
If `claude -p "hello"` works, the proxy works. It uses whatever account the CLI is signed into.

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "claude CLI not found" | Install: `npm install -g @anthropic-ai/claude-code` |
| "Authentication failed" in CLI | Run `claude login` to re-authenticate |
| "Connection refused" on port 3456 | Ensure proxy is running: `cc-max-proxy` |
| Port already in use | `kill $(lsof -ti :3456)` or use `PORT=4567 cc-max-proxy` |
| No response from proxy | Run `cc-max-proxy --debug` for verbose CLI spawn logs |
| Slow first response | Normal — CLI process spawn takes ~1-2s |
| "Nested session" errors | Proxy strips `CLAUDECODE` env var automatically since v0.1.0 |

## Disclaimer

Unofficial project. Not affiliated with or endorsed by Anthropic. Uses the publicly available Claude Code CLI. Use at your own risk — review Anthropic's [Terms of Service](https://www.anthropic.com/terms) and [Usage Policy](https://www.anthropic.com/usage-policy).

## Credits

- Inspired by [opencode-claude-max-proxy](https://github.com/rynfar/opencode-claude-max-proxy) by [@rynfar](https://github.com/rynfar)
- Built with the [Claude Code CLI](https://code.claude.com) by Anthropic

## License

[MIT](LICENSE)
