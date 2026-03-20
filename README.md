# cc-max-proxy-rs

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange.svg)](https://www.rust-lang.org/)
[![Claude Code](https://img.shields.io/badge/Claude_Code-Compatible-blueviolet.svg)](https://code.claude.com)

**Transparent proxy that bridges your Claude Max subscription to any Anthropic API client.**

Use Claude Opus, Sonnet, or Haiku from any tool that speaks the Anthropic Messages API — without paying per-token API costs. Just use your existing Max subscription.

> Inspired by [opencode-claude-max-proxy](https://github.com/rynfar/opencode-claude-max-proxy) (TypeScript/Bun). This is a standalone Rust rewrite — single binary, zero runtime dependencies, generic for any client.

---

## How It Works

```
Your Tool ──POST /v1/messages──▶ Proxy (localhost:3456) ──▶ claude CLI ──▶ Claude Max
                                        │
                                  Anthropic SSE ◀──── NDJSON stdout
```

The proxy is a **drop-in replacement** for `api.anthropic.com`. It accepts standard Anthropic Messages API requests, spawns the `claude` CLI under the hood (which authenticates via your Max subscription), and returns standard Anthropic SSE streaming events. Your tool never knows the difference.

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

### Install & Run

```bash
git clone https://github.com/adolfousier/cc-max-proxy-rs
cd cc-max-proxy-rs
cargo build --release
./target/release/cc-max-proxy-rs
```

```
cc-max-proxy-rs listening on http://127.0.0.1:3456
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
| `RUST_LOG` | `cc_max_proxy_rs=info` | Log level |

## Features

- **Single binary** — no Node.js, no Bun, no runtime dependencies
- **Transparent** — standard Anthropic Messages API, streaming SSE
- **Zero config on client** — just set base URL, accepts any API key
- **Model mapping** — `*opus*` → opus, `*haiku*` → haiku, default → sonnet
- **Streaming + non-streaming** — both `stream: true` and `stream: false` supported
- **Request serialization** — configurable semaphore prevents CLI spawn conflicts
- **Health endpoint** — `GET /` returns proxy status

## Architecture

```
src/
├── main.rs          # Entry point, env config, server startup
├── server.rs        # Axum router, /v1/messages handler, SSE streaming
├── claude_cli.rs    # Spawn claude CLI, read NDJSON stream
├── translate.rs     # CLI NDJSON → Anthropic SSE event translation
├── types.rs         # Request/response/CLI message types
└── error.rs         # Typed errors (thiserror)
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

## Limitations

- **Concurrency**: Claude CLI may not handle parallel spawns well — default `MAX_CONCURRENT=1` serializes requests
- **Latency**: Extra hop through CLI process spawn adds ~1-2s to first response
- **Rate limits**: Subject to your Claude Max subscription limits
- **No tool execution**: The proxy passes tool_use blocks through but does not execute them — your client handles tool execution

## FAQ

**Why not just use an API key?**
API keys cost per token. Claude Max is a flat subscription with generous limits. This proxy lets you use that subscription from any tool — same models, zero per-token costs.

**Why Rust instead of TypeScript?**
Single binary. No `node_modules`, no `bun install`, no runtime dependencies. Download, run, done.

**How do I connect my tool?**
Just change your tool's Anthropic base URL to `http://127.0.0.1:3456`. Your existing API key stays as-is — it still works for things like model listing (`/v1/models`), while the proxy handles the actual inference through the CLI in the background.

**Why does it need Claude Code CLI?**
The CLI handles all authentication with your Max subscription. The proxy just spawns it — no tokens, no OAuth, no credentials to manage. If `claude login` works, the proxy works.

**What about rate limits?**
Your Claude Max subscription has its own usage limits. The proxy adds no additional constraints.

**Is my data sent somewhere?**
No. Everything runs locally. Requests go from your tool → proxy → `claude` CLI → Anthropic. Same path as using Claude Code directly.

**Can I run multiple proxies?**
Yes. Use different ports: `PORT=3457 cargo run`.

**Does this work with Claude Teams/Enterprise?**
If `claude login` authenticates your account and `claude -p "hello"` works, the proxy works. It uses whatever account the CLI is signed into.

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "claude CLI not found" | Install: `npm install -g @anthropic-ai/claude-code` |
| "Authentication failed" in CLI | Run `claude login` to re-authenticate |
| "Connection refused" on port 3456 | Ensure proxy is running: `cargo run` |
| Port already in use | `kill $(lsof -ti :3456)` or use `PORT=4567 cargo run` |
| No response from proxy | Check `RUST_LOG=debug cargo run` for CLI spawn errors |
| Slow first response | Normal — CLI process spawn takes ~1-2s |

## Disclaimer

This is an unofficial project that uses Anthropic's publicly available Claude Code CLI. It is **not** affiliated with, endorsed by, or supported by Anthropic.

**Use at your own risk.** Review Anthropic's [Terms of Service](https://www.anthropic.com/terms) and [Authorized Usage Policy](https://www.anthropic.com/usage-policy). This project calls the `claude` CLI using your own authenticated account — no API keys are intercepted, no authentication is bypassed, no proprietary systems are reverse-engineered.

## Credits

- Inspired by [opencode-claude-max-proxy](https://github.com/rynfar/opencode-claude-max-proxy) by [@rynfar](https://github.com/rynfar)
- Built with the [Claude Code CLI](https://code.claude.com) by Anthropic

## License

[MIT](LICENSE)
