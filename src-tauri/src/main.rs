// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::path::PathBuf;
use std::time::Duration;
use tauri::Manager;
use tokio::sync::RwLock;

use gateway_lib::config::{self, Config};
use gateway_lib::state::AppState;
use gateway_lib::stats::Stats;
use gateway_lib::{admin, aggregator, auth, proxy, stats};

use axum::{middleware, Router};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;

// Internal token for Tauri frontend
#[derive(Clone)]
struct InternalToken(Arc<String>);

// Tauri command to get internal token
#[tauri::command]
fn get_internal_token(token: tauri::State<InternalToken>) -> String {
    token.0.as_ref().clone()
}

#[tauri::command]
async fn get_server_status() -> Result<ServerStatus, String> {
    Ok(ServerStatus {
        running: true,
        address: "http://127.0.0.1:7890".to_string(),
    })
}

#[derive(serde::Serialize)]
struct ServerStatus {
    running: bool,
    address: String,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            // Generate internal token
            let internal_token = Arc::new(format!("tauri-internal-{}", uuid::Uuid::new_v4()));
            tracing::info!("Generated internal token for Tauri frontend");

            let token_clone = internal_token.clone();

            // Start backend server in background
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_backend_server(token_clone).await {
                    tracing::error!("Backend server error: {}", e);
                }
            });

            // Store internal token in Tauri state
            app.manage(InternalToken(internal_token));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_internal_token,
            get_server_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn start_backend_server(internal_token: Arc<String>) -> anyhow::Result<()> {
    // Config is in project root (parent of src-tauri)
    let config_path = PathBuf::from("../config.yaml");

    let initial = Config::load(&config_path)
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to load config: {}, using default", e);
            Config::default()
        });
    let shared = config::new_shared(initial);

    let http = reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(10))
        .build()?;

    let stats_tracker = Arc::new(RwLock::new(Stats::new()));

    let state = AppState::new(
        shared.clone(),
        http,
        config_path,
        internal_token,
        stats_tracker,
    );

    let app = build_router(state);

    let addr: std::net::SocketAddr = "127.0.0.1:7890".parse()?;
    tracing::info!("Backend server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn build_router(state: AppState) -> Router {
    let protected = Router::<AppState>::new()
        .route("/chat/:provider_id/*path", axum::routing::any(proxy::proxy))
        .route("/models", axum::routing::get(aggregator::list_models))
        .route("/providers", axum::routing::get(aggregator::list_providers))
        .route("/admin/config", axum::routing::get(admin::get_config).post(admin::post_config))
        .route("/admin/reload", axum::routing::post(admin::reload))
        .route("/admin/stats", axum::routing::get(stats::get_stats))
        .route("/admin/logs", axum::routing::get(stats::get_logs))
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_auth));

    Router::<AppState>::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
        .merge(protected)
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .fallback_service(ServeDir::new("../ui"))
}