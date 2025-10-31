use crate::backend::types::Config;
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
                config
                    .validate()
                    .with_context(|| format!("Config validation failed for {}", path.display()))?;
                Ok(config)
            }
            Err(parse_error) => {
                tracing::error!(
                    "Corrupted YAML config at {}: {}",
                    path.display(),
                    parse_error
                );

                let backup_path = path.with_extension("yaml.bak");
                if let Err(e) = fs::copy(path, &backup_path).await {
                    tracing::warn!("Failed to create backup of corrupted config: {}", e);
                } else {
                    tracing::info!(
                        "Created backup of corrupted config at {}",
                        backup_path.display()
                    );
                }

                let default_config = Config::default();
                save_config(path, &default_config).await.with_context(|| {
                    format!(
                        "Failed to create new config after corruption at {}",
                        path.display()
                    )
                })?;

                Err(anyhow::anyhow!(
                    "Config file was corrupted and has been replaced with defaults. Backup saved to {}. Error: {}",
                    backup_path.display(),
                    parse_error
                ))
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let default_config = Config::default();
            save_config(path, &default_config).await.with_context(|| {
                format!("Failed to create default config at {}", path.display())
            })?;
            Ok(default_config)
        }
        Err(e) => Err(e).with_context(|| format!("Failed to read config from {}", path.display())),
    }
}

// Atomic write with temp file
pub async fn save_config(path: &Path, config: &Config) -> anyhow::Result<()> {
    let yaml_content =
        serde_yaml::to_string(config).context("Failed to serialize config to YAML")?;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .await
        .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;

    let tmp_path = path.with_extension("tmp");

    fs::write(&tmp_path, yaml_content.as_bytes())
        .await
        .with_context(|| format!("Failed to write temporary config to {}", tmp_path.display()))
        .map_err(|e| {
            if e.to_string().contains("No space left on device")
                || e.to_string().contains("disk full")
            {
                anyhow::anyhow!(
                    "Disk space exhausted. Cannot save configuration. Free up disk space and try again."
                )
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
            .context("Failed to open temp file for fsync")?;
        file.sync_all().await.context("Failed to fsync temp file")?;
    }

    fs::rename(&tmp_path, path).await.with_context(|| {
        format!(
            "Failed to rename {} to {}",
            tmp_path.display(),
            path.display()
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
    .context("Failed to create file watcher")?;

    watcher
        .watch(&config_path, RecursiveMode::NonRecursive)
        .with_context(|| format!("Failed to watch config file: {}", config_path.display()))?;

    std::mem::forget(watcher);

    Ok(rx)
}

#[allow(dead_code)]
pub async fn cleanup_old_logs(log_directory: &Path, retention_days: u32) -> anyhow::Result<()> {
    let cutoff_time = std::time::SystemTime::now()
        - std::time::Duration::from_secs(retention_days as u64 * 24 * 60 * 60);

    let mut read_dir = fs::read_dir(log_directory)
        .await
        .with_context(|| format!("Failed to read log directory: {}", log_directory.display()))?;

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

    if deleted_count > 0 {
        tracing::info!("Cleaned up {} old log files", deleted_count);
    }

    Ok(())
}
