use crate::backend::Backend;
use crate::backend::process::ProcessInstance;
use crate::backend::types::{Config, ProcessId, TunnelEntry, TunnelId, TunnelRuntimeState};
use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct BackendState {
    config: Arc<ArcSwap<Config>>,
    processes: HashMap<TunnelId, ProcessInstance>,
    last_known_log_paths: HashMap<TunnelId, PathBuf>,
    config_path: PathBuf,
    wstunnel_binary_path: PathBuf,
    cancellation_token: CancellationToken,
    runtime_handle: tokio::runtime::Handle,
}

impl BackendState {
    pub fn new(
        runtime_handle: tokio::runtime::Handle,
        config_path: PathBuf,
        wstunnel_binary_path: PathBuf,
    ) -> Self {
        let config = runtime_handle
            .block_on(async { crate::backend::config::load_config(&config_path).await })
            .unwrap_or_else(|e| {
                tracing::error!("Failed to load config: {}, using defaults", e);
                Config::default()
            });

        Self {
            config: Arc::new(ArcSwap::from_pointee(config)),
            processes: HashMap::new(),
            last_known_log_paths: HashMap::new(),
            config_path,
            wstunnel_binary_path,
            cancellation_token: CancellationToken::new(),
            runtime_handle,
        }
    }

    fn cleanup_dead_processes(&mut self) {
        let dead_tunnel_ids: Vec<TunnelId> = self
            .processes
            .iter_mut()
            .filter_map(|(tunnel_id, process_instance)| {
                if let Some(ref mut child) = process_instance.child_handle {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            let exit_code = status.code();
                            tracing::info!(
                                "Process for tunnel {:?} exited with status: {} (code: {:?})",
                                tunnel_id,
                                status,
                                exit_code
                            );
                            Some(*tunnel_id)
                        }
                        Ok(None) => None,
                        Err(e) => {
                            tracing::error!(
                                "Error checking process status for tunnel {:?}: {}",
                                tunnel_id,
                                e
                            );
                            Some(*tunnel_id)
                        }
                    }
                } else {
                    Some(*tunnel_id)
                }
            })
            .collect();

        for tunnel_id in dead_tunnel_ids {
            if let Some(mut process) = self.processes.remove(&tunnel_id) {
                self.last_known_log_paths
                    .insert(tunnel_id, process.log_path.clone());
                process.cancellation_token.cancel();
                if let Some(monitor_task) = process.monitor_task.take() {
                    monitor_task.abort();
                }
                tracing::info!("Cleaned up dead process for tunnel {:?}", tunnel_id);
            }
        }
    }
}

impl Backend for BackendState {
    fn load_config(&mut self, _path: &Path) -> Result<Arc<Config>> {
        unimplemented!("load_config - to be implemented in Phase 3")
    }

    fn save_config(&self, _config: &Config, _path: &Path) -> Result<()> {
        unimplemented!("save_config - to be implemented in Phase 3")
    }

    fn get_config(&self) -> Arc<Config> {
        self.config.load_full()
    }

    fn validate_tunnel_entry(&self, entry: &TunnelEntry) -> Result<()> {
        entry.validate()
    }

    fn add_tunnel(&mut self, mut entry: TunnelEntry) -> Result<TunnelId> {
        self.validate_tunnel_entry(&entry)
            .context("Failed to validate tunnel entry")?;

        if entry.id == TunnelId::default() {
            entry.id = TunnelId::new();
        }

        let mut new_config = (*self.config.load_full()).clone();
        new_config.tunnels.push(entry.clone());
        new_config
            .validate()
            .context("Configuration validation failed after adding tunnel")?;

        let config_path = self.config_path.clone();
        self.runtime_handle
            .block_on(async {
                crate::backend::config::save_config(&config_path, &new_config).await
            })
            .context("Failed to save configuration to disk")?;

        self.config.store(Arc::new(new_config));
        tracing::info!("Added tunnel: {}", entry.tag);
        Ok(entry.id)
    }

    fn edit_tunnel(&mut self, id: TunnelId, entry: TunnelEntry) -> Result<()> {
        self.validate_tunnel_entry(&entry)
            .context("Failed to validate tunnel entry")?;

        anyhow::ensure!(
            !self.is_tunnel_running(id),
            "Cannot edit tunnel while it is running. Stop the tunnel first."
        );

        let mut new_config = (*self.config.load_full()).clone();
        let tunnel_index = new_config
            .tunnels
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!("Tunnel with ID {:?} not found in configuration", id))?;

