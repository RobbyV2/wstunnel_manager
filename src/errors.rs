pub mod tunnel {
    pub fn not_found(id: &str) -> String {
        format!("Tunnel with ID {} not found", id)
    }

    pub fn already_running(tag: &str) -> String {
        format!(
            "Tunnel '{}' is already running. Stop it before starting again.",
            tag
        )
    }

    pub fn transitional_state(tag: &str) -> String {
        format!(
            "Tunnel '{}' is currently starting or stopping. Please wait.",
            tag
        )
    }

    pub const CANNOT_EDIT_RUNNING: &str =
        "Cannot edit tunnel while it is running. Stop the tunnel first.";
    pub const NOT_RUNNING: &str = "Tunnel is not running";
    pub const ALREADY_STOPPING: &str = "Tunnel is already stopping or has stopped";
    pub const NO_LOGS: &str = "Tunnel is not running or has no logs";

    pub fn failed_to_start(tag: &str) -> String {
        format!("Failed to start tunnel '{}'", tag)
    }

    pub mod validation {
        pub const TAG_EMPTY: &str = "Tunnel tag cannot be empty or whitespace-only";

        pub fn tag_too_long(tag: &str) -> String {
            format!("Tunnel tag too long (max 100 characters): {}", tag)
        }

        pub const CLI_ARGS_EMPTY: &str = "CLI arguments cannot be empty";

        pub fn failed(context: &str) -> String {
            format!("Failed to validate tunnel entry: {}", context)
        }

        pub fn duplicate_id(id: &str) -> String {
            format!("Duplicate tunnel ID found: {}", id)
        }
    }
}

pub mod binary {
    pub fn not_found(path: &str) -> String {
        format!(
            "wstunnel binary not found at path: {}. Please check the binary path configuration or use --wstunnel-path flag.",
            path
        )
    }

    pub fn not_found_simple(path: &str) -> String {
        format!(
            "wstunnel binary not found at {}. Please verify the binary path.",
            path
        )
    }

    pub fn permission_denied(path: &str) -> String {
        format!(
            "Permission denied executing wstunnel binary at {}. Check file permissions.",
            path
        )
    }
}

pub mod config {
    pub fn validation_failed(context: &str) -> String {
        format!("Config validation failed for {}", context)
    }

    pub fn corrupted(_path: &str, backup_path: &str, error: &str) -> String {
        format!(
            "Config file was corrupted and has been replaced with defaults. Backup saved to {}. Error: {}",
            backup_path, error
        )
    }

    pub fn corrupted_yaml(path: &str, error: &str) -> String {
        format!("Corrupted YAML config at {}: {}", path, error)
    }

    pub fn backup_created(path: &str) -> String {
        format!("Created backup of corrupted config at {}", path)
    }

    pub fn validation_failed_after_add() -> String {
        "Configuration validation failed after adding tunnel".to_string()
    }

    pub fn validation_failed_after_edit() -> String {
        "Configuration validation failed after editing tunnel".to_string()
    }

    pub const SAVE_FAILED: &str = "Failed to save configuration to disk";
    pub const GLOBAL_VALIDATION_FAILED: &str = "Global settings validation failed";

    pub fn unsupported_version(version: u32) -> String {
        format!(
            "Unsupported config version: {}. Expected version 1",
            version
        )
    }

    pub fn failed_to_create_default(path: &str) -> String {
        format!("Failed to create default config at {}", path)
    }

    pub fn failed_to_read(path: &str) -> String {
        format!("Failed to read config from {}", path)
    }

    pub fn failed_to_serialize() -> String {
        "Failed to serialize config to YAML".to_string()
    }

    pub fn failed_to_create_dir(error: &str) -> String {
        format!("Failed to create config directory: {}", error)
    }

    pub fn failed_to_write_temp(path: &str) -> String {
        format!("Failed to write temporary config to {}", path)
    }

    pub fn failed_to_rename(from: &str, to: &str) -> String {
        format!("Failed to rename {} to {}", from, to)
    }

    #[cfg(unix)]
    pub const FAILED_TO_OPEN_TEMP: &str = "Failed to open temp file for fsync";
    #[cfg(unix)]
    pub const FAILED_TO_FSYNC: &str = "Failed to fsync temp file";
    pub const FAILED_TO_CREATE_WATCHER: &str = "Failed to create file watcher";

    pub fn failed_to_watch(path: &str) -> String {
        format!("Failed to watch config file: {}", path)
    }
}

pub mod disk {
    pub const FULL: &str =
        "Disk space exhausted. Cannot save configuration. Free up disk space and try again.";

    pub fn full_log_write(error: &str) -> String {
        format!("Disk full - cannot write to log file: {}", error)
    }
}

pub mod logs {
    pub const FAILED_TO_CREATE_DIR: &str = "Failed to create log directory";

    pub const FAILED_TO_CREATE_FILE: &str = "Failed to create log file";

    pub fn not_found(path: &str) -> String {
        format!("Log file not found at: {}", path)
    }

    pub fn failed_to_open(error: &str) -> String {
        format!("Failed to open log file: {}", error)
    }

    pub fn failed_to_write_stdout(error: &str) -> String {
        format!("Failed to write stdout to log: {}", error)
    }

    pub fn failed_to_write_stderr(error: &str) -> String {
        format!("Failed to write stderr to log: {}", error)
    }

    pub fn failed_to_flush(error: &str) -> String {
        format!("Failed to flush log file: {}", error)
    }

    pub fn retention_invalid(days: u32) -> String {
        format!(
            "Log retention days must be between 1 and 3650 (10 years), got: {}",
            days
        )
    }
}

pub mod process {
    pub const PORT_IN_USE: &str =
        "Port is already in use. The tunnel may be using a port that is already bound.";

    pub fn spawn_failed(error: &str) -> String {
        format!("Failed to spawn wstunnel process: {}", error)
    }

    pub const FAILED_TO_GET_PID: &str = "Failed to get process ID";
    pub const FAILED_TO_PROCESS_PID: &str = "Failed to process ID after spawning tunnel";
    pub const FAILED_TO_CAPTURE_STDOUT: &str = "Failed to capture stdout";
    pub const FAILED_TO_CAPTURE_STDERR: &str = "Failed to capture stderr";
}
