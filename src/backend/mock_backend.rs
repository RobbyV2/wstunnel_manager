use crate::backend::Backend;
use crate::backend::types::{
    Config, ProcessId, Timestamp, TunnelEntry, TunnelId, TunnelRuntimeState,
};
use crate::errors;
use anyhow::Result;
use arc_swap::ArcSwap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
struct MockProcess {
    pid: ProcessId,
    started_at: Timestamp,
}

pub struct MockBackend {
    config: Arc<ArcSwap<Config>>,
    mock_processes: HashMap<TunnelId, MockProcess>,
    config_path: PathBuf,
    runtime_handle: tokio::runtime::Handle,
}

impl MockBackend {
    pub fn new(runtime_handle: tokio::runtime::Handle, config_path: PathBuf) -> Self {
        let config = runtime_handle
            .block_on(async { crate::backend::config::load_config(&config_path).await })
            .unwrap_or_else(|e| {
                tracing::warn!("MOCK: Failed to load config: {}, using defaults", e);
                Config::default()
            });

        Self {
            config: Arc::new(ArcSwap::from_pointee(config)),
            mock_processes: HashMap::new(),
            config_path,
            runtime_handle,
        }
    }

    fn generate_fake_pid() -> ProcessId {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        ProcessId::from((timestamp % 100000) as u32 + 10000)
    }
}

impl Backend for MockBackend {
    fn load_config(&mut self, path: &Path) -> Result<Arc<Config>> {
        self.runtime_handle.block_on(async {
            match crate::backend::config::load_config(path).await {
                Ok(config) => {
                    self.config.store(Arc::new(config.clone()));
                    Ok(Arc::new(config))
                }
                Err(e) => {
                    tracing::warn!("Failed to load config, using default: {}", e);
                    let default_config = Config::default();
                    self.config.store(Arc::new(default_config.clone()));
                    Ok(Arc::new(default_config))
                }
            }
        })
    }

    fn save_config(&self, config: &Config, path: &Path) -> Result<()> {
        self.runtime_handle
            .block_on(async { crate::backend::config::save_config(path, config).await })
    }

    fn get_config(&self) -> Arc<Config> {
        self.config.load_full()
    }

    fn validate_tunnel_entry(&self, entry: &TunnelEntry) -> Result<()> {
        entry.validate()
    }

    fn add_tunnel(&mut self, mut entry: TunnelEntry) -> Result<TunnelId> {
        self.validate_tunnel_entry(&entry)?;

        if entry.id == TunnelId::default() {
            entry.id = TunnelId::new();
        }

        let mut new_config = (*self.config.load_full()).clone();
        new_config.tunnels.push(entry.clone());
        new_config.validate()?;

        let config_path = self.config_path.clone();
        self.runtime_handle.block_on(async {
            crate::backend::config::save_config(&config_path, &new_config).await
        })?;

        self.config.store(Arc::new(new_config));
        Ok(entry.id)
    }

    fn edit_tunnel(&mut self, id: TunnelId, entry: TunnelEntry) -> Result<()> {
        self.validate_tunnel_entry(&entry)?;

        anyhow::ensure!(
            !self.is_tunnel_running(id),
            errors::tunnel::CANNOT_EDIT_RUNNING
        );

        let mut new_config = (*self.config.load_full()).clone();
        let tunnel_index = new_config
            .tunnels
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!(errors::tunnel::not_found(&format!("{:?}", id))))?;

        new_config.tunnels[tunnel_index] = entry;
        new_config.validate()?;

        let config_path = self.config_path.clone();
        self.runtime_handle.block_on(async {
            crate::backend::config::save_config(&config_path, &new_config).await
        })?;

        self.config.store(Arc::new(new_config));
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
            .ok_or_else(|| anyhow::anyhow!(errors::tunnel::not_found(&format!("{:?}", id))))?;

        let removed_tunnel = new_config.tunnels.remove(tunnel_index);

