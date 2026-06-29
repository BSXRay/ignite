use crate::config::PterodactylConfig;
use log::{error, info, warn};
use reqwest::Client as HttpClient;
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Client {
    http: HttpClient,
    config: PterodactylConfig,
}

#[derive(Debug, Serialize)]
struct PowerAction {
    signal: String,
}

impl Client {
    pub fn new(config: &PterodactylConfig) -> Self {
        let http = HttpClient::builder()
            .timeout(Duration::from_secs(30))
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {}", config.api_key)
                        .parse()
                        .unwrap(),
                );
                headers.insert(
                    reqwest::header::CONTENT_TYPE,
                    "application/json".parse().unwrap(),
                );
                headers.insert(
                    reqwest::header::ACCEPT,
                    "application/json".parse().unwrap(),
                );
                headers
            })
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            config: config.clone(),
        }
    }

    pub async fn start_server(&self, server_id: &str) -> anyhow::Result<()> {
        info!("Pterodactyl: Starte Server {}", server_id);
        let url = format!(
            "{}/api/client/servers/{}/power",
            self.config.base_url, server_id
        );

        let action = PowerAction {
            signal: "start".into(),
        };

        let resp = self.http.post(&url).json(&action).send().await?;

        if resp.status().is_success() {
            info!("Pterodactyl: Server {} gestartet", server_id);
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            error!("Pterodactyl: Start fehlgeschlagen ({}): {}", status, body);
            Err(anyhow::anyhow!("Start fehlgeschlagen: {} {}", status, body))
        }
    }

    pub async fn stop_server(&self, server_id: &str) -> anyhow::Result<()> {
        info!("Pterodactyl: Stoppe Server {}", server_id);
        let url = format!(
            "{}/api/client/servers/{}/power",
            self.config.base_url, server_id
        );

        let action = PowerAction {
            signal: "stop".into(),
        };

        let resp = self.http.post(&url).json(&action).send().await?;

        if resp.status().is_success() {
            info!("Pterodactyl: Server {} gestoppt", server_id);
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            error!("Pterodactyl: Stop fehlgeschlagen ({}): {}", status, body);
            Err(anyhow::anyhow!("Stop fehlgeschlagen: {} {}", status, body))
        }
    }

    pub async fn kill_server(&self, server_id: &str) -> anyhow::Result<()> {
        info!("Pterodactyl: Kille Server {}", server_id);
        let url = format!(
            "{}/api/client/servers/{}/power",
            self.config.base_url, server_id
        );

        let action = PowerAction {
            signal: "kill".into(),
        };

        let resp = self.http.post(&url).json(&action).send().await?;

        if resp.status().is_success() {
            info!("Pterodactyl: Server {} gekillt", server_id);
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Kill fehlgeschlagen: {} {}", status, body))
        }
    }

    pub async fn get_server_status(&self, server_id: &str) -> anyhow::Result<String> {
        let url = format!(
            "{}/api/client/servers/{}/resources",
            self.config.base_url, server_id
        );

        let resp = self.http.get(&url).send().await?;

        if resp.status().is_success() {
            let data: serde_json::Value = resp.json().await?;
            let state = data["attributes"]["current_state"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            Ok(state)
        } else {
            Err(anyhow::anyhow!("Status-Abfrage fehlgeschlagen"))
        }
    }

    pub async fn send_command(&self, server_id: &str, command: &str) -> anyhow::Result<()> {
        let url = format!(
            "{}/api/client/servers/{}/command",
            self.config.base_url, server_id
        );

        #[derive(Serialize)]
        struct CommandBody {
            command: String,
        }

        let resp = self.http
            .post(&url)
            .json(&CommandBody {
                command: command.to_string(),
            })
            .send()
            .await?;

        if resp.status().is_success() {
            info!("Pterodactyl: Command an {} gesendet: {}", server_id, command);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Command fehlgeschlagen"))
        }
    }

    pub async fn wait_for_status(
        &self,
        server_id: &str,
        target_status: &[&str],
        max_retries: u32,
        interval_secs: u64,
    ) -> anyhow::Result<bool> {
        for i in 0..max_retries {
            match self.get_server_status(server_id).await {
                Ok(status) => {
                    info!("Server {} Status: {} (Versuch {}/{})", server_id, status, i + 1, max_retries);
                    if target_status.contains(&status.as_str()) {
                        return Ok(true);
                    }
                }
                Err(e) => {
                    warn!("Status-Fehler für {}: {}", server_id, e);
                }
            }
            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        }
        Ok(false)
    }

    pub fn server_a_id(&self) -> &str {
        &self.config.server_a_id
    }

    pub fn server_b_id(&self) -> &str {
        &self.config.server_b_id
    }
}
