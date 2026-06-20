//! Shared application state passed to all handlers.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::SharedConfig;
use crate::stats::Stats;

/// Clonable, cheap state available to every handler.
#[derive(Clone)]
pub struct AppState {
    pub config: SharedConfig,
    pub http: reqwest::Client,
    pub config_path: Arc<PathBuf>,
    /// Internal token for Tauri frontend (bypasses auth)
    pub internal_token: Arc<String>,
    /// Statistics tracker
    pub stats: Arc<RwLock<Stats>>,
}

impl AppState {
    pub fn new(
        config: SharedConfig,
        http: reqwest::Client,
        config_path: PathBuf,
        internal_token: Arc<String>,
        stats: Arc<RwLock<Stats>>,
    ) -> Self {
        Self {
            config,
            http,
            config_path: Arc::new(config_path),
            internal_token,
            stats,
        }
    }

    /// Current config snapshot (cheap clone under a read lock).
    #[allow(dead_code)]
    pub async fn snapshot(&self) -> crate::config::Config {
        crate::config::read(&self.config).await.clone()
    }
}
