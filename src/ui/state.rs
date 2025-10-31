use crate::backend::types::TunnelId;

#[derive(Debug, Clone)]
pub struct TunnelListState {
    #[allow(dead_code)]
    pub scroll_position: f32,
    pub error_message: Option<String>,
}

impl Default for TunnelListState {
    fn default() -> Self {
        Self {
            scroll_position: 0.0,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EditMode {
    Create,
    Edit { id: TunnelId },
}

#[derive(Debug, Clone)]
pub struct EditTunnelState {
    pub mode: EditMode,
    pub tag_input: String,
    pub cli_args_input: String,
    pub autostart_checkbox: bool,
    pub validation_errors: Vec<String>,
}

impl EditTunnelState {
    pub fn new_create() -> Self {
        Self {
            mode: EditMode::Create,
            tag_input: String::new(),
            cli_args_input: String::new(),
            autostart_checkbox: false,
            validation_errors: Vec::new(),
        }
    }

    pub fn new_edit(id: TunnelId, tag: String, cli_args: String, autostart: bool) -> Self {
        Self {
            mode: EditMode::Edit { id },
            tag_input: tag,
            cli_args_input: cli_args,
            autostart_checkbox: autostart,
            validation_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfirmDeleteState {
    pub tunnel_id: TunnelId,
    pub tunnel_name: String,
}

impl ConfirmDeleteState {
    pub fn new(tunnel_id: TunnelId, tunnel_name: String) -> Self {
        Self {
            tunnel_id,
            tunnel_name,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Screen {
    TunnelList(TunnelListState),
    EditTunnel(EditTunnelState),
    ConfirmDelete(ConfirmDeleteState),
}

impl Default for Screen {
    fn default() -> Self {
        Screen::TunnelList(TunnelListState::default())
    }
}
