//! Shared application state passed to all handlers.

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::SharedConfig;

/// Clonable, cheap state available to every handler.
#[derive(Clone)]
pub struct AppState {
    pub config: SharedConfig,
    pub http: reqwest::Client,
    pub config_path: Arc<PathBuf>,
}

impl AppState {
    pub fn new(config: SharedConfig, http: reqwest::Client, config_path: PathBuf) -> Self {
        Self { config, http, config_path: Arc::new(config_path) }
    }

    /// Current config snapshot (cheap clone under a read lock).
    #[allow(dead_code)]
    pub fn snapshot(&self) -> crate::config::Config {
        crate::config::read(&self.config).clone()
    }
}
