//! Admin endpoints (protected by the same global auth key as the proxy).
//!
//!   GET  /admin/config   -> return full config (incl. api_keys)
//!   POST /admin/config   -> replace config, persist to disk, hot-reload live
//!   POST /admin/reload   -> re-read config.yaml and hot-reload live

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// `GET /admin/config`.
pub async fn get_config(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let cfg = crate::config::read(&state.config).clone();
    Ok(Json(cfg))
}

/// `POST /admin/config` with a JSON body.
pub async fn post_config(
    State(state): State<AppState>,
    Json(new_config): Json<Config>,
) -> AppResult<impl IntoResponse> {
    new_config.validate().map_err(|e| AppError::bad_request(e.to_string()))?;

    // Persist first. If it fails, do not touch the live config.
    if let Err(e) = new_config.save(state.config_path.as_ref()) {
        tracing::error!(error = %e, "admin: failed to persist config");
        return Err(AppError::internal(format!("failed to persist config: {e}")));
    }

    // Update the live config. The file-watcher will also fire and reload,
    // but applying immediately means the change is visible without waiting.
    crate::config::replace(&state.config, new_config.clone());
    tracing::info!("admin: config updated");

    Ok((StatusCode::OK, Json(new_config)))
}

/// `POST /admin/reload`.
pub async fn reload(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let reloaded = Config::load(state.config_path.as_ref())
        .map_err(|e| AppError::internal(format!("reload failed: {e}")))?;

    crate::config::replace(&state.config, reloaded.clone());
    tracing::info!("admin: config reloaded from disk");

    Ok((StatusCode::OK, Json(reloaded)))
}
