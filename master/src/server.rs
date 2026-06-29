use crate::protocol::*;
use crate::AppState;
use log::{error, info, warn};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub id: String,
    pub server_type: ServerType,
    pub addr: String,
    pub session_id: String,
    pub last_health: u64,
    pub active: bool,
    pub players_online: u32,
}

pub struct BackupSession {
    pub server_id: String,
    pub session_id: String,
    pub expected_chunks: u64,
    pub received_chunks: u64,
    pub total_size: u64,
    pub buffer: Vec<u8>,
}

pub async fn run_backup_server(state: AppState) -> anyhow::Result<()> {
    let addr = &state.config.backup.listen_addr;
    let listener = TcpListener::bind(addr).await?;
    info!("Backup-Server gestartet auf {}", addr);

    let sessions: Arc<RwLock<HashMap<String, BackupSession>>> =
        Arc::new(RwLock::new(HashMap::new()));

    loop {
        let (mut stream, peer) = listener.accept().await?;
        info!("Neue Backup-Verbindung von {}", peer);

        let state = state.clone();
        let sessions = sessions.clone();

        tokio::spawn(async move {
            let mut buf = Vec::with_capacity(8192);
            let mut header_buf = [0u8; 8];

            loop {
                match timeout(Duration::from_secs(30), stream.read_exact(&mut header_buf)).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        error!("Lese Fehler von {}: {}", peer, e);
                        break;
                    }
                    Err(_) => {
                        warn!("Timeout für {}, Verbindung geschlossen", peer);
                        break;
                    }
                }

                let data_len = u64::from_be_bytes(header_buf) as usize;
                if data_len > 100 * 1024 * 1024 {
                    error!("Packet zu groß von {}: {} bytes", peer, data_len);
                    break;
                }

                buf.resize(data_len, 0);
                if let Err(e) = stream.read_exact(&mut buf).await {
                    error!("Fehler beim Lesen von Packet von {}: {}", peer, e);
                    break;
                }

                let packet: Packet = match bincode_deserialize(&buf) {
                    Ok(p) => p,
                    Err(e) => {
                        error!("Deserialize Fehler von {}: {}", peer, e);
                        continue;
                    }
                };

                let response = handle_backup_packet(&state, &sessions, peer, packet).await;

                if let Some(resp) = response {
                    let data = bincode_serialize(&resp);
                    let len = (data.len() as u64).to_be_bytes();
                    if let Err(e) = stream.write_all(&len).await {
                        error!("Schreib Fehler an {}: {}", peer, e);
                        break;
                    }
                    if let Err(e) = stream.write_all(&data).await {
                        error!("Schreib Fehler an {}: {}", peer, e);
                        break;
                    }
                }
            }

            info!("Verbindung zu {} geschlossen", peer);
        });
    }
}

