//! gateway — a lightweight, transparent OpenAI-compatible API gateway.
//!
//! Usage:
//!     gateway server [--config <path>]   run the always-on proxy server
//!     gateway ui                          (placeholder; Slint GUI not yet implemented)
//!     gateway --help

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{middleware, Router};
use notify::{event::EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

mod admin;
mod aggregator;
mod auth;
mod config;
mod error;
mod proxy;
mod state;

use config::{Config, SharedConfig};
use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,tower_http=warn")),
        )
        .init();

    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("server") => run_server(args).await,
        Some("ui") => {
            println!("GUI not yet implemented. Run `gateway server` to start the proxy.");
            Ok(())
        }
        Some("--help") | Some("-h") | None => {
            print_usage();
            Ok(())
        }
        Some(other) => {
            eprintln!("unknown subcommand: {other}");
            print_usage();
            std::process::exit(2);
        }
    }
}

fn print_usage() {
    eprintln!(
        "Usage:\n  gateway server [--config <path>]\n  gateway ui\n  gateway --help"
    );
}

/// Parse `--config <path>` (default `./config.yaml`).
fn parse_server_args(mut args: std::iter::Skip<std::env::Args>) -> PathBuf {
    let mut config_path = PathBuf::from("config.yaml");
    while let Some(arg) = args.next() {
        if arg == "--config" {
            if let Some(v) = args.next() {
                config_path = PathBuf::from(v);
            } else {
                eprintln!("--config requires a value");
                std::process::exit(2);
            }
        } else {
            eprintln!("ignoring unknown arg: {arg}");
        }
    }
    config_path
}

async fn run_server(args: std::iter::Skip<std::env::Args>) -> Result<()> {
    let config_path = parse_server_args(args);

    let initial = Config::load(&config_path)
        .with_context(|| format!("could not load config from {}", config_path.display()))?;
    let shared: SharedConfig = config::new_shared(initial);

    // Shared HTTP client: pooled, with sane defaults.
    let http = reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .context("failed to build reqwest client")?;

    let state = AppState::new(shared.clone(), http, config_path.clone());

    // Hot reload watcher.
    spawn_config_watcher(shared.clone(), config_path.clone());

    let app = build_router(state.clone());

    let (host, port) = {
        let cfg = config::read(&shared);
        (cfg.server.host.clone(), cfg.server.port)
    };
    let addr: std::net::SocketAddr = format!("{host}:{port}")
        .parse()
        .with_context(|| format!("invalid bind address {host}:{port}"))?;

    tracing::info!("gateway listening on http://{addr}");
    tracing::info!("config file: {}", config_path.display());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    tracing::info!("shutdown complete");
    Ok(())
}

fn build_router(state: AppState) -> Router {
    // Protected routes share the auth middleware.
    let protected = Router::<AppState>::new()
        .route("/chat/:provider_id/*path", axum::routing::any(proxy::proxy))
        .route("/models", axum::routing::get(aggregator::list_models))
        .route("/providers", axum::routing::get(aggregator::list_providers))
        .route("/admin/config", axum::routing::get(admin::get_config).post(admin::post_config))
        .route("/admin/reload", axum::routing::post(admin::reload))
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_auth));

    Router::<AppState>::new()
        // Health is public (no auth).
        .route("/health", axum::routing::get(|| async { "ok" }))
        .merge(protected)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Spawn a background task that hot-reloads `config.yaml` on change.
fn spawn_config_watcher(shared: SharedConfig, config_path: PathBuf) {
    let watched_file = config::canonicalize(&config_path);

    // Buffer + debounce file events. Editors often fire several events per save.
    let (tx, rx) = tokio::sync::mpsc::channel::<()>(32);

    match build_watcher(tx, watched_file, &config_path) {
        Ok(watcher) => {
            tokio::spawn(async move {
                // Hold the watcher for the lifetime of the task.
                let _watcher = watcher;
                run_reload_loop(shared, config_path, rx).await;
            });
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "config file-watcher disabled; hot reload via edit unavailable \
                 (POST /admin/reload still works)"
            );
            tokio::spawn(run_reload_loop(shared, config_path, rx));
        }
    }
}

fn build_watcher(
    tx: tokio::sync::mpsc::Sender<()>,
    watched_file: PathBuf,
    config_path: &Path,
) -> Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            let relevant = matches!(
                ev.kind,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
            );
            if !relevant {
                return;
            }
            if ev.paths.iter().any(|p| same_file(p, &watched_file)) {
                // Best-effort; a dropped signal here is harmless thanks to debounce.
                let _ = tx.try_send(());
            }
        }
    })
    .context("failed to create file watcher")?;

    // Watch the parent dir so rename-based saves (atomic write) fire correctly.
    let watch_dir = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    watcher
        .watch(&watch_dir, RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch {}", watch_dir.display()))?;

    Ok(watcher)
}

/// Consumer loop: debounces bursts, reloads config, keeps the previous one on error.
async fn run_reload_loop(shared: SharedConfig, config_path: PathBuf, mut rx: tokio::sync::mpsc::Receiver<()>) {
    while rx.recv().await.is_some() {
        // Debounce: collapse bursts into a single reload after 300ms of quiet.
        tokio::time::sleep(Duration::from_millis(300)).await;
        // Drain any coalesced follow-up signals.
        while rx.try_recv().is_ok() {}

        match Config::load(&config_path) {
            Ok(new_cfg) => {
                let changed = {
                    let cur = config::read(&shared);
                    !config_equal(&cur, &new_cfg)
                };
                if changed {
                    config::replace(&shared, new_cfg);
                    tracing::info!("config reloaded from {}", config_path.display());
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "config reload skipped: parse/IO error, keeping previous config"
                );
            }
        }
    }
}

/// Cheap structural equality for "did the config actually change?"
fn config_equal(a: &Config, b: &Config) -> bool {
    a.server.host == b.server.host
        && a.server.port == b.server.port
        && a.server.auth_key == b.server.auth_key
        && a.providers.len() == b.providers.len()
        && a.providers.iter().zip(b.providers.iter()).all(|(x, y)| {
            x.id == y.id
                && x.name == y.name
                && x.base_url == y.base_url
                && x.api_key == y.api_key
        })
}

/// Compare two paths, tolerating canonicalization differences.
fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    a.canonicalize().ok() == b.canonicalize().ok()
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
