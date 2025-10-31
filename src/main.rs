// wstunnel Manager
// Entry point for the application

mod backend;
mod ui;

use anyhow::{Context, Result};
use backend::Backend;
use backend::backend_impl::BackendState;
use clap::Parser;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "wstunnel_manager")]
#[command(about = "wstunnel Manager - GUI and headless mode for managing wstunnel instances")]
struct Args {
    #[arg(long, help = "Run in headless mode without GUI")]
    headless: bool,

    #[arg(long, help = "Path to configuration file")]
    config: Option<PathBuf>,

    #[arg(long, help = "Path to wstunnel binary")]
    wstunnel_path: Option<PathBuf>,
}

fn setup_tracing(headless: bool) -> Result<()> {
    let log_directory = PathBuf::from(".").join("logs");
    std::fs::create_dir_all(&log_directory).context("Failed to create log directory")?;

    let file_appender = tracing_appender::rolling::daily(&log_directory, "app.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    if headless {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().with_writer(non_blocking).json())
            .with(fmt::layer().json().with_writer(std::io::stdout))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().with_writer(non_blocking).json())
            .with(fmt::layer().pretty().with_writer(std::io::stdout))
            .init();
    }

    std::mem::forget(_guard);

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    setup_tracing(args.headless).context("Failed to initialize tracing")?;

    type BackendHandle = Arc<Mutex<Option<Arc<Mutex<dyn Backend>>>>>;
    let backend_for_panic: BackendHandle = Arc::new(Mutex::new(None));
    let backend_for_panic_clone = backend_for_panic.clone();

    std::panic::set_hook(Box::new(move |panic_info| {
        tracing::error!("Application panic: {:?}", panic_info);

        if let Ok(backend_guard) = backend_for_panic_clone.lock()
            && let Some(backend) = backend_guard.as_ref()
            && let Ok(mut backend_lock) = backend.lock()
        {
            tracing::info!("Shutting down tunnels due to panic");
            let _ = backend_lock.shutdown();
        }
    }));

    tracing::info!("wstunnel Manager starting - Phase 10 complete");

    // Create tokio runtime
    let runtime = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
    let runtime_handle = runtime.handle().clone();

    // Get executable directory for relative path resolution
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    // Resolve config and binary paths from CLI args or defaults
    let config_path = args.config.unwrap_or_else(|| match &exe_dir {
        Some(dir) => dir.join("wstunnel_config.yaml"),
        None => PathBuf::from("wstunnel_config.yaml"),
    });
    let wstunnel_binary_path = args.wstunnel_path.unwrap_or_else(|| {
        let binary_name = if cfg!(windows) {
            "wstunnel.exe"
        } else {
            "wstunnel"
        };
        match &exe_dir {
            Some(dir) => dir.join(binary_name),
            None => PathBuf::from(binary_name),
        }
    });

    tracing::info!("Config path: {}", config_path.display());
    tracing::info!("Binary path: {}", wstunnel_binary_path.display());

    let use_mock = std::env::var("WSTUNNEL_MANAGER_MOCK").is_ok();

    if !use_mock && !wstunnel_binary_path.exists() {
        let error_msg = format!(
            "wstunnel binary not found at {}. Please install wstunnel or use --wstunnel-path flag.",
            wstunnel_binary_path.display()
        );
        tracing::error!("{}", error_msg);
        return Err(anyhow::anyhow!(error_msg));
    }

    if use_mock {
        tracing::info!("Running in MOCK mode - no real processes will be spawned");
    }

    let backend: Arc<Mutex<dyn Backend>> = if use_mock {
        Arc::new(Mutex::new(backend::mock_backend::MockBackend::new(
            runtime_handle.clone(),
            config_path.clone(),
        )))
    } else {
        let backend_state =
            BackendState::new(runtime_handle.clone(), config_path, wstunnel_binary_path);
        Arc::new(Mutex::new(backend_state))
    };

    *backend_for_panic.lock().unwrap() = Some(backend.clone());

    tracing::info!("Backend initialized");

    if args.headless {
        tracing::info!("Running in headless mode");

        {
            let mut backend_lock = backend.lock().unwrap();
            match backend_lock.start_autostart_tunnels() {
                Ok(results) => {
                    for (tunnel_id, result) in results {
                        match result {
                            Ok(pid) => {
                                tracing::info!(
                                    "Headless: Started tunnel {:?} with PID {}",
                                    tunnel_id,
                                    pid
                                );
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Headless: Failed to start tunnel {:?}: {}",
                                    tunnel_id,
                                    e
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Headless: Failed to start autostart tunnels: {}", e);
                }
            }
        }

        tracing::info!("Headless mode running. Press Ctrl+C to exit.");

        runtime.block_on(async {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    tracing::info!("Ctrl+C received, shutting down");
                }
                Err(e) => {
                    tracing::error!("Error listening for Ctrl+C: {}", e);
                }
            }
        });

        tracing::info!("Shutting down backend");
        {
            let mut backend_lock = backend.lock().unwrap();
            if let Err(e) = backend_lock.shutdown() {
                tracing::error!("Error during shutdown: {}", e);
            }
        }

        return Ok(());
    }

    // Launch iced application (GUI mode)
    tracing::info!("Launching UI");

    let backend_clone = backend.clone();
    let result = iced::application(
        ui::WstunnelManagerApp::title,
        ui::WstunnelManagerApp::update,
        ui::WstunnelManagerApp::view,
    )
    .subscription(ui::WstunnelManagerApp::subscription)
    .theme(ui::WstunnelManagerApp::theme)
    .window_size((1200.0, 800.0))
    .run_with(move || {
        let app = ui::WstunnelManagerApp::new(backend_clone.clone());
        (app, iced::Task::none())
    })
    .map_err(|e| anyhow::anyhow!("UI error: {:?}", e));

    tracing::info!("UI closed, shutting down backend");
    {
        let mut backend_lock = backend.lock().unwrap();
        if let Err(e) = backend_lock.shutdown() {
            tracing::error!("Error during shutdown: {}", e);
        }
    }

    result?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_args_headless() {
        let args = Args::parse_from(["wstunnel_manager", "--headless"]);
        assert!(args.headless);
        assert!(args.config.is_none());
        assert!(args.wstunnel_path.is_none());
    }

    #[test]
    fn test_cli_args_config_path() {
        let args = Args::parse_from(["wstunnel_manager", "--config", "custom_config.yaml"]);
        assert!(!args.headless);
        assert_eq!(args.config.unwrap(), PathBuf::from("custom_config.yaml"));
    }

    #[test]
    fn test_cli_args_wstunnel_path() {
        let args = Args::parse_from(["wstunnel_manager", "--wstunnel-path", "/usr/bin/wstunnel"]);
        assert!(!args.headless);
        assert_eq!(
            args.wstunnel_path.unwrap(),
            PathBuf::from("/usr/bin/wstunnel")
        );
    }

    #[test]
    fn test_cli_args_all_flags() {
        let args = Args::parse_from([
            "wstunnel_ui",
            "--headless",
            "--config",
            "test.yaml",
            "--wstunnel-path",
            "./wstunnel",
        ]);
        assert!(args.headless);
        assert_eq!(args.config.unwrap(), PathBuf::from("test.yaml"));
        assert_eq!(args.wstunnel_path.unwrap(), PathBuf::from("./wstunnel"));
    }
}