        let config_path = self.config_path.clone();
        self.runtime_handle.block_on(async {
            crate::backend::config::save_config(&config_path, &new_config).await
        })?;

        self.config.store(Arc::new(new_config));

        tracing::info!("MOCK: Deleted tunnel: {}", removed_tunnel.tag);

        Ok(())
    }

    fn list_tunnels(&mut self) -> Vec<TunnelEntry> {
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

        let tunnel = config
            .tunnels
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!(errors::tunnel::not_found(&format!("{:?}", id))))?;

        anyhow::ensure!(
            !self.is_tunnel_running(id),
            errors::tunnel::already_running(&tunnel.tag)
        );

        let fake_pid = Self::generate_fake_pid();

        tracing::info!(
            "MOCK: Starting tunnel {} with fake PID {}",
            tunnel.tag,
            fake_pid
        );

        std::thread::sleep(std::time::Duration::from_millis(100));

        let mock_process = MockProcess {
            pid: fake_pid,
            started_at: Timestamp::now(),
        };

        self.mock_processes.insert(id, mock_process);

        tracing::info!(
            "MOCK: Started tunnel {} with fake PID {}",
            tunnel.tag,
            fake_pid
        );

        Ok(fake_pid)
    }

    fn stop_tunnel(&mut self, id: TunnelId) -> Result<()> {
        let _process = self
            .mock_processes
            .remove(&id)
            .ok_or_else(|| anyhow::anyhow!(errors::tunnel::NOT_RUNNING))?;

        tracing::info!("MOCK: Stopping tunnel {:?}", id);

        std::thread::sleep(std::time::Duration::from_millis(50));

        tracing::info!("MOCK: Stopped tunnel {:?}", id);

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
                    tracing::info!(
                        "MOCK: Autostart: Started tunnel {:?} with fake PID {}",
                        tunnel_id,
                        pid
                    );
                    started_count += 1;
                }
                Err(e) => {
                    tracing::error!(
                        "MOCK: Autostart: Failed to start tunnel {:?}: {}",
                        tunnel_id,
                        e
                    );
                    failed_count += 1;
                }
            }
            results.push((tunnel_id, result));
        }

        tracing::info!(
            "MOCK: Autostart complete: {} started, {} failed",
            started_count,
            failed_count
        );

        Ok(results)
    }

    fn get_tunnel_status(&self, id: TunnelId) -> TunnelRuntimeState {
        match self.mock_processes.get(&id) {
            Some(mock_process) => TunnelRuntimeState::Running {
                pid: mock_process.pid,
                started_at: mock_process.started_at,
                log_path: PathBuf::from(format!("logs/mock-{}.log", mock_process.pid)),
            },
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
        self.mock_processes.contains_key(&id)
    }

    fn get_log_path(&self, id: TunnelId) -> Option<PathBuf> {
        self.mock_processes
            .get(&id)
            .map(|p| PathBuf::from(format!("logs/mock-{}.log", p.pid)))
    }

    fn shutdown(&mut self) -> Result<()> {
        tracing::info!("MOCK: Shutting down backend, stopping all tunnels");

        let tunnel_ids: Vec<TunnelId> = self.mock_processes.keys().copied().collect();

        for tunnel_id in tunnel_ids {
            if let Err(e) = self.stop_tunnel(tunnel_id) {
                tracing::error!(
                    "MOCK: Error stopping tunnel {:?} during shutdown: {}",
                    tunnel_id,
                    e
                );
            }
        }

        tracing::info!("MOCK: Backend shutdown complete");

        Ok(())
    }

    fn cleanup_old_logs_if_configured(&self) -> Result<()> {
        let config = self.config.load();

        match config.global.log_retention_days {
            Some(days) => {
                tracing::info!(
                    "MOCK: Would clean up logs older than {} days in {}",
                    days,
                    config.global.log_directory.display()
                );
                Ok(())
            }
            None => {
                tracing::debug!("Log retention not configured, skipping log cleanup");
                Ok(())
            }
        }
    }
}
