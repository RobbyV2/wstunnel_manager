pub mod backend_impl;
pub mod config;
pub mod mock_backend;
pub mod process;
pub mod types;

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use types::{Config, ProcessId, TunnelEntry, TunnelId, TunnelRuntimeState};

pub trait Backend: Send + Sync {
    // Configuration Management
    #[allow(dead_code)]
    fn load_config(&mut self, path: &Path) -> Result<Arc<Config>>;
    #[allow(dead_code)]
    fn save_config(&self, config: &Config, path: &Path) -> Result<()>;
    #[allow(dead_code)]
    fn get_config(&self) -> Arc<Config>;
    fn validate_tunnel_entry(&self, entry: &TunnelEntry) -> Result<()>;

    // Tunnel CRUD Operations
    fn add_tunnel(&mut self, entry: TunnelEntry) -> Result<TunnelId>;
    fn edit_tunnel(&mut self, id: TunnelId, entry: TunnelEntry) -> Result<()>;
    fn delete_tunnel(&mut self, id: TunnelId) -> Result<()>;
    fn list_tunnels(&mut self) -> Vec<TunnelEntry>;
    fn get_tunnel(&mut self, id: TunnelId) -> Option<TunnelEntry>;

    // Process Lifecycle Management
    fn start_tunnel(&mut self, id: TunnelId) -> Result<ProcessId>;
    fn stop_tunnel(&mut self, id: TunnelId) -> Result<()>;
    fn start_autostart_tunnels(&mut self) -> Result<Vec<(TunnelId, Result<ProcessId>)>>;

    // State Queries
    fn get_tunnel_status(&self, id: TunnelId) -> TunnelRuntimeState;
    #[allow(dead_code)]
    fn get_all_statuses(&self) -> Vec<(TunnelId, TunnelRuntimeState)>;
    fn is_tunnel_running(&self, id: TunnelId) -> bool;
    fn get_log_path(&self, id: TunnelId) -> Option<PathBuf>;

    // Lifecycle
    fn shutdown(&mut self) -> Result<()>;
}
