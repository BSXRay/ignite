use crate::protocol::Packet;
use crate::pterodactyl;
use crate::AppState;
use log::{error, info, warn};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};

#[derive(Debug)]
pub struct SyncCommand {
    pub action: crate::protocol::SyncAction,
    pub source: String,
}

pub struct Engine {
    state: AppState,
    ptero: pterodactyl::Client,
    current_active: String,
    is_rebooting: bool,
}

impl Engine {
    pub fn new(state: AppState, ptero: pterodactyl::Client) -> Self {
        Self {
            state,
            ptero,
            current_active: "server_a".into(),
            is_rebooting: false,
        }
    }

    pub async fn handle_command(&mut self, cmd: SyncCommand) -> anyhow::Result<()> {
        match cmd.action {
            crate::protocol::SyncAction::PrepareReboot { target_server, reason } => {
                info!("=== Reboot eingeleitet: {} (Grund: {}) ===", target_server, reason);
                self.execute_reboot(&reason).await?;
            }
            crate::protocol::SyncAction::ActivateServer { server_id } => {
                info!("=== Server-Wechsel zu {} ===", server_id);
                self.switch_active(&server_id).await?;
            }
            crate::protocol::SyncAction::RecoverMain { server_id } => {
                info!("=== Recovery von {} eingeleitet ===", server_id);
                self.execute_recovery(&server_id).await?;
            }
            _ => {
                warn!("Ignoriere SyncAction: {:?}", cmd.action);
            }
        }
        Ok(())
    }

    async fn execute_reboot(&mut self, _reason: &str) -> anyhow::Result<()> {
        if self.is_rebooting {
            warn!("Reboot läuft bereits, ignoriere");
            return Ok(());
        }
        self.is_rebooting = true;

        info!("Phase 1: Server B wird gestartet...");
        self.ptero.start_server(self.ptero.server_b_id()).await?;

        info!("Warte auf Server B (running)...");
        let started = self.ptero
            .wait_for_status(
                self.ptero.server_b_id(),
                &["running"],
                self.state.config.servers.health_check_max_retries,
                self.state.config.servers.health_check_interval_secs,
            )
            .await?;

        if !started {
            error!("Server B wurde nicht rechtzeitig gestartet!");
            self.is_rebooting = false;
            return Err(anyhow::anyhow!("Server B start nicht rechtzeitig"));
        }

        info!("Phase 2: Warte auf Backup-Agent von Server B...");
        sleep(Duration::from_secs(10)).await;

        let latest_backup = self.find_latest_backup().await;
        if let Some(backup_id) = latest_backup {
            info!("Phase 3: Sende letztes Backup ({}) an Server B", backup_id);
            self.send_backup_to_server("server_b", &backup_id).await?;
        }

        info!("Phase 4: Inkrementeller Sync von A nach B...");
        self.sync_incremental("server_a", "server_b").await?;

        info!("Phase 5: Aktiviere Server B als Ziel...");
        self.switch_active("server_b").await?;

        info!("Phase 6: Graceful Shutdown von Server A...");
        self.graceful_shutdown_a().await?;

        info!("Phase 7: Warte auf Server A offline...");
        let stopped = self.ptero
            .wait_for_status(
                self.ptero.server_a_id(),
                &["offline", "stopped"],
                self.state.config.servers.health_check_max_retries,
                self.state.config.servers.health_check_interval_secs,
            )
            .await?;

        if stopped {
            info!("Server A ist offline.");
        }

        self.is_rebooting = false;
        self.current_active = "server_b".into();
        info!("=== Reboot abgeschlossen. Server B ist aktiv. ===");
        Ok(())
    }

    async fn execute_recovery(&mut self, _server_id: &str) -> anyhow::Result<()> {
        if self.is_rebooting {
            warn!("Reboot läuft noch, ignoriere Recovery");
            return Ok(());
        }

        info!("Recovery: Starte Server A...");
        self.ptero.start_server(self.ptero.server_a_id()).await?;

        info!("Warte auf Server A (running)...");
        let started = self.ptero
            .wait_for_status(
                self.ptero.server_a_id(),
                &["running"],
                self.state.config.servers.health_check_max_retries,
                self.state.config.servers.health_check_interval_secs,
            )
            .await?;

        if !started {
            error!("Server A wurde nicht rechtzeitig gestartet!");
            return Err(anyhow::anyhow!("Server A start nicht rechtzeitig"));
        }

        sleep(Duration::from_secs(10)).await;

        info!("Recovery: Sync von B nach A...");
        self.sync_incremental("server_b", "server_a").await?;

        info!("Recovery: Aktiviere Server A...");
        self.switch_active("server_a").await?;

        info!("Recovery: Stoppe Server B...");
        self.ptero.stop_server(self.ptero.server_b_id()).await?;

        self.current_active = "server_a".into();
        info!("=== Recovery abgeschlossen. Server A ist wieder aktiv. ===");
        Ok(())
    }

