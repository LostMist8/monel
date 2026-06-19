//! Transparent pass-through proxy: the heart of the gateway.
//!
//! Maps `/chat/{provider_id}/<rest>` -> `{provider.base_url}/<rest, with one
//! leading `/v1` segment dropped if present>` and streams request+response
//! bytes verbatim. Upstream status codes, headers, and bodies (including SSE)
//! are forwarded unchanged; gateway-side failures yield a 502.

use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, Response};
use futures::stream::StreamExt;
use url::Url;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

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
    // Resolve the provider under a short-lived read lock, then release it
    // before any `.await` below — the guard is `!Send` and would make the
    // whole handler future `!Send`, which axum's `Handler` trait rejects.
    let provider = {
        let cfg = crate::config::read(&state.config);
        cfg.find_provider(&provider_id)
            .ok_or_else(|| AppError::not_found(format!("unknown provider: {provider_id}")))?
    };

    let upstream_url = build_upstream_url(&provider.base_url, &rest, req.uri().query())?;

    let (parts, body) = req.into_parts();
    let method = parts.method;

    // Forwarded request headers: copy + sanitize, then set the upstream auth.
    let mut fwd_headers = sanitize(&parts.headers, REQ_HOP_BY_HOP);
    fwd_headers.insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(&format!("Bearer {}", provider.api_key))
            .map_err(|e| AppError::internal(format!("invalid provider api_key: {e}")))?,
    );

    let req_body = reqwest::Body::wrap_stream(body.into_data_stream());

    let upstream_req = state
        .http
        .request(reqwest::Method::from_bytes(method.as_str().as_bytes())
            .map_err(|e| AppError::internal(format!("invalid method: {e}")))?, upstream_url)
        .headers(fwd_headers)
        .body(req_body)
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| AppError::bad_gateway(format!("failed to build upstream request: {e}")))?;

    let upstream_resp = state
        .http
        .execute(upstream_req)
        .await
        .map_err(|e| AppError::bad_gateway(format!("upstream connect failed: {e}")))?;

    let status = upstream_resp.status();
    let resp_headers = sanitize(upstream_resp.headers(), RESP_HOP_BY_HOP);

    let byte_stream = upstream_resp.bytes_stream().map(|res| {
        res.map_err(|e| {
            // axum BoxError is a generic error type; lossy-convert to std::io::Error.
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })
    });

    let mut out = Response::new(Body::from_stream(byte_stream));
    *out.status_mut() = status;
    *out.headers_mut() = resp_headers;

    Ok(out)
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
