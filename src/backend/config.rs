use crate::backend::types::Config;
use crate::errors;
use anyhow::Context;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::mpsc;

#[allow(dead_code)]
pub async fn load_config(path: &Path) -> anyhow::Result<Config> {
    match fs::read_to_string(path).await {
        Ok(contents) => match serde_yaml::from_str::<Config>(&contents) {
            Ok(config) => {
                config.validate().with_context(|| {
                    errors::config::validation_failed(&path.display().to_string())
                })?;
                Ok(config)
            }
            Err(parse_error) => {
                tracing::error!(
                    "{}",
                    errors::config::corrupted_yaml(
                        &path.display().to_string(),
                        &parse_error.to_string()
                    )
                );

                let backup_path = path.with_extension("yaml.bak");
                if let Err(e) = fs::copy(path, &backup_path).await {
                    tracing::warn!("Failed to create backup of corrupted config: {}", e);
                } else {
                    tracing::info!(
                        "{}",
                        errors::config::backup_created(&backup_path.display().to_string())
                    );
                }

                let default_config = Config::default();
                save_config(path, &default_config).await.with_context(|| {
                    errors::config::failed_to_create_default(&path.display().to_string())
                })?;

                Err(anyhow::anyhow!(errors::config::corrupted(
                    &path.display().to_string(),
                    &backup_path.display().to_string(),
                    &parse_error.to_string()
                )))
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let default_config = Config::default();
            save_config(path, &default_config).await.with_context(|| {
                errors::config::failed_to_create_default(&path.display().to_string())
            })?;
            Ok(default_config)
        }
        Err(e) => {
            Err(e).with_context(|| errors::config::failed_to_read(&path.display().to_string()))
        }
    }
}

// Atomic write with temp file
pub async fn save_config(path: &Path, config: &Config) -> anyhow::Result<()> {
    let yaml_content =
        serde_yaml::to_string(config).context(errors::config::failed_to_serialize())?;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .await
        .with_context(|| errors::config::failed_to_create_dir(&parent.display().to_string()))?;

    let tmp_path = path.with_extension("tmp");

    fs::write(&tmp_path, yaml_content.as_bytes())
        .await
        .with_context(|| errors::config::failed_to_write_temp(&tmp_path.display().to_string()))
        .map_err(|e| {
            if e.to_string().contains("No space left on device")
                || e.to_string().contains("disk full")
            {
                anyhow::anyhow!(errors::disk::FULL)
            } else {
                e
            }
        })?;

    #[cfg(unix)]
    #[allow(unused_imports)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&tmp_path)
            .await
            .context(errors::config::FAILED_TO_OPEN_TEMP)?;
        file.sync_all()
            .await
            .context(errors::config::FAILED_TO_FSYNC)?;
    }

    fs::rename(&tmp_path, path).await.with_context(|| {
        errors::config::failed_to_rename(
            &tmp_path.display().to_string(),
            &path.display().to_string(),
        )
    })?;

    Ok(())
}

#[allow(dead_code)]
pub fn watch_config_file(
    config_path: PathBuf,
) -> anyhow::Result<mpsc::Receiver<notify::Result<Event>>> {
    let (tx, rx) = mpsc::channel(10);

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            let _ = tx.blocking_send(res);
        },
        notify::Config::default(),
    )
    .context(errors::config::FAILED_TO_CREATE_WATCHER)?;

    watcher
        .watch(&config_path, RecursiveMode::NonRecursive)
        .with_context(|| errors::config::failed_to_watch(&config_path.display().to_string()))?;

    std::mem::forget(watcher);

    Ok(rx)
}

pub async fn cleanup_old_logs(log_directory: &Path, retention_days: u32) -> anyhow::Result<()> {
    if !log_directory.exists() {
        tracing::info!(
            "Log directory does not exist, creating: {}",
            log_directory.display()
        );
        fs::create_dir_all(log_directory).await.with_context(|| {
            errors::config::failed_to_create_dir(&log_directory.display().to_string())
        })?;
        return Ok(());
    }

    let cutoff_time = std::time::SystemTime::now()
        - std::time::Duration::from_secs(retention_days as u64 * 24 * 60 * 60);

    let mut read_dir = match fs::read_dir(log_directory).await {
        Ok(dir) => dir,
        Err(e) => {
            tracing::warn!(
                "Failed to read log directory {}: {}, skipping cleanup",
                log_directory.display(),
                e
            );
            return Ok(());
        }
    };

    let mut deleted_count = 0;
    while let Some(entry) = read_dir.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("log")
            && let Ok(metadata) = entry.metadata().await
            && let Ok(modified) = metadata.modified()
            && modified < cutoff_time
        {
            match fs::remove_file(&path).await {
                Ok(_) => {
                    tracing::info!("Deleted old log file: {}", path.display());
                    deleted_count += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to delete old log file {}: {}", path.display(), e);
                }
            }
        }
    }

    match deleted_count {
        0 => tracing::debug!("No old log files to clean up"),
        n => tracing::info!("Cleaned up {} old log files", n),
    }

    Ok(())
}

pub fn cleanup_old_logs_sync(
    runtime_handle: &tokio::runtime::Handle,
    log_directory: &Path,
    retention_days: u32,
) -> anyhow::Result<()> {
    tracing::info!(
        "Log retention enabled: cleaning up logs older than {} days in {}",
        retention_days,
        log_directory.display()
    );

    runtime_handle.block_on(async { cleanup_old_logs(log_directory, retention_days).await })
}
