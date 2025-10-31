use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TunnelId(Uuid);

impl TunnelId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TunnelId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumIter)]
#[serde(rename_all = "lowercase")]
pub enum TunnelMode {
    Client,
    Server,
}

impl TunnelMode {
    #[allow(dead_code)]
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }
}

impl fmt::Display for TunnelMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TunnelMode::Client => write!(f, "Client"),
            TunnelMode::Server => write!(f, "Server"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessId(u32);

impl From<u32> for ProcessId {
    fn from(pid: u32) -> Self {
        Self(pid)
    }
}

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(SystemTime);

impl Timestamp {
    pub fn now() -> Self {
        Self(SystemTime::now())
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.0.elapsed().unwrap_or_default()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", humantime::format_rfc3339(self.0))
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TunnelRuntimeState {
    Stopped,
    Starting,
    Running {
        pid: ProcessId,
        started_at: Timestamp,
        log_path: PathBuf,
    },
    Failed {
        error: String,
        last_attempt: Timestamp,
        exit_code: Option<i32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelEntry {
    pub id: TunnelId,
    pub tag: String,
    pub mode: TunnelMode,
    pub cli_args: String,
    pub autostart: bool,

    #[serde(skip)]
    pub runtime_state: Option<TunnelRuntimeState>,
}

impl TunnelEntry {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.tag.len() <= 100,
            "Tunnel tag too long (max 100 characters): {}",
            self.tag.len()
        );
        ensure!(
            !self.cli_args.trim().is_empty(),
            "CLI arguments cannot be empty"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    #[serde(default)]
    pub wstunnel_binary_path: Option<PathBuf>,

    #[serde(default = "default_log_directory")]
    pub log_directory: PathBuf,

    #[serde(default)]
    pub log_retention_days: Option<u32>,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            wstunnel_binary_path: None,
            log_directory: default_log_directory(),
            log_retention_days: None,
        }
    }
}

fn default_log_directory() -> PathBuf {
    PathBuf::from(".").join("logs")
}

impl GlobalSettings {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(ref path) = self.wstunnel_binary_path {
            ensure!(
                path.exists(),
                "wstunnel binary not found at path: {}",
                path.display()
            );
        }

        if let Some(days) = self.log_retention_days {
            ensure!(
                days >= 1,
                "Log retention days must be at least 1, got: {}",
                days
            );
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default)]
    pub global: GlobalSettings,

    #[serde(default)]
    pub tunnels: Vec<TunnelEntry>,
}

fn default_version() -> u32 {
    1
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            global: GlobalSettings::default(),
            tunnels: Vec::new(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.version == 1,
            "Unsupported config version: {}. Expected version 1",
            self.version
        );

        let mut seen_ids = HashSet::new();
        for tunnel in &self.tunnels {
            ensure!(
                seen_ids.insert(tunnel.id),
                "Duplicate tunnel ID found: {:?}",
                tunnel.id
            );
            tunnel
                .validate()
                .with_context(|| format!("Validation failed for tunnel: {}", tunnel.tag))?;
        }

        self.global
            .validate()
            .context("Global settings validation failed")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validate_valid() {
        let config = Config {
            version: 1,
            global: GlobalSettings::default(),
            tunnels: vec![TunnelEntry {
                id: TunnelId::new(),
                tag: "test-tunnel".to_string(),
                mode: TunnelMode::Client,
                cli_args: "client ws://example.com".to_string(),
                autostart: false,
                runtime_state: None,
            }],
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_duplicate_ids() {
        let id = TunnelId::new();
        let config = Config {
            version: 1,
            global: GlobalSettings::default(),
            tunnels: vec![
                TunnelEntry {
                    id,
                    tag: "tunnel-1".to_string(),
                    mode: TunnelMode::Client,
                    cli_args: "client ws://example.com".to_string(),
                    autostart: false,
                    runtime_state: None,
                },
                TunnelEntry {
                    id,
                    tag: "tunnel-2".to_string(),
                    mode: TunnelMode::Server,
                    cli_args: "server ws://0.0.0.0:8080".to_string(),
                    autostart: false,
                    runtime_state: None,
                },
            ],
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Duplicate tunnel ID")
        );
    }

    #[test]
    fn test_config_validate_invalid_version() {
        let config = Config {
            version: 999,
            global: GlobalSettings::default(),
            tunnels: vec![],
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported config version")
        );
    }

    #[test]
    fn test_tunnel_entry_validate_valid() {
        let entry = TunnelEntry {
            id: TunnelId::new(),
            tag: "valid-tunnel".to_string(),
            mode: TunnelMode::Client,
            cli_args: "client ws://example.com".to_string(),
            autostart: true,
            runtime_state: None,
        };

        assert!(entry.validate().is_ok());
    }

    #[test]
    fn test_tunnel_entry_validate_empty_tag() {
        let entry = TunnelEntry {
            id: TunnelId::new(),
            tag: "   ".to_string(),
            mode: TunnelMode::Client,
            cli_args: "client ws://example.com".to_string(),
            autostart: false,
            runtime_state: None,
        };

        let result = entry.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("tag cannot be empty")
        );
    }

    #[test]
    fn test_tunnel_entry_validate_tag_too_long() {
        let entry = TunnelEntry {
            id: TunnelId::new(),
            tag: "a".repeat(101),
            mode: TunnelMode::Client,
            cli_args: "client ws://example.com".to_string(),
            autostart: false,
            runtime_state: None,
        };

        let result = entry.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("tag too long"));
    }

    #[test]
    fn test_tunnel_entry_validate_empty_cli_args() {
        let entry = TunnelEntry {
            id: TunnelId::new(),
            tag: "test-tunnel".to_string(),
            mode: TunnelMode::Client,
            cli_args: "   ".to_string(),
            autostart: false,
            runtime_state: None,
        };

        let result = entry.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("CLI arguments cannot be empty")
        );
    }

    #[test]
    fn test_tunnel_entry_autostart_flag() {
        let entry_with_autostart = TunnelEntry {
            id: TunnelId::new(),
            tag: "autostart-tunnel".to_string(),
            mode: TunnelMode::Server,
            cli_args: "server ws://0.0.0.0:8080".to_string(),
            autostart: true,
            runtime_state: None,
        };

        assert!(entry_with_autostart.validate().is_ok());
        assert!(entry_with_autostart.autostart);

        let entry_without_autostart = TunnelEntry {
            id: TunnelId::new(),
            tag: "manual-tunnel".to_string(),
            mode: TunnelMode::Client,
            cli_args: "client ws://example.com".to_string(),
            autostart: false,
            runtime_state: None,
        };

        assert!(entry_without_autostart.validate().is_ok());
        assert!(!entry_without_autostart.autostart);
    }
}