        let old_tag = new_config.tunnels[tunnel_index].tag.clone();
        new_config.tunnels[tunnel_index] = entry.clone();
        new_config
            .validate()
            .context("Configuration validation failed after editing tunnel")?;

        let config_path = self.config_path.clone();
        self.runtime_handle
            .block_on(async {
                crate::backend::config::save_config(&config_path, &new_config).await
            })
            .context("Failed to save configuration to disk")?;

        self.config.store(Arc::new(new_config));
        tracing::info!("Edited tunnel: {} -> {}", old_tag, entry.tag);
        Ok(())
    }

    fn delete_tunnel(&mut self, id: TunnelId) -> Result<()> {
        if self.is_tunnel_running(id) {
            self.stop_tunnel(id)?;
        }

        let mut new_config = (*self.config.load_full()).clone();
        let tunnel_index = new_config
            .tunnels
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!("Tunnel with ID {:?} not found", id))?;

        let removed_tunnel = new_config.tunnels.remove(tunnel_index);

        let config_path = self.config_path.clone();
        self.runtime_handle.block_on(async {
            crate::backend::config::save_config(&config_path, &new_config).await
        })?;

        self.config.store(Arc::new(new_config));
        self.last_known_log_paths.remove(&id);

        tracing::info!("Deleted tunnel: {}", removed_tunnel.tag);

        Ok(())
    }

    fn list_tunnels(&mut self) -> Vec<TunnelEntry> {
        self.cleanup_dead_processes();
        let config = self.config.load();
        config
            .tunnels
            .iter()
            .map(|tunnel| {
                let mut entry = tunnel.clone();
                let status = self.get_tunnel_status(entry.id);
                entry.runtime_state = Some(status);
                entry
            })
            .collect()
    }

    fn get_tunnel(&mut self, id: TunnelId) -> Option<TunnelEntry> {
        self.cleanup_dead_processes();
        let config = self.config.load();
        config.tunnels.iter().find(|t| t.id == id).map(|tunnel| {
            let mut entry = tunnel.clone();
            let status = self.get_tunnel_status(entry.id);
            entry.runtime_state = Some(status);
            entry
        })
    }

    fn start_tunnel(&mut self, id: TunnelId) -> Result<ProcessId> {
        let config = self.config.load();

        let tunnel =
            config.tunnels.iter().find(|t| t.id == id).ok_or_else(|| {
                anyhow::anyhow!("Tunnel with ID {:?} not found in configuration", id)
            })?;

        if let Some(process) = self.processes.get(&id) {
            if process.pid().is_some() {
                anyhow::bail!(
                    "Tunnel '{}' is already running. Stop it before starting again.",
                    tunnel.tag
                );
            } else {
                anyhow::bail!(
                    "Tunnel '{}' is currently starting or stopping. Please wait.",
                    tunnel.tag
                );
            }
        }

        let binary_path = config
            .global
            .wstunnel_binary_path
            .clone()
            .unwrap_or_else(|| self.wstunnel_binary_path.clone());

        anyhow::ensure!(
            binary_path.exists(),
            "wstunnel binary not found at path: {}. Please check the binary path configuration or use --wstunnel-path flag.",
            binary_path.display()
        );

        let cli_args = tunnel.cli_args.clone();
        let log_directory = config.global.log_directory.clone();
        let tunnel_id = tunnel.id;
        let tunnel_tag = tunnel.tag.clone();

        let child_token = self.cancellation_token.child_token();

        let process_instance = self
            .runtime_handle
            .block_on(async {
                let child =
                    crate::backend::process::spawn_tunnel_process(&binary_path, &cli_args).await?;
                crate::backend::process::create_process_instance(
                    tunnel_id,
                    tunnel_tag.clone(),
                    child,
                    &log_directory,
                    child_token,
                )
                .await
            })
            .with_context(|| format!("Failed to start tunnel '{}'", tunnel_tag))?;

        let pid = process_instance
            .pid()
            .context("Failed to get process ID after spawning tunnel")?;

        tracing::info!("Started tunnel '{}' with PID {}", tunnel_tag, pid);

        self.last_known_log_paths
            .insert(id, process_instance.log_path.clone());
        self.processes.insert(id, process_instance);

        Ok(pid)
    }

    fn stop_tunnel(&mut self, id: TunnelId) -> Result<()> {
        let process_instance = self
            .processes
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("Tunnel is not running"))?;

        if process_instance.pid().is_none() {
            anyhow::bail!("Tunnel is already stopping or has stopped");
        }

        let mut process_instance = self.processes.remove(&id).unwrap();
        self.last_known_log_paths
            .insert(id, process_instance.log_path.clone());

        process_instance.cancellation_token.cancel();

        let exit_code = self.runtime_handle.block_on(async {
            let mut exit_code = None;
            if let Some(mut child) = process_instance.child_handle.take() {
                let pid = child.id();

                match child.start_kill() {
                    Ok(_) => {
                        tracing::info!("Sent kill signal to process {:?}", pid);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to send kill signal to process {:?}: {}", pid, e);
                    }
                }

                match tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await {
                    Ok(Ok(status)) => {
                        exit_code = status.code();
                        tracing::info!(
                            "Process {:?} exited with status: {} (code: {:?})",
                            pid,
                            status,
                            exit_code
                        );
                    }
                    Ok(Err(e)) => {
                        tracing::error!("Error waiting for process {:?}: {}", pid, e);
                    }
                    Err(_) => {
                        tracing::warn!(
                            "Process {:?} did not exit within timeout, forcing kill",
                            pid
                        );
                    }
                }
            }

            if let Some(monitor_task) = process_instance.monitor_task.take() {
                monitor_task.abort();
                let _ = monitor_task.await;
            }

            exit_code
        });

        if let Some(code) = exit_code
            && code != 0
        {
            tracing::warn!("Tunnel {:?} stopped with non-zero exit code: {}", id, code);
        }

        tracing::info!("Stopped tunnel {:?}", id);

        Ok(())
    }

    fn start_autostart_tunnels(&mut self) -> Result<Vec<(TunnelId, Result<ProcessId>)>> {
        let config = self.config.load();
        let autostart_tunnels: Vec<TunnelId> = config
            .tunnels
            .iter()
            .filter(|t| t.autostart)
            .map(|t| t.id)
            .collect();

        let mut results = Vec::new();
        let mut started_count = 0;
        let mut failed_count = 0;

        for tunnel_id in autostart_tunnels {
            let result = self.start_tunnel(tunnel_id);
            match &result {
                Ok(pid) => {
                    tracing::info!("Autostart: Started tunnel {:?} with PID {}", tunnel_id, pid);
                    started_count += 1;
                }
                Err(e) => {
                    tracing::error!("Autostart: Failed to start tunnel {:?}: {}", tunnel_id, e);
                    failed_count += 1;
                }
            }
            results.push((tunnel_id, result));
        }

        tracing::info!(
            "Autostart complete: {} started, {} failed",
            started_count,
            failed_count
        );

        Ok(results)
    }

    fn get_tunnel_status(&self, id: TunnelId) -> TunnelRuntimeState {
        match self.processes.get(&id) {
            Some(process_instance) => {
                if let Some(pid) = process_instance.pid() {
                    TunnelRuntimeState::Running {
                        pid,
                        started_at: process_instance.started_at,
                        log_path: process_instance.log_path.clone(),
                    }
                } else {
                    TunnelRuntimeState::Stopped
                }
            }
            None => TunnelRuntimeState::Stopped,
        }
    }

    fn get_all_statuses(&self) -> Vec<(TunnelId, TunnelRuntimeState)> {
        let config = self.config.load();
        config
            .tunnels
            .iter()
            .map(|tunnel| (tunnel.id, self.get_tunnel_status(tunnel.id)))
            .collect()
    }

    fn is_tunnel_running(&self, id: TunnelId) -> bool {
        self.processes.get(&id).and_then(|p| p.pid()).is_some()
    }

    fn get_log_path(&self, id: TunnelId) -> Option<PathBuf> {
        self.processes
            .get(&id)
            .map(|p| p.log_path.clone())
            .or_else(|| self.last_known_log_paths.get(&id).cloned())
    }

    fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down backend, stopping all tunnels");

        self.cancellation_token.cancel();

        let tunnel_ids: Vec<TunnelId> = self.processes.keys().copied().collect();

        for tunnel_id in tunnel_ids {
            if let Err(e) = self.stop_tunnel(tunnel_id) {
                tracing::error!(
                    "Error stopping tunnel {:?} during shutdown: {}",
                    tunnel_id,
                    e
                );
            }
        }

        tracing::info!("Backend shutdown complete");

        Ok(())
    }
}
