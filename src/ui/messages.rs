use crate::backend::types::{Config, TunnelId, TunnelRuntimeState};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum TunnelListMessage {
    AddTunnel,
    EditTunnel(TunnelId),
    DeleteTunnel(TunnelId),
    StartTunnel(TunnelId),
    StopTunnel(TunnelId),
    OpenLogs(TunnelId),
    Refresh,
    DismissError,
}

#[derive(Debug, Clone)]
pub enum EditTunnelMessage {
    TagChanged(String),
    CliArgsChanged(String),
    AutostartToggled(bool),
    Save,
    Cancel,
    SaveCompleted(Result<TunnelId, String>),
}

#[derive(Debug, Clone)]
pub enum ConfirmDeleteMessage {
    Confirm,
    Cancel,
}

#[derive(Debug, Clone)]
pub enum Message {
    TunnelList(TunnelListMessage),
    EditTunnel(EditTunnelMessage),
    ConfirmDelete(ConfirmDeleteMessage),
    ProcessStatusChanged {
        id: TunnelId,
        status: TunnelRuntimeState,
    },
    #[allow(dead_code)]
    ConfigReloaded(Arc<Config>),
    Error(String),
}
