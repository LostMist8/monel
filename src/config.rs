//! Configuration types, persistence, and the shared mutable config handle.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Process-wide shared, hot-reloadable configuration.
pub type SharedConfig = Arc<RwLock<Config>>;

/// Convenience constructor.
pub fn new_shared(config: Config) -> SharedConfig {
    Arc::new(RwLock::new(config))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub providers: Vec<Provider>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub auth_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    7890
}

impl Config {
    /// Read and parse a YAML config file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file: {}", path.as_ref().display()))?;
        let config: Config = serde_yaml::from_str(&raw).context("failed to parse config YAML")?;
        config.validate()?;
        Ok(config)
    }

    /// Create a default configuration
    pub fn default() -> Self {
        Self {
            server: ServerConfig {
                host: default_host(),
                port: default_port(),
                auth_key: String::new(),
            },
            providers: Vec::new(),
        }
    }

    /// Write the config to disk atomically (write to temp file, then rename).
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let yaml = serde_yaml::to_string(self).context("failed to serialize config YAML")?;

        // Write to a sibling temp file then rename, so a partial write never
        // replaces the real config mid-flight.
        let mut tmp = path.to_path_buf();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        tmp.set_extension(format!("tmp.{ts}"));
        std::fs::write(&tmp, yaml)
            .with_context(|| format!("failed to write temp config: {}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .with_context(|| format!("failed to move config into place: {}", path.display()))?;
        Ok(())
    }

    /// Validate structural invariants.
    pub fn validate(&self) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        for p in &self.providers {
            if p.id.trim().is_empty() {
                anyhow::bail!("provider has empty id");
            }
            if !seen.insert(p.id.as_str()) {
                anyhow::bail!("duplicate provider id: {}", p.id);
            }
            if p.base_url.trim().is_empty() {
                anyhow::bail!("provider '{}' has empty base_url", p.id);
            }
        }
        Ok(())
    }

    /// Find a provider by id (cheap clone).
    pub fn find_provider(&self, id: &str) -> Option<Provider> {
        self.providers.iter().find(|p| p.id == id).cloned()
    }
}

/// Read the current config under a read lock. Panics only on lock poisoning.
pub fn read(shared: &SharedConfig) -> std::sync::RwLockReadGuard<'_, Config> {
    shared.read().expect("config lock poisoned")
}

/// Replace the live config under a write lock.
pub fn replace(shared: &SharedConfig, config: Config) {
    let mut guard = shared.write().expect("config lock poisoned");
    *guard = config;
}

/// Path canonicalized for watcher event matching (falls back to input on failure).
pub fn canonicalize(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref().canonicalize().unwrap_or_else(|_| path.as_ref().to_path_buf())
}
