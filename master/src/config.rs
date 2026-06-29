use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub backup: BackupConfig,
    pub pterodactyl: PterodactylConfig,
    pub proxy: ProxyConfig,
    pub api: ApiConfig,
    pub servers: ServersConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub listen_addr: String,
    pub data_dir: PathBuf,
    pub compression_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PterodactylConfig {
    pub base_url: String,
    pub api_key: String,
    pub server_a_id: String,
    pub server_b_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub listen_addr: String,
    pub target_a: String,
    pub target_b: String,
    pub connect_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub listen_addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServersConfig {
    pub backup_interval_secs: u64,
    pub sync_timeout_secs: u64,
    pub health_check_interval_secs: u64,
    pub health_check_max_retries: u32,
    pub graceful_shutdown_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backup: BackupConfig {
                listen_addr: "0.0.0.0:9100".into(),
                data_dir: PathBuf::from("data/backups"),
                compression_level: 6,
            },
            pterodactyl: PterodactylConfig {
                base_url: "https://panel.example.com".into(),
                api_key: "dein-api-key".into(),
                server_a_id: "server-a-id".into(),
                server_b_id: "server-b-id".into(),
            },
            proxy: ProxyConfig {
                listen_addr: "0.0.0.0:25565".into(),
                target_a: "127.0.0.1:25566".into(),
                target_b: "127.0.0.1:25567".into(),
                connect_timeout_secs: 10,
            },
            api: ApiConfig {
                listen_addr: "0.0.0.0:9200".into(),
            },
            servers: ServersConfig {
                backup_interval_secs: 300,
                sync_timeout_secs: 120,
                health_check_interval_secs: 10,
                health_check_max_retries: 3,
                graceful_shutdown_secs: 30,
            },
        }
    }
}

pub fn load() -> anyhow::Result<Config> {
    let config_path = PathBuf::from("ignite-config.toml");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        Ok(toml::from_str(&content)?)
    } else {
        let cfg = Config::default();
        let content = toml::to_string_pretty(&cfg)?;
        std::fs::write(&config_path, &content)?;
        println!("Konfiguration erstellt: {:?}. Bitte anpassen und neu starten.", config_path);
        std::process::exit(0);
    }
}
