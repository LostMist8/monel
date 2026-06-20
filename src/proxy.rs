//! Transparent pass-through proxy: the heart of the gateway.
//!
//! Maps `/chat/{provider_id}/<rest>` -> `{provider.base_url}/<rest, with one
//! leading `/v1` segment dropped if present>` and streams request+response
//! bytes verbatim. Upstream status codes, headers, and bodies (including SSE)
//! are forwarded unchanged; gateway-side failures yield a 502.

use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Request, Response};
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

/// Only small JSON request bodies are buffered for model-name stats. Everything
/// else is streamed through unchanged.
const REQUEST_MODEL_INSPECTION_LIMIT: u64 = 64 * 1024;

/// Small JSON responses can still be buffered to collect usage stats. SSE,
/// unknown-size responses, and larger responses are streamed.
const RESPONSE_USAGE_INSPECTION_LIMIT: u64 = 1024 * 1024;

enum ForwardBody {
    Buffered(axum::body::Bytes),
    Stream(reqwest::Body),
}

/// Catch-all handler: `/chat/{provider_id}/*path`.
pub async fn proxy(
    State(state): State<AppState>,
    Path((provider_id, rest)): Path<(String, String)>,
    req: Request<Body>,
) -> AppResult<Response<Body>> {
    let start = Instant::now();

    let provider = {
        let cfg = crate::config::read(&state.config).await;
        cfg.find_provider(&provider_id)
            .ok_or_else(|| AppError::not_found(format!("unknown provider: {provider_id}")))?
    };

    let upstream_url = build_upstream_url(&provider.base_url, &rest, req.uri().query())?;

    let (parts, body) = req.into_parts();
    let method = parts.method;

    let (model, forward_body) = prepare_forward_body(&parts.headers, body).await?;

    // Forwarded request headers: copy + sanitize, then set the upstream auth.
    let mut fwd_headers = sanitize(&parts.headers, REQ_HOP_BY_HOP);
    fwd_headers.insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(&format!("Bearer {}", provider.api_key))
            .map_err(|e| AppError::internal(format!("invalid provider api_key: {e}")))?,
    );

    let req_builder = state
        .http
        .request(
            reqwest::Method::from_bytes(method.as_str().as_bytes())
                .map_err(|e| AppError::internal(format!("invalid method: {e}")))?,
            upstream_url,
        )
        .headers(fwd_headers)
        .timeout(Duration::from_secs(600));

    let upstream_req = match forward_body {
        ForwardBody::Buffered(bytes) => req_builder.body(bytes),
        ForwardBody::Stream(body) => req_builder.body(body),
    }
    .build()
    .map_err(|e| AppError::bad_gateway(format!("failed to build upstream request: {e}")))?;

    let upstream_resp = state.http.execute(upstream_req).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match upstream_resp {
        Ok(resp) => {
            let status = resp.status();
            let resp_headers = sanitize(resp.headers(), RESP_HOP_BY_HOP);
            let should_buffer = should_buffer_response_for_usage(&resp);

            if should_buffer {
                let resp_bytes = resp.bytes().await.map_err(|e| {
                    AppError::bad_gateway(format!("failed to read upstream response: {e}"))
                })?;
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
                )
                .await;

                let mut out = Response::new(Body::from(resp_bytes));
                *out.status_mut() = status;
                *out.headers_mut() = resp_headers;

                return Ok(out);
            }

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
                None,
            )
            .await;

            let mut out = Response::new(Body::from_stream(resp.bytes_stream()));
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
            )
            .await;

            Err(AppError::bad_gateway(format!(
                "upstream connect failed: {e}"
            )))
        }
    }
}

async fn prepare_forward_body(headers: &HeaderMap, body: Body) -> AppResult<(String, ForwardBody)> {
    if should_buffer_request_for_model(headers) {
        let body_bytes = axum::body::to_bytes(body, REQUEST_MODEL_INSPECTION_LIMIT as usize)
            .await
            .map_err(|e| AppError::internal(format!("failed to read request body: {e}")))?;
        let model = extract_model_from_body(&body_bytes);
        return Ok((model, ForwardBody::Buffered(body_bytes)));
    }

    Ok((
        "unknown".to_string(),
        ForwardBody::Stream(reqwest::Body::wrap_stream(body.into_data_stream())),
    ))
}

fn should_buffer_request_for_model(headers: &HeaderMap) -> bool {
    let Some(content_length) = content_length(headers) else {
        return false;
    };

    content_length <= REQUEST_MODEL_INSPECTION_LIMIT && is_json_content_type(headers)
}

fn should_buffer_response_for_usage(resp: &reqwest::Response) -> bool {
    let headers = resp.headers();
    let Some(content_length) = resp.content_length() else {
        return false;
    };

    content_length <= RESPONSE_USAGE_INSPECTION_LIMIT
        && is_json_content_type(headers)
        && !is_event_stream(headers)
}

fn content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
}

fn is_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("json"))
        .unwrap_or(false)
}

fn is_event_stream(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false)
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
    let input = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cache = usage
        .get("cache_read_input_tokens")
        .or_else(|| {
            usage
                .get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
        })
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    Some(TokenUsage {
        input_tokens: input,
        output_tokens: output,
        cache_read_input_tokens: cache,
    })
}

/// Build the final upstream URL.
///
/// Logic: take the path after `/chat/{provider_id}` (`rest`), drop one leading
/// `v1` segment if present, then append to the provider's `base_url`, finally
/// re-attach the query string.
fn build_upstream_url(base_url: &str, rest: &str, query: Option<&str>) -> AppResult<Url> {
    let base = Url::parse(base_url).map_err(|e| {
        AppError::bad_gateway(format!("invalid provider base_url '{base_url}': {e}"))
    })?;

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

    let mut out = base
        .join(&base_path)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_url_drops_one_leading_v1_segment() {
        let url = build_upstream_url(
            "https://api.example.com/v1",
            "v1/chat/completions",
            Some("stream=true"),
        )
        .unwrap();

        assert_eq!(
            url.as_str(),
            "https://api.example.com/v1/chat/completions?stream=true"
        );
    }

    #[test]
    fn upstream_url_preserves_base_path_without_duplicate_slashes() {
        let url = build_upstream_url(
            "https://proxy.example.com/provider/openai/",
            "/v1/models",
            None,
        )
        .unwrap();

        assert_eq!(
            url.as_str(),
            "https://proxy.example.com/provider/openai/models"
        );
    }

    #[test]
    fn upstream_url_handles_root_v1_path() {
        let url = build_upstream_url("https://api.example.com/v1", "v1", None).unwrap();

        assert_eq!(url.as_str(), "https://api.example.com/v1");
    }

    #[test]
    fn request_model_inspection_requires_small_json_body() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("128"));

        assert!(should_buffer_request_for_model(&headers));

        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("1048576"));
        assert!(!should_buffer_request_for_model(&headers));
    }
}