async fn handle_backup_packet(
    state: &AppState,
    sessions: &Arc<RwLock<HashMap<String, BackupSession>>>,
    peer: std::net::SocketAddr,
    packet: Packet,
) -> Option<Packet> {
    match packet {
        Packet::Handshake { server_id, server_type, version } => {
            info!("Handshake von {} (Typ: {:?}, Version: {})", server_id, server_type, version);
            let session_id = Uuid::new_v4().to_string();
            let server_info = ServerInfo {
                id: server_id.clone(),
                server_type,
                addr: peer.to_string(),
                session_id: session_id.clone(),
                last_health: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                active: true,
                players_online: 0,
            };

            let mut servers = state.servers.write().await;
            servers.retain(|s| s.id != server_id);
            servers.push(server_info);

            Some(Packet::HandshakeAck {
                session_id,
                backup_interval_secs: state.config.servers.backup_interval_secs,
            })
        }
        Packet::BackupStart { session_id, backup_id, total_size, chunk_count } => {
            info!("BackupStart: {} (size: {}, chunks: {})", backup_id, total_size, chunk_count);
            let mut sess = sessions.write().await;
            sess.insert(backup_id.clone(), BackupSession {
                server_id: String::new(),
                session_id,
                expected_chunks: chunk_count,
                received_chunks: 0,
                total_size,
                buffer: Vec::with_capacity(total_size as usize),
            });
            Some(Packet::BackupAck {
                backup_id,
                status: BackupStatus::Accepted,
            })
        }
        Packet::BackupChunk { session_id: _, backup_id, chunk_index, data, checksum } => {
            let hash = Sha256::digest(&data);
            let hash_hex = hex::encode(hash);
            if hash_hex != checksum {
                warn!("Checksum Fehler in chunk {} von {}", chunk_index, backup_id);
                return Some(Packet::Error {
                    code: 1001,
                    message: "Checksum mismatch".into(),
                });
            }

            let mut sess = sessions.write().await;
            if let Some(session) = sess.get_mut(&backup_id) {
                session.received_chunks += 1;
                session.buffer.extend_from_slice(&data);
                info!("Chunk {}/{} für {} empfangen", chunk_index, session.expected_chunks, backup_id);
            }
            None
        }
        Packet::BackupComplete { session_id: _, backup_id, checksum: _ } => {
            info!("BackupComplete: {}", backup_id);
            let buffer = {
                let mut sess = sessions.write().await;
                sess.remove(&backup_id).map(|s| s.buffer)
            };

            if let Some(data) = buffer {
                let backup_dir = state.config.backup.data_dir.join(&backup_id);
                if let Err(_e) = std::fs::create_dir_all(&backup_dir) {
                    return Some(Packet::BackupAck {
                        backup_id,
                        status: BackupStatus::StorageError,
                    });
                }

                let tar_path = backup_dir.join("backup.tar.gz");
                std::fs::write(&tar_path, &data).ok();

                info!("Backup {} gespeichert unter {:?}", backup_id, tar_path);

                Some(Packet::BackupAck {
                    backup_id,
                    status: BackupStatus::Accepted,
                })
            } else {
                Some(Packet::BackupAck {
                    backup_id,
                    status: BackupStatus::StorageError,
                })
            }
        }
        Packet::HealthCheck { .. } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let mut servers = state.servers.write().await;
            if let Some(s) = servers.iter_mut().find(|s| s.addr == peer.to_string()) {
                s.last_health = now;
            }
            None
        }
        _ => {
            warn!("Unbekanntes Packet von {}", peer);
            Some(Packet::Error {
                code: 9999,
                message: "Unknown packet type".into(),
            })
        }
    }
}

pub async fn run_proxy(state: AppState) -> anyhow::Result<()> {
    let addr = &state.config.proxy.listen_addr;
    let listener = TcpListener::bind(addr).await?;
    info!("Proxy gestartet auf {}", addr);

    loop {
        let (client_stream, peer) = listener.accept().await?;
        let target = state.proxy_target.read().await.clone();
        let connect_timeout = state.config.proxy.connect_timeout_secs;

        tokio::spawn(async move {
            let server_stream = match timeout(
                Duration::from_secs(connect_timeout),
                TcpStream::connect(&target),
            )
            .await
            {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => {
                    error!("Proxy: Konnte nicht zu {} verbinden: {}", target, e);
                    return;
                }
                Err(_) => {
                    error!("Proxy: Timeout beim Verbinden zu {}", target);
                    return;
                }
            };

            let (mut cr, mut cw) = tokio::io::split(client_stream);
            let (mut sr, mut sw) = tokio::io::split(server_stream);

            let c_to_s = tokio::spawn(async move {
                tokio::io::copy(&mut cr, &mut sw).await.ok();
            });
            let s_to_c = tokio::spawn(async move {
                tokio::io::copy(&mut sr, &mut cw).await.ok();
            });

            tokio::select! {
                _ = c_to_s => {},
                _ = s_to_c => {},
            }

            info!("Proxy-Verbindung von {} beendet", peer);
        });
    }
}

