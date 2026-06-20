# Monel Gateway

Monel Gateway is a lightweight OpenAI-compatible API gateway. It provides a local proxy, provider routing, model aggregation, admin endpoints, usage stats, and an optional Tauri desktop shell.

## Quick Start

1. Edit `config.yaml` and set your local auth key and provider credentials.
2. Start the gateway:

```powershell
cargo run -- server --config config.yaml
```

3. Check the service:

```powershell
curl http://127.0.0.1:7890/health
```

4. Send OpenAI-compatible requests through a provider route:

```text
http://127.0.0.1:7890/chat/{provider_id}/v1/chat/completions
```

## Configuration

`config.yaml` contains the local server address, optional gateway auth key, and provider definitions.

Do not commit real API keys. Use local-only files such as `config.local.yaml` for private credentials, or rotate any key that has ever appeared in Git history.

## API Surface

- `GET /health`
- `ANY /chat/:provider_id/*path`
- `GET /models`
- `GET /providers`
- `GET /admin/config`
- `POST /admin/config`
- `POST /admin/reload`
- `GET /admin/stats`
- `GET /admin/logs`

Protected routes accept `?key=...` or `Authorization: Bearer ...` when `server.auth_key` is configured.

## Desktop App

The Tauri app lives in `src-tauri`. In desktop mode, Monel stores runtime configuration in the platform app config directory and copies a development config there on first launch when available.

```powershell
cd src-tauri
cargo check
```

## Archived Notes

Older project notes and setup writeups are kept in `docs/archive/` for reference. New contributor-facing guidance should go in `CONTRIBUTING.md`.
