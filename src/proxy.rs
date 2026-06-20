//! Transparent pass-through proxy: the heart of the gateway.
//!
//! Maps `/chat/{provider_id}/<rest>` -> `{provider.base_url}/<rest, with one
//! leading `/v1` segment dropped if present>` and streams request+response
//! bytes verbatim. Upstream status codes, headers, and bodies (including SSE)
//! are forwarded unchanged; gateway-side failures yield a 502.

use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, Response};
use serde::Serialize;
use url::Url;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::stats;

/// Headers we strip from the *request* before forwarding to the upstream.
const REQ_HOP_BY_HOP: &[&str] = &[
    "host",
    "authorization",
    "content-length",
    "connection",
    "keep-alive",
    "proxy-authorization",
    "proxy-authenticate",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// Headers we strip from the upstream *response* before returning to the client.
const RESP_HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "content-length", // body is re-framed by the stream
];

/// Catch-all handler: `/chat/{provider_id}/*path`.
pub async fn proxy(
    State(state): State<AppState>,
    Path((provider_id, rest)): Path<(String, String)>,
    req: Request<Body>,
) -> AppResult<Response<Body>> {
    let start = Instant::now();

    let provider = {
        let cfg = crate::config::read(&state.config);
        cfg.find_provider(&provider_id)
            .ok_or_else(|| AppError::not_found(format!("unknown provider: {provider_id}")))?
    };

    let upstream_url = build_upstream_url(&provider.base_url, &rest, req.uri().query())?;

    let (parts, body) = req.into_parts();
    let method = parts.method;

    // Buffer the request body so we can extract the model name
    let body_bytes = axum::body::to_bytes(body, 10 * 1024 * 1024) // 10MB limit
        .await
        .map_err(|e| AppError::internal(format!("failed to read request body: {e}")))?;

    // Extract model name from request body
    let model = extract_model_from_body(&body_bytes);

    // Forwarded request headers: copy + sanitize, then set the upstream auth.
    let mut fwd_headers = sanitize(&parts.headers, REQ_HOP_BY_HOP);
    fwd_headers.insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(&format!("Bearer {}", provider.api_key))
            .map_err(|e| AppError::internal(format!("invalid provider api_key: {e}")))?,
    );

    let upstream_req = state
        .http
        .request(reqwest::Method::from_bytes(method.as_str().as_bytes())
            .map_err(|e| AppError::internal(format!("invalid method: {e}")))?, upstream_url)
        .headers(fwd_headers)
        .body(body_bytes.clone())
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| AppError::bad_gateway(format!("failed to build upstream request: {e}")))?;

    let upstream_resp = state
        .http
        .execute(upstream_req)
        .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match upstream_resp {
        Ok(resp) => {
            let status = resp.status();
            let resp_headers = sanitize(resp.headers(), RESP_HOP_BY_HOP);

            // Buffer the response to extract token usage
            let resp_bytes = resp.bytes().await
                .map_err(|e| AppError::bad_gateway(format!("failed to read upstream response: {e}")))?;

            // Extract token usage from response
            let usage = extract_usage_from_response(&resp_bytes);

            let error_msg = if !status.is_success() {
                Some(format!("HTTP {}", status.as_u16()))
            } else {
                None
            };

            stats::record_request(
                state.stats.clone(),
                provider_id.clone(),
                model,
                status.as_u16(),
                duration_ms,
                error_msg,
                usage,
            ).await;

            let mut out = Response::new(Body::from(resp_bytes));
            *out.status_mut() = status;
            *out.headers_mut() = resp_headers;

            Ok(out)
        }
        Err(e) => {
            stats::record_request(
                state.stats.clone(),
                provider_id.clone(),
                model,
                502,
                duration_ms,
                Some(e.to_string()),
                None,
            ).await;

            Err(AppError::bad_gateway(format!("upstream connect failed: {e}")))
        }
    }
}

/// Extract model name from JSON request body.
fn extract_model_from_body(body: &[u8]) -> String {
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
        if let Some(model) = json.get("model").and_then(|v| v.as_str()) {
            return model.to_string();
        }
    }
    "unknown".to_string()
}

/// Token usage info extracted from response.
#[derive(Debug, Clone, Serialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
}

/// Extract token usage from JSON response body.
fn extract_usage_from_response(body: &[u8]) -> Option<TokenUsage> {
    let json: serde_json::Value = serde_json::from_slice(body).ok()?;
    let usage = json.get("usage")?;

    // OpenAI format: prompt_tokens, completion_tokens
    // Anthropic format: input_tokens, output_tokens
    let input = usage.get("input_tokens").or_else(|| usage.get("prompt_tokens"))
        .and_then(|v| v.as_u64()).unwrap_or(0);
    let output = usage.get("output_tokens").or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_u64()).unwrap_or(0);
    let cache = usage.get("cache_read_input_tokens")
        .or_else(|| usage.get("prompt_tokens_details").and_then(|d| d.get("cached_tokens")))
        .and_then(|v| v.as_u64()).unwrap_or(0);

    Some(TokenUsage { input_tokens: input, output_tokens: output, cache_read_input_tokens: cache })
}

/// Build the final upstream URL.
///
/// Logic: take the path after `/chat/{provider_id}` (`rest`), drop one leading
/// `v1` segment if present, then append to the provider's `base_url`, finally
/// re-attach the query string.
fn build_upstream_url(base_url: &str, rest: &str, query: Option<&str>) -> AppResult<Url> {
    let base = Url::parse(base_url)
        .map_err(|e| AppError::bad_gateway(format!("invalid provider base_url '{base_url}': {e}")))?;

    // Normalize `rest`: axum may give us "v1/chat/completions" (no leading slash).
    let mut path = rest.trim_start_matches('/').to_string();

    // Drop one leading "v1" segment.
    if path == "v1" {
        path.clear();
    } else if let Some(stripped) = path.strip_prefix("v1/") {
        path = stripped.to_string();
    }

    // Compose the path: base_url's existing path + the (possibly-v1-stripped) tail.
    let mut base_path = base.path().trim_end_matches('/').to_string();
    if !path.is_empty() {
        base_path.push('/');
        base_path.push_str(&path);
    }
    if base_path.is_empty() {
        base_path.push('/');
    }

    let mut out = base.join(&base_path)
        .map_err(|e| AppError::bad_gateway(format!("failed to join upstream path: {e}")))?;
    if let Some(q) = query {
        out.set_query(Some(q));
    }

    Ok(out)
}

/// Copy a header map, omitting any header in the denylist (case-insensitive).
fn sanitize(src: &HeaderMap, denylist: &[&str]) -> HeaderMap {
    let mut out = HeaderMap::with_capacity(src.len());
    for (name, value) in src.iter() {
        let lname = name.as_str().to_ascii_lowercase();
        if denylist.iter().any(|h| *h == lname) {
            continue;
        }
        // HeaderName is cheap to clone.
        out.append(name.clone(), value.clone());
    }
    out
}
