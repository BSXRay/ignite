mod config;
mod protocol;
mod server;
mod pterodactyl;
mod sync;

use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use log::{info, error};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<config::Config>,
    pub servers: Arc<RwLock<Vec<server::ServerInfo>>>,
    pub sync_tx: mpsc::Sender<sync::SyncCommand>,
    pub proxy_target: Arc<RwLock<String>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    )
    .format_timestamp_millis()
    .init();

    info!("=== Ignite Master v{} ===", "1.0.0");

    let cfg = config::load()?;
    info!("Konfiguration geladen: {:?}", cfg);

    let (sync_tx, mut sync_rx) = mpsc::channel::<sync::SyncCommand>(256);
    let proxy_target: Arc<RwLock<String>> = Arc::new(RwLock::new(cfg.proxy.target_a.clone()));

    let state = AppState {
        config: Arc::new(cfg.clone()),
        servers: Arc::new(RwLock::new(Vec::new())),
        sync_tx,
        proxy_target,
    };

    let ptero = pterodactyl::Client::new(&cfg.pterodactyl);
    let mut sync_engine = sync::Engine::new(state.clone(), ptero);

    let backup_state = state.clone();
    let _backup_handle = tokio::spawn(async move {
        server::run_backup_server(backup_state).await.unwrap_or_else(|e| {
            error!("Backup-Server abgestürzt: {}", e);
        });
    });

    let proxy_state = state.clone();
    let _proxy_handle = tokio::spawn(async move {
        server::run_proxy(proxy_state).await.unwrap_or_else(|e| {
            error!("Proxy abgestürzt: {}", e);
        });
    });

    let api_state = state.clone();
    let _api_handle = tokio::spawn(async move {
        server::run_api_server(api_state).await.unwrap_or_else(|e| {
            error!("API-Server abgestürzt: {}", e);
        });
    });

    let health_state = state.clone();
    let _health_handle = tokio::spawn(async move {
        server::run_health_monitor(health_state).await.unwrap_or_else(|e| {
            error!("Health-Monitor abgestürzt: {}", e);
        });
    });

    while let Some(cmd) = sync_rx.recv().await {
        if let Err(e) = sync_engine.handle_command(cmd).await {
            error!("Sync-Fehler: {}", e);
        }
    }

    Ok(())
}
