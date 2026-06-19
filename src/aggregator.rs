//! Aggregation endpoints: `GET /models` (fan-out) and `GET /providers`.

use std::time::Duration;

use axum::extract::State;
use axum::http::HeaderValue;
use axum::response::IntoResponse;
use axum::Json;
use futures::future::join_all;
use serde::Serialize;

use crate::config::Provider;
use crate::error::AppResult;
use crate::state::AppState;

/// Per-provider timeout when fanning out for `/models`. Keeps one slow
/// provider from stalling the aggregate.
const MODELS_FANOUT_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Serialize)]
pub struct ProviderSummary {
    pub id: String,
    pub name: String,
    pub base_url: String,
}

#[derive(Debug, Serialize)]
pub struct ModelEntry {
    pub provider_id: String,
    pub model: String,
    pub name: String,
}

/// `GET /providers` — list configured providers without exposing api_key.
pub async fn list_providers(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let cfg = crate::config::read(&state.config).clone();
    let summaries: Vec<ProviderSummary> = cfg
        .providers
        .into_iter()
        .map(|p| ProviderSummary { id: p.id, name: p.name, base_url: p.base_url })
        .collect();
    Ok(Json(summaries))
}

/// `GET /models` — aggregate every provider's `/models` into one flat list.
///
/// Failing or slow providers are silently skipped; partial results are always
/// returned.
pub async fn list_models(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let cfg = crate::config::read(&state.config).clone();

    let futures = cfg.providers.iter().cloned().map(|p| fetch_models_for(state.http.clone(), p));
    let per_provider = join_all(futures).await;

    let mut out: Vec<ModelEntry> = Vec::new();
    for entries in per_provider.into_iter().flatten() {
        out.extend(entries);
    }

    Ok(Json(out))
}

/// Hit one provider's `GET {base_url}/models` and turn the response into
/// `ModelEntry`s. Returns `None` (logged) on any failure.
async fn fetch_models_for(http: reqwest::Client, provider: Provider) -> Option<Vec<ModelEntry>> {
    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
    let result = async {
        let resp = http
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", provider.api_key))
            .header(reqwest::header::USER_AGENT, HeaderValue::from_static("monel-gateway/0.1"))
            .timeout(MODELS_FANOUT_TIMEOUT)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("upstream returned status {}", resp.status());
        }

        // OpenAI shape: { "data": [{ "id": "..." }, ...] }. Be tolerant: accept
        // either an object-with-data or a bare array.
        let value: serde_json::Value = resp.json().await?;
        let arr: Vec<serde_json::Value> = match &value {
            serde_json::Value::Object(map) if matches!(map.get("data"), Some(serde_json::Value::Array(_))) => {
                match map.get("data") {
                    Some(serde_json::Value::Array(a)) => a.clone(),
                    _ => Vec::new(),
                }
            }
            serde_json::Value::Array(a) => a.clone(),
            _ => Vec::new(),
        };

        let mut entries = Vec::with_capacity(arr.len());
        for item in arr {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                entries.push(ModelEntry {
                    provider_id: provider.id.clone(),
                    model: id.to_string(),
                    name: provider.name.clone(),
                });
            }
        }
        Ok::<Vec<ModelEntry>, anyhow::Error>(entries)
    }
    .await;

    match result {
        Ok(entries) => Some(entries),
        Err(e) => {
            tracing::warn!(provider = %provider.id, error = %e, "skipping provider in /models fan-out");
            None
        }
    }
}
