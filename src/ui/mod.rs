pub mod messages;
pub mod screens;
pub mod state;
pub mod theme;

use crate::backend::Backend;
use crate::backend::types::{TunnelEntry, TunnelId, TunnelMode};
use crate::errors;
use messages::{ConfirmDeleteMessage, EditTunnelMessage, Message, TunnelListMessage};
use state::{ConfirmDeleteState, EditTunnelState, Screen};
use std::sync::{Arc, Mutex};

pub struct WstunnelManagerApp {
    screen: Screen,
    backend: Arc<Mutex<dyn Backend>>,
    tunnels: Vec<TunnelEntry>,
    theme: theme::WstunnelTheme,
}

impl WstunnelManagerApp {
    pub fn new(backend: Arc<Mutex<dyn Backend>>) -> Self {
        let tunnels = {
            let mut backend_lock = backend.lock().unwrap();

            if let Err(e) = backend_lock.cleanup_old_logs_if_configured() {
                tracing::warn!("Log cleanup failed: {}", e);
            }

            match backend_lock.start_autostart_tunnels() {
                Ok(results) => {
                    for (tunnel_id, result) in results {
                        match result {
                            Ok(pid) => {
                                tracing::info!(
                                    "UI: Autostart tunnel {:?} started with PID {}",
                                    tunnel_id,
                                    pid
                                );
                            }
                            Err(e) => {
                                tracing::error!(
                                    "UI: Autostart tunnel {:?} failed: {}",
                                    tunnel_id,
                                    e
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("UI: Failed to start autostart tunnels: {}", e);
                }
            }

            backend_lock.list_tunnels()
        };

        Self {
            screen: Screen::default(),
            backend,
            tunnels,
            theme: theme::WstunnelTheme::new(),
        }
    }

    pub fn title(&self) -> String {
        crate::constants::APP_TITLE.to_string()
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        match &self.screen {
            Screen::TunnelList(state) => {
                screens::tunnel_list::tunnel_list_view(state.clone(), self.tunnels.clone())
            }
            Screen::EditTunnel(state) => screens::edit_tunnel::edit_tunnel_view(state.clone()),
            Screen::ConfirmDelete(state) => {
                screens::tunnel_list::confirm_delete_view(state.clone())
            }
        }
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::TunnelList(tunnel_list_msg) => {
                self.handle_tunnel_list_message(tunnel_list_msg)
            }
            Message::EditTunnel(edit_tunnel_msg) => {
                self.handle_edit_tunnel_message(edit_tunnel_msg)
            }
            Message::ConfirmDelete(confirm_delete_msg) => {
                self.handle_confirm_delete_message(confirm_delete_msg)
            }
            Message::ProcessStatusChanged { id, status } => {
                self.handle_process_status_changed(id, status)
            }
            Message::ConfigReloaded(config) => self.handle_config_reloaded(config),
            Message::Error(error) => self.handle_error(error),
        }
    }

    fn handle_tunnel_list_message(&mut self, message: TunnelListMessage) -> iced::Task<Message> {
        match &mut self.screen {
            Screen::TunnelList(state) => match message {
                TunnelListMessage::AddTunnel => {
                    self.screen = Screen::EditTunnel(EditTunnelState::new_create());
                    iced::Task::none()
                }
                TunnelListMessage::EditTunnel(id) => {
                    let mut backend = self.backend.lock().unwrap();
                    match backend.get_tunnel(id) {
                        Some(tunnel) => {
                            self.screen = Screen::EditTunnel(EditTunnelState::new_edit(
                                tunnel.id,
                                tunnel.tag,
                                tunnel.cli_args,
                                tunnel.autostart,
                            ));
                        }
                        None => {
                            state.error_message =
                                Some(errors::tunnel::not_found(&format!("{:?}", id)));
                        }
                    }
                    iced::Task::none()
                }
                TunnelListMessage::DeleteTunnel(id) => {
                    let mut backend = self.backend.lock().unwrap();
                    match backend.get_tunnel(id) {
                        Some(tunnel) => {
                            self.screen = Screen::ConfirmDelete(ConfirmDeleteState::new(
                                tunnel.id, tunnel.tag,
                            ));
                        }
                        None => {
                            state.error_message =
                                Some(errors::tunnel::not_found(&format!("{:?}", id)));
                        }
                    }
                    iced::Task::none()
                }
                TunnelListMessage::StartTunnel(id) => {
                    let backend = Arc::clone(&self.backend);
                    iced::Task::perform(
                        async move {
                            let mut backend_lock = backend.lock().unwrap();
                            match backend_lock.start_tunnel(id) {
                                Ok(pid) => {
                                    let status = backend_lock.get_tunnel_status(id);
                                    Ok((id, status, pid))
                                }
                                Err(e) => Err(e.to_string()),
                            }
                        },
                        |result| match result {
                            Ok((id, status, _pid)) => Message::ProcessStatusChanged { id, status },
                            Err(error) => Message::Error(error),
                        },
                    )
                }
                TunnelListMessage::StopTunnel(id) => {
                    let backend = Arc::clone(&self.backend);
                    iced::Task::perform(
                        async move {
                            let mut backend_lock = backend.lock().unwrap();
                            match backend_lock.stop_tunnel(id) {
                                Ok(_) => {
                                    let status = backend_lock.get_tunnel_status(id);
                                    Ok((id, status))
                                }
                                Err(e) => Err(e.to_string()),
                            }
                        },
                        |result| match result {
                            Ok((id, status)) => Message::ProcessStatusChanged { id, status },
                            Err(error) => Message::Error(error),
                        },
                    )
                }
                TunnelListMessage::OpenLogs(id) => {
                    let backend = Arc::clone(&self.backend);
                    iced::Task::perform(
                        async move {
                            let backend_lock = backend.lock().unwrap();
                            match backend_lock.get_log_path(id) {
                                Some(path) => {
                                    if path.exists() {
                                        match open::that(&path) {
                                            Ok(_) => Ok(()),
                                            Err(e) => {
                                                Err(errors::logs::failed_to_open(&e.to_string()))
                                            }
                                        }
                                    } else {
                                        Err(errors::logs::not_found(&path.display().to_string()))
                                    }
                                }
                                None => Err(errors::tunnel::NO_LOGS.to_string()),
                            }
                        },
                        |result| match result {
                            Ok(_) => Message::TunnelList(TunnelListMessage::Refresh),
                            Err(error) => Message::Error(error),
                        },
                    )
                }
                TunnelListMessage::Refresh => {
                    self.refresh_tunnels();
                    iced::Task::none()
                }
                TunnelListMessage::DismissError => {
                    state.error_message = None;
                    iced::Task::none()
                }
            },
            Screen::EditTunnel(_) | Screen::ConfirmDelete(_) => iced::Task::none(),
        }
    }

    fn handle_edit_tunnel_message(&mut self, message: EditTunnelMessage) -> iced::Task<Message> {
        match &mut self.screen {
            Screen::EditTunnel(state) => match message {
                EditTunnelMessage::TagChanged(new_tag) => {
                    state.tag_input = new_tag;
                    iced::Task::none()
                }
                EditTunnelMessage::CliArgsChanged(new_args) => {
                    state.cli_args_input = new_args;
                    iced::Task::none()
                }
                EditTunnelMessage::AutostartToggled(checked) => {
                    state.autostart_checkbox = checked;
                    iced::Task::none()
                }
                EditTunnelMessage::Save => {
                    let entry = TunnelEntry {
                        id: match state.mode {
                            state::EditMode::Create => TunnelId::default(),
                            state::EditMode::Edit { id } => id,
                        },
                        tag: state.tag_input.clone(),
                        mode: TunnelMode::Client,
                        cli_args: state.cli_args_input.clone(),
                        autostart: state.autostart_checkbox,
                        runtime_state: None,
                    };

                    let backend = Arc::clone(&self.backend);
                    let mode = state.mode.clone();

                    iced::Task::perform(
                        async move {
                            let mut backend_lock = backend.lock().unwrap();

                            match mode {
                                state::EditMode::Create => {
                                    backend_lock.add_tunnel(entry).map_err(|e| e.to_string())
                                }
                                state::EditMode::Edit { id } => backend_lock
                                    .edit_tunnel(id, entry)
                                    .map(|_| id)
                                    .map_err(|e| e.to_string()),
                            }
                        },
                        |result| Message::EditTunnel(EditTunnelMessage::SaveCompleted(result)),
                    )
                }
                EditTunnelMessage::Cancel => {
                    self.screen = Screen::TunnelList(state::TunnelListState::default());
                    iced::Task::none()
                }
                EditTunnelMessage::SaveCompleted(result) => match result {
                    Ok(_tunnel_id) => {
                        self.screen = Screen::TunnelList(state::TunnelListState::default());
                        self.refresh_tunnels();
                        iced::Task::none()
                    }
                    Err(error) => {
                        state.validation_errors = vec![error];
                        iced::Task::none()
                    }
                },
            },
            Screen::TunnelList(_) | Screen::ConfirmDelete(_) => iced::Task::none(),
        }
    }

    fn handle_confirm_delete_message(
        &mut self,
        message: ConfirmDeleteMessage,
    ) -> iced::Task<Message> {
        match &self.screen {
            Screen::ConfirmDelete(state) => match message {
                ConfirmDeleteMessage::Confirm => {
                    let backend = Arc::clone(&self.backend);
                    let tunnel_id = state.tunnel_id;

                    self.screen = Screen::TunnelList(state::TunnelListState::default());

                    iced::Task::perform(
                        async move {
                            let mut backend_lock = backend.lock().unwrap();
                            backend_lock
                                .delete_tunnel(tunnel_id)
                                .map_err(|e| e.to_string())
                        },
                        |result| match result {
                            Ok(_) => Message::TunnelList(TunnelListMessage::Refresh),
                            Err(error) => Message::Error(error),
                        },
                    )
                }
                ConfirmDeleteMessage::Cancel => {
                    self.screen = Screen::TunnelList(state::TunnelListState::default());
                    iced::Task::none()
                }
            },
            Screen::TunnelList(_) | Screen::EditTunnel(_) => iced::Task::none(),
        }
    }

    fn handle_process_status_changed(
        &mut self,
        _id: crate::backend::types::TunnelId,
        _status: crate::backend::types::TunnelRuntimeState,
    ) -> iced::Task<Message> {
        self.refresh_tunnels();
        iced::Task::none()
    }

    fn handle_config_reloaded(
        &mut self,
        _config: Arc<crate::backend::types::Config>,
    ) -> iced::Task<Message> {
        self.refresh_tunnels();
        iced::Task::none()
    }

    fn handle_error(&mut self, error: String) -> iced::Task<Message> {
        match &mut self.screen {
            Screen::TunnelList(state) => {
                state.error_message = Some(error);
            }
            Screen::EditTunnel(state) => {
                state.validation_errors = vec![error];
            }
            Screen::ConfirmDelete(_) => {
                self.screen = Screen::TunnelList(state::TunnelListState {
                    scroll_position: 0.0,
                    error_message: Some(error),
                });
            }
        }
        iced::Task::none()
    }

    fn refresh_tunnels(&mut self) {
        let mut backend_lock = self.backend.lock().unwrap();
        self.tunnels = backend_lock.list_tunnels();
    }

    pub fn theme(&self) -> iced::Theme {
        self.theme.to_iced_theme()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::none()
    }
}
