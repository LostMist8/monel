//! Auth middleware: enforces the global `server.auth_key`.

use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use crate::error::AppError;
use crate::state::AppState;
use subtle::ConstantTimeEq;

/// Middleware: requires a valid auth key from `?key=` or `Authorization: Bearer`.
/// Internal token from Tauri frontend bypasses auth.
pub async fn require_auth(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Check if request has internal token (from Tauri frontend)
    if let Some(key) = extract_key(&req)? {
        // Priority 1: Check internal token (Tauri frontend)
        if constant_time_eq(key.as_bytes(), state.internal_token.as_bytes()) {
            return Ok(next.run(req).await);
        }
    }

    // Priority 2: Check user-configured auth_key (external requests)
    let expected = {
        let cfg = crate::config::read(&state.config).await;
        cfg.server.auth_key.clone()
    };

    // No key configured = open access (still bound to 127.0.0.1 by default).
    if expected.is_empty() {
        return Ok(next.run(req).await);
    }

    if let Some(key) = extract_key(&req)? {
        if constant_time_eq(key.as_bytes(), expected.as_bytes()) {
            return Ok(next.run(req).await);
        }
    }

    Err(AppError::new(
        StatusCode::UNAUTHORIZED,
        "invalid or missing auth key",
    ))
}

/// Pull a candidate key from query string or Bearer header.
fn extract_key(req: &Request<axum::body::Body>) -> Result<Option<String>, AppError> {
    // ?key=...
    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            let mut it = pair.splitn(2, '=');
            if it.next() == Some("key") {
                if let Some(v) = it.next() {
                    return Ok(Some(percent_decode(v)));
                }
            }
        }
    }

    // Authorization: Bearer ...
    if let Some(auth) = req.headers().get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = auth.to_str() {
            if let Some(rest) = s.strip_prefix("Bearer ") {
                return Ok(Some(rest.trim().to_string()));
            }
        }
    }

    Ok(None)
}

/// Minimal percent-decoding for the `key` query value.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}
