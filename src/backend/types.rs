use crate::errors;
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
            !self.tag.trim().is_empty(),
            errors::tunnel::validation::TAG_EMPTY
        );
        ensure!(
            self.tag.len() <= 100,
            errors::tunnel::validation::tag_too_long(&self.tag)
        );
        ensure!(
            !self.cli_args.trim().is_empty(),
            errors::tunnel::validation::CLI_ARGS_EMPTY
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
    crate::constants::default_log_directory()
}

impl GlobalSettings {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(ref path) = self.wstunnel_binary_path {
            ensure!(
                path.exists(),
                errors::binary::not_found(&path.display().to_string())
            );
        }

        if let Some(days) = self.log_retention_days {
            ensure!(
                (1..=3650).contains(&days),
                errors::logs::retention_invalid(days)
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
            errors::config::unsupported_version(self.version)
        );

        let mut seen_ids = HashSet::new();
        for tunnel in &self.tunnels {
            ensure!(
                seen_ids.insert(tunnel.id),
                errors::tunnel::validation::duplicate_id(&format!("{:?}", tunnel.id))
            );
            tunnel
                .validate()
                .with_context(|| errors::tunnel::validation::failed(&tunnel.tag))?;
        }

        self.global
            .validate()
            .context(errors::config::GLOBAL_VALIDATION_FAILED)?;

        Ok(())
    }
}
