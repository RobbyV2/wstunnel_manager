use crate::backend::types::{ProcessId, Timestamp, TunnelId};
use crate::errors;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub struct ProcessInstance {
    #[allow(dead_code)]
    pub tunnel_id: TunnelId,
    pub child_handle: Option<Child>,
    pub monitor_task: Option<JoinHandle<()>>,
    pub log_path: PathBuf,
    pub started_at: Timestamp,
    pub cancellation_token: CancellationToken,
    #[allow(dead_code)]
    pub exit_code: Option<i32>,
    pub stderr_buffer: Arc<tokio::sync::Mutex<String>>,
}

impl ProcessInstance {
    pub fn new(
        tunnel_id: TunnelId,
        child_handle: Child,
        monitor_task: JoinHandle<()>,
        log_path: PathBuf,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            tunnel_id,
            child_handle: Some(child_handle),
            monitor_task: Some(monitor_task),
            log_path,
            started_at: Timestamp::now(),
            cancellation_token,
            exit_code: None,
            stderr_buffer: Arc::new(tokio::sync::Mutex::new(String::new())),
        }
    }

    pub fn pid(&self) -> Option<ProcessId> {
        self.child_handle
            .as_ref()
            .and_then(|child| child.id().map(ProcessId::from))
    }

    #[allow(dead_code)]
    pub async fn get_stderr(&self) -> String {
        self.stderr_buffer.lock().await.clone()
    }
}

fn parse_cli_args(cli_args: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut in_quotes = false;
    let chars = cli_args.chars().peekable();

    for c in chars {
        match c {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' if !in_quotes => {
                if !current_arg.is_empty() {
                    args.push(current_arg.clone());
                    current_arg.clear();
                }
            }
            _ => {
                current_arg.push(c);
            }
        }
    }

    if !current_arg.is_empty() {
        args.push(current_arg);
    }

    args
}

pub async fn spawn_tunnel_process(binary_path: &PathBuf, cli_args: &str) -> Result<Child> {
    let args = parse_cli_args(cli_args);

    tracing::info!(
        "Spawning wstunnel process: {} {}",
        binary_path.display(),
        cli_args
    );

    let mut command = Command::new(binary_path);
    command
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let child = command.spawn().map_err(|e| {
        let error_msg = e.to_string();
        if error_msg.contains("No such file or directory")
            || error_msg.contains("cannot find the path")
        {
            anyhow::anyhow!(errors::binary::not_found_simple(
                &binary_path.display().to_string()
            ))
        } else if error_msg.contains("Permission denied") {
            anyhow::anyhow!(errors::binary::permission_denied(
                &binary_path.display().to_string()
            ))
        } else if error_msg.contains("Address already in use") {
            anyhow::anyhow!(errors::process::PORT_IN_USE)
        } else {
            anyhow::anyhow!(errors::process::spawn_failed(&error_msg))
        }
    })?;

    Ok(child)
}

pub async fn create_process_instance(
    tunnel_id: TunnelId,
    tunnel_name: String,
    mut child: Child,
    log_directory: &PathBuf,
    cancellation_token: CancellationToken,
) -> Result<ProcessInstance> {
    let pid = child.id().context(errors::process::FAILED_TO_GET_PID)?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");

    let sanitized_name = if tunnel_name.is_empty() {
        format!("{:?}", tunnel_id)
    } else {
        sanitize_filename(&tunnel_name)
    };

    let log_filename = format!("{}-{}-{}.log", sanitized_name, pid, timestamp);
    let log_path = log_directory.join(log_filename);

    tokio::fs::create_dir_all(log_directory)
        .await
        .context(errors::logs::FAILED_TO_CREATE_DIR)?;

    let log_file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .await
        .context(errors::logs::FAILED_TO_CREATE_FILE)?;

    let stdout = child
        .stdout
        .take()
        .context(errors::process::FAILED_TO_CAPTURE_STDOUT)?;
    let stderr = child
        .stderr
        .take()
        .context(errors::process::FAILED_TO_CAPTURE_STDERR)?;

    let log_path_clone = log_path.clone();
    let monitor_token = cancellation_token.clone();
    let stderr_buffer = Arc::new(tokio::sync::Mutex::new(String::new()));
    let stderr_buffer_clone = stderr_buffer.clone();

    let monitor_task = tokio::spawn(async move {
        let mut log_writer = tokio::io::BufWriter::new(log_file);
        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        let mut stdout_lines = stdout_reader.lines();
        let mut stderr_lines = stderr_reader.lines();

        loop {
            tokio::select! {
                _ = monitor_token.cancelled() => {
                    tracing::info!("Monitor task cancelled for log: {}", log_path_clone.display());
                    break;
                }
                result = stdout_lines.next_line() => {
                    match result {
                        Ok(Some(line)) => {
                            let timestamp = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                            let log_line = format!("[{}] [STDOUT] {}\n", timestamp, line);
                            if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut log_writer, log_line.as_bytes()).await {
                                if e.to_string().contains("No space left on device") || e.to_string().contains("disk full") {
                                    tracing::error!("{}", errors::disk::full_log_write(&log_path_clone.display().to_string()));
                                } else {
                                    tracing::error!("{}", errors::logs::failed_to_write_stdout(&e.to_string()));
                                }
                                break;
                            }
                        }
                        Ok(None) => {
                            tracing::info!("Stdout stream closed for log: {}", log_path_clone.display());
                            break;
                        }
                        Err(e) => {
                            tracing::error!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
                result = stderr_lines.next_line() => {
                    match result {
                        Ok(Some(line)) => {
                            let timestamp = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                            let log_line = format!("[{}] [STDERR] {}\n", timestamp, line);

                            let mut buffer = stderr_buffer_clone.lock().await;
                            buffer.push_str(&line);
                            buffer.push('\n');
                            if buffer.len() > 4096 {
                                *buffer = buffer.chars().rev().take(4096).collect::<String>().chars().rev().collect();
                            }
                            drop(buffer);

                            if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut log_writer, log_line.as_bytes()).await {
                                if e.to_string().contains("No space left on device") || e.to_string().contains("disk full") {
                                    tracing::error!("{}", errors::disk::full_log_write(&log_path_clone.display().to_string()));
                                } else {
                                    tracing::error!("{}", errors::logs::failed_to_write_stderr(&e.to_string()));
                                }
                                break;
                            }
                        }
                        Ok(None) => {
                            tracing::info!("Stderr stream closed for log: {}", log_path_clone.display());
                            break;
                        }
                        Err(e) => {
                            tracing::error!("Error reading stderr: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        if let Err(e) = tokio::io::AsyncWriteExt::flush(&mut log_writer).await {
            tracing::error!("{}", errors::logs::failed_to_flush(&e.to_string()));
        }
    });

    let mut instance =
        ProcessInstance::new(tunnel_id, child, monitor_task, log_path, cancellation_token);
    instance.stderr_buffer = stderr_buffer;

    Ok(instance)
}
