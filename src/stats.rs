//! Statistics and logging module for tracking API usage

use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLog {
    pub timestamp: u64,
    pub provider: String,
    pub model: String,
    pub status_code: u16,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStats {
    pub request_count: u64,
    pub total_duration_ms: u64,
    pub avg_duration_ms: u64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
    pub error_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStats {
    pub request_count: u64,
    pub total_duration_ms: u64,
    pub avg_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsResponse {
    pub providers: HashMap<String, ProviderStats>,
    pub models: HashMap<String, ModelStats>,
    pub total_requests: u64,
    pub total_errors: u64,
}

#[derive(Debug)]
pub struct Stats {
    logs: VecDeque<RequestLog>,
    provider_stats: HashMap<String, ProviderStats>,
    model_stats: HashMap<String, ModelStats>,
    total_requests: u64,
    total_errors: u64,
    max_logs: usize,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            logs: VecDeque::new(),
            provider_stats: HashMap::new(),
            model_stats: HashMap::new(),
            total_requests: 0,
            total_errors: 0,
            max_logs: 1000,
        }
    }

    pub fn record(&mut self, log: RequestLog) {
        self.total_requests += 1;

        if log.error.is_some() || log.status_code >= 400 {
            self.total_errors += 1;
        }

        // Update provider stats
        let provider_stat =
            self.provider_stats
                .entry(log.provider.clone())
                .or_insert(ProviderStats {
                    request_count: 0,
                    total_duration_ms: 0,
                    avg_duration_ms: 0,
                    min_duration_ms: u64::MAX,
                    max_duration_ms: 0,
                    error_count: 0,
                });

        provider_stat.request_count += 1;
        provider_stat.total_duration_ms += log.duration_ms;
        provider_stat.avg_duration_ms =
            provider_stat.total_duration_ms / provider_stat.request_count;
        provider_stat.min_duration_ms = provider_stat.min_duration_ms.min(log.duration_ms);
        provider_stat.max_duration_ms = provider_stat.max_duration_ms.max(log.duration_ms);

        if log.error.is_some() || log.status_code >= 400 {
            provider_stat.error_count += 1;
        }

        // Update model stats
        let model_stat = self
            .model_stats
            .entry(log.model.clone())
            .or_insert(ModelStats {
                request_count: 0,
                total_duration_ms: 0,
                avg_duration_ms: 0,
            });

        model_stat.request_count += 1;
        model_stat.total_duration_ms += log.duration_ms;
        model_stat.avg_duration_ms = model_stat.total_duration_ms / model_stat.request_count;

        // Add to logs (keep last N)
        self.logs.push_back(log);
        if self.logs.len() > self.max_logs {
            self.logs.pop_front();
        }
    }

    pub fn get_stats(&self) -> StatsResponse {
        StatsResponse {
            providers: self.provider_stats.clone(),
            models: self.model_stats.clone(),
            total_requests: self.total_requests,
            total_errors: self.total_errors,
        }
    }

    pub fn get_logs(&self, limit: usize) -> Vec<RequestLog> {
        let start = if self.logs.len() > limit {
            self.logs.len() - limit
        } else {
            0
        };
        self.logs.iter().skip(start).cloned().collect()
    }
}

/// GET /admin/stats - Get usage statistics
pub async fn get_stats(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let stats = state.stats.read().await;
    Ok(Json(stats.get_stats()))
}

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    100
}

/// GET /admin/logs?limit=100 - Get recent request logs
pub async fn get_logs(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<LogsQuery>,
) -> AppResult<impl IntoResponse> {
    let stats = state.stats.read().await;
    Ok(Json(stats.get_logs(query.limit)))
}

/// Helper to record a request in statistics
pub async fn record_request(
    stats: Arc<RwLock<Stats>>,
    provider: String,
    model: String,
    status_code: u16,
    duration_ms: u64,
    error: Option<String>,
    usage: Option<crate::proxy::TokenUsage>,
) {
    let (input_tokens, output_tokens, cache_read_input_tokens) = match usage {
        Some(u) => (
            Some(u.input_tokens),
            Some(u.output_tokens),
            Some(u.cache_read_input_tokens),
        ),
        None => (None, None, None),
    };

    let log = RequestLog {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        provider,
        model,
        status_code,
        duration_ms,
        error,
        input_tokens,
        output_tokens,
        cache_read_input_tokens,
    };

    let mut stats = stats.write().await;
    stats.record(log);
}
