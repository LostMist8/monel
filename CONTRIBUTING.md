# Contributing

## Development

Run formatting and tests before handing off changes:

```powershell
cargo fmt --check
cargo test
cd src-tauri
cargo check
```

Use `cargo fmt` to apply Rust formatting.

## Secrets

Never commit real provider API keys, auth tokens, or generated local config. If a secret is committed, rotate it immediately and clean the repository history before publishing.

Recommended local files:

- `config.local.yaml`
- `.env`

## Proxy Behavior

The proxy should stay transparent by default:

- Stream SSE and large responses instead of buffering them.
- Stream large or unknown-size request bodies.
- Only buffer small JSON request/response bodies when needed for stats.
- Preserve upstream status codes and non-hop-by-hop headers.

## Tests

Add focused unit tests for URL rewriting, request/response buffering decisions, and config reload behavior whenever those paths change.