pub async fn run_health_monitor(state: AppState) -> anyhow::Result<()> {
    let interval = Duration::from_secs(state.config.servers.health_check_interval_secs);
    info!("Health-Monitor gestartet (Intervall: {}s)", interval.as_secs());

    loop {
        tokio::time::sleep(interval).await;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut servers = state.servers.write().await;
        let timeout = state.config.servers.health_check_interval_secs * 3;

        servers.retain(|s| {
            let alive = now - s.last_health < timeout;
            if !alive {
                warn!("Server {} nicht erreichbar (last health: {}s)", s.id, now - s.last_health);
            }
            alive
        });
    }
}

pub async fn run_api_server(state: AppState) -> anyhow::Result<()> {
    let addr = &state.config.api.listen_addr;
    let listener = TcpListener::bind(addr).await?;
    info!("API-Server gestartet auf {}", addr);

    loop {
        let (mut stream, peer) = listener.accept().await?;
        let state = state.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match stream.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let msg = String::from_utf8_lossy(&buf[..n]);
                        let response = handle_api_command(&state, &msg).await;
                        if let Err(e) = stream.write_all(response.as_bytes()).await {
                            error!("API Schreib Fehler an {}: {}", peer, e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("API Lese Fehler von {}: {}", peer, e);
                        break;
                    }
                }
            }
        });
    }
}

async fn handle_api_command(state: &AppState, cmd: &str) -> String {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
    if parts.is_empty() {
        return "ERROR: Empty command\n".into();
    }

    match parts[0] {
        "status" => {
            let servers = state.servers.read().await;
            let mut out = String::new();
            for s in servers.iter() {
                out.push_str(&format!(
                    "{} | Typ: {:?} | Active: {} | Players: {} | LastHealth: {}\n",
                    s.id, s.server_type, s.active, s.players_online, s.last_health
                ));
            }
            if out.is_empty() {
                out = "No servers connected\n".into();
            }
            out
        }
        "reboot" => {
            let reason = parts.get(1).copied().unwrap_or("manual");
            info!("API: Reboot angefordert: {}", reason);
            let cmd = crate::sync::SyncCommand {
                action: crate::protocol::SyncAction::PrepareReboot {
                    target_server: "server_a".into(),
                    reason: reason.to_string(),
                },
                source: "server_a".into(),
            };
            if let Err(e) = state.sync_tx.send(cmd).await {
                format!("ERROR: {}\n", e)
            } else {
                "OK: Reboot initiated\n".into()
            }
        }
        "switch" => {
            info!("API: Switch angefordert");
            let cmd = crate::sync::SyncCommand {
                action: crate::protocol::SyncAction::ActivateServer {
                    server_id: parts.get(1).copied().unwrap_or("server_b").to_string(),
                },
                source: "api".into(),
            };
            if let Err(e) = state.sync_tx.send(cmd).await {
                format!("ERROR: {}\n", e)
            } else {
                "OK: Switch initiated\n".into()
            }
        }
        "recover" => {
            info!("API: Recovery angefordert");
            let cmd = crate::sync::SyncCommand {
                action: crate::protocol::SyncAction::RecoverMain {
                    server_id: "server_a".into(),
                },
                source: "api".into(),
            };
            if let Err(e) = state.sync_tx.send(cmd).await {
                format!("ERROR: {}\n", e)
            } else {
                "OK: Recovery initiated\n".into()
            }
        }
        "help" => {
            "Commands:\n  status         - Server-Status anzeigen\n  reboot [grund] - Reboot von Server A einleiten\n  recover        - Server A wiederherstellen (nach Reboot)\n  switch [id]    - Aktiviert Server (a/b)\n".into()
        }
        _ => format!("ERROR: Unknown command: {}\n", parts[0]),
    }
}

fn bincode_serialize(packet: &Packet) -> Vec<u8> {
    let json = serde_json::to_vec(packet).unwrap_or_default();
    json
}

fn bincode_deserialize(data: &[u8]) -> Result<Packet, serde_json::Error> {
    serde_json::from_slice(data)
}