    async fn find_latest_backup(&self) -> Option<String> {
        let backup_dir = &self.state.config.backup.data_dir;
        if !backup_dir.exists() {
            return None;
        }

        let mut entries: Vec<_> = std::fs::read_dir(backup_dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("backup.tar.gz").exists())
            .collect();

        entries.sort_by(|a, b| {
            let a_time = a.path().metadata().ok().and_then(|m| m.modified().ok());
            let b_time = b.path().metadata().ok().and_then(|m| m.modified().ok());
            a_time.cmp(&b_time)
        });
        entries.last().map(|e| {
            e.file_name().to_string_lossy().to_string()
        })
    }

    async fn send_backup_to_server(&self, _server_id: &str, backup_id: &str) -> anyhow::Result<()> {
        let backup_path = self.state.config.backup.data_dir.join(backup_id).join("backup.tar.gz");
        let data = std::fs::read(&backup_path)?;

        let servers = self.state.servers.read().await;
        let target = servers.iter().find(|s| s.id == "server_b" || s.id == "server_b");

        if let Some(server) = target {
            info!("Sende Backup {} an {} ({} bytes)", backup_id, server.addr, data.len());

            if let Ok(mut stream) = TcpStream::connect(&server.addr).await {
                let packet = Packet::SyncData {
                    session_id: "master-sync".into(),
                    data_type: crate::protocol::SyncDataType::WorldData,
                    data,
                };

                let payload = serde_json::to_vec(&packet)?;
                let len = (payload.len() as u64).to_be_bytes();
                stream.write_all(&len).await?;
                stream.write_all(&payload).await?;

                info!("Backup an Server B gesendet");
            }
        } else {
            warn!("Server B nicht verbunden, überspringe Backup-Transfer");
        }

        Ok(())
    }

    async fn sync_incremental(&self, source: &str, target: &str) -> anyhow::Result<()> {
        info!("Inkrementeller Sync von {} nach {}...", source, target);

        let servers = self.state.servers.read().await;
        let target_server = servers.iter().find(|s| s.id == target);

        if let Some(server) = target_server {
            let command = crate::protocol::SyncCommand {
                action: crate::protocol::SyncAction::StartSync {
                    source: source.into(),
                    target: target.into(),
                },
            };
            let packet = Packet::SyncMessage { command };
            let payload = serde_json::to_vec(&packet)?;

            if let Ok(mut stream) = TcpStream::connect(&server.addr).await {
                let len = (payload.len() as u64).to_be_bytes();
                stream.write_all(&len).await?;
                stream.write_all(&payload).await?;
                info!("Sync-Command an {} gesendet", target);
            }

            info!("Warte {}s auf Sync...", self.state.config.servers.sync_timeout_secs);
            sleep(Duration::from_secs(self.state.config.servers.sync_timeout_secs)).await;
            info!("Inkrementeller Sync abgeschlossen");
        }

        Ok(())
    }

    async fn switch_active(&mut self, server_id: &str) -> anyhow::Result<()> {
        info!("Wechsle aktiven Server zu {}", server_id);

        let new_target = match server_id {
            "server_b" => &self.state.config.proxy.target_b,
            _ => &self.state.config.proxy.target_a,
        };

        {
            let mut target = self.state.proxy_target.write().await;
            *target = new_target.clone();
            info!("Proxy-Ziel geändert zu {}", new_target);
        }

        self.current_active = server_id.to_string();

        // Plugin auf dem Ziel-Server informieren
        let plugin_cmd = crate::protocol::SyncCommand {
            action: crate::protocol::SyncAction::ActivateServer {
                server_id: server_id.to_string(),
            },
        };
        let packet = Packet::SyncMessage { command: plugin_cmd };

        let servers = self.state.servers.read().await;
        for s in servers.iter() {
            if s.id == server_id {
                let payload = serde_json::to_vec(&packet)?;
                if let Ok(mut stream) = TcpStream::connect(&s.addr).await {
                    let len = (payload.len() as u64).to_be_bytes();
                    stream.write_all(&len).await?;
                    stream.write_all(&payload).await?;
                    info!("ActivateSignal an {} gesendet", server_id);
                }
            }
        }

        info!("Server-Wechsel abgeschlossen");
        Ok(())
    }

    async fn graceful_shutdown_a(&self) -> anyhow::Result<()> {
        info!("Leite gracefull Shutdown von Server A ein...");

        let servers = self.state.servers.read().await;
        for s in servers.iter() {
            if s.id == "server_a" {
                let packet = Packet::ShutdownNotice {
                    reason: "Reboot durch Ignite Master".into(),
                    grace_period_secs: self.state.config.servers.graceful_shutdown_secs,
                };
                let payload = serde_json::to_vec(&packet)?;
                if let Ok(mut stream) = TcpStream::connect(&s.addr).await {
                    let len = (payload.len() as u64).to_be_bytes();
                    stream.write_all(&len).await?;
                    stream.write_all(&payload).await?;
                    info!("ShutdownNotice an Server A gesendet");
                }
            }
        }

        sleep(Duration::from_secs(self.state.config.servers.graceful_shutdown_secs)).await;

        self.ptero.stop_server(self.ptero.server_a_id()).await?;
        info!("Server A gestoppt via Pterodactyl");

        // Nach Shutdown von A: Server B wird zum Proxy-Target A
        // Damit B die gleiche Rolle wie A übernimmt
        info!("Server B ist jetzt aktiv. Warte auf Server A Neustart...");

        Ok(())
    }
}
