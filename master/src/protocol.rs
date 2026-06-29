use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Packet {
    Handshake {
        server_id: String,
        server_type: ServerType,
        version: String,
    },
    HandshakeAck {
        session_id: String,
        backup_interval_secs: u64,
    },
    BackupStart {
        session_id: String,
        backup_id: String,
        total_size: u64,
        chunk_count: u64,
    },
    BackupChunk {
        session_id: String,
        backup_id: String,
        chunk_index: u64,
        data: Vec<u8>,
        checksum: String,
    },
    BackupComplete {
        session_id: String,
        backup_id: String,
        checksum: String,
    },
    BackupAck {
        backup_id: String,
        status: BackupStatus,
    },
    SyncMessage {
        command: SyncCommand,
    },
    SyncData {
        session_id: String,
        data_type: SyncDataType,
        data: Vec<u8>,
    },
    SyncComplete {
        session_id: String,
        status: SyncStatus,
    },
    HealthCheck,
    HealthCheckOk {
        server_id: String,
        players_online: u32,
        uptime_secs: u64,
    },
    ShutdownNotice {
        reason: String,
        grace_period_secs: u64,
    },
    Error {
        code: u32,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerType {
    ServerA,
    ServerB,
    Standalone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupStatus {
    Accepted,
    InvalidChecksum,
    StorageError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCommand {
    pub action: SyncAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncAction {
    PrepareReboot {
        target_server: String,
        reason: String,
    },
    StartSync {
        source: String,
        target: String,
    },
    ApplyFullBackup {
        backup_id: String,
    },
    IncrementalSync {
        changes: Vec<FileChange>,
    },
    ActivateServer {
        server_id: String,
    },
    DeactivateServer {
        server_id: String,
        graceful: bool,
    },
    Abort,
    RecoverMain {
        server_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncDataType {
    WorldData,
    PlayerData,
    ConfigData,
    PluginData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncStatus {
    Complete,
    Failed(String),
    Partial(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change_type: ChangeType,
    pub checksum: Option<String>,
    pub data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Modified,
    Added,
    Deleted,
}
