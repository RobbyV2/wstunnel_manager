use std::path::PathBuf;

pub const APP_TITLE: &str = "wstunnel Manager";

pub fn default_log_directory() -> PathBuf {
    PathBuf::from(".").join("logs")
}
