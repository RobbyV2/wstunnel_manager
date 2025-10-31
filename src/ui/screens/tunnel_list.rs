use crate::backend::types::{TunnelEntry, TunnelMode, TunnelRuntimeState};
use crate::ui::messages::{ConfirmDeleteMessage, Message, TunnelListMessage};
use crate::ui::state::{ConfirmDeleteState, TunnelListState};
use iced::widget::{Column, Container, button, column, container, row, scrollable, text};
use iced::{Alignment, Color, Element, Length};

pub fn status_indicator(state: &TunnelRuntimeState) -> Container<'static, Message> {
    let color = match state {
        TunnelRuntimeState::Running { .. } => Color::from_rgb(0.0, 0.8, 0.0), // green
        TunnelRuntimeState::Stopped => Color::from_rgb(0.8, 0.0, 0.0),        // red
        TunnelRuntimeState::Failed { .. } => Color::from_rgb(0.8, 0.0, 0.0),  // red
        TunnelRuntimeState::Starting => Color::from_rgb(0.8, 0.8, 0.0),       // yellow
    };

    container(text("●").size(20).color(color))
        .width(30)
        .center_x(30)
}

fn mode_badge(mode: TunnelMode) -> Container<'static, Message> {
    let (label, color) = match mode {
        TunnelMode::Client => ("CLIENT", Color::from_rgb(0.2, 0.5, 0.8)),
        TunnelMode::Server => ("SERVER", Color::from_rgb(0.5, 0.2, 0.8)),
    };

    container(text(label).size(12))
        .padding(4)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(color)),
            text_color: Some(Color::WHITE),
            border: iced::Border {
                color,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
}

fn tunnel_row(tunnel: TunnelEntry) -> Element<'static, Message> {
    let status = tunnel
        .runtime_state
        .as_ref()
        .unwrap_or(&TunnelRuntimeState::Stopped);

    let status_text = match status {
        TunnelRuntimeState::Running {
            pid, started_at, ..
        } => {
            format!(
                "Running (PID: {}, uptime: {}s)",
                pid,
                started_at.elapsed().as_secs()
            )
        }
        TunnelRuntimeState::Stopped => "Stopped".to_string(),
        TunnelRuntimeState::Failed { error, .. } => format!("Failed: {}", error),
        TunnelRuntimeState::Starting => "Starting...".to_string(),
    };

    let is_running = matches!(status, TunnelRuntimeState::Running { .. });
    let tunnel_id = tunnel.id;
    let tunnel_tag = tunnel.tag.clone();
    let tunnel_mode = tunnel.mode;

    let action_button = if is_running {
        button("Stop").on_press(Message::TunnelList(TunnelListMessage::StopTunnel(
            tunnel_id,
        )))
    } else {
        button("Start").on_press(Message::TunnelList(TunnelListMessage::StartTunnel(
            tunnel_id,
        )))
    };

    let row_content = row![
        status_indicator(status),
        container(text(tunnel_tag).size(16))
            .width(Length::Fixed(200.0))
            .padding(5),
        mode_badge(tunnel_mode),
        container(text(status_text).size(14))
            .width(Length::Fill)
            .padding(5),
        action_button,
        button("Edit").on_press(Message::TunnelList(TunnelListMessage::EditTunnel(
            tunnel_id
        ))),
        button("Logs").on_press(Message::TunnelList(TunnelListMessage::OpenLogs(tunnel_id))),
        button("Delete").on_press(Message::TunnelList(TunnelListMessage::DeleteTunnel(
            tunnel_id
        ))),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding(10);

    container(row_content)
        .width(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color::from_rgb(0.95, 0.95, 0.95))),
            border: iced::Border {
                color: Color::from_rgb(0.8, 0.8, 0.8),
                width: 1.0,
                radius: 5.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn empty_state_view() -> Element<'static, Message> {
    container(
        column![
            text("No tunnels configured").size(24),
            text("Click 'Add Tunnel' to create your first tunnel").size(16),
            button("Add Tunnel")
                .on_press(Message::TunnelList(TunnelListMessage::AddTunnel))
                .padding(10)
        ]
        .spacing(20)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

pub fn tunnel_list_view(
    state: TunnelListState,
    tunnels: Vec<TunnelEntry>,
) -> Element<'static, Message> {
    if tunnels.is_empty() {
        return empty_state_view();
    }

    let mut content = Column::new().spacing(10).padding(10);

    for tunnel in tunnels {
        content = content.push(tunnel_row(tunnel));
    }

    let scrollable_content = scrollable(content).height(Length::Fill).width(Length::Fill);

    let header = row![
        text("wstunnel Manager").size(24),
        container(button("Add Tunnel").on_press(Message::TunnelList(TunnelListMessage::AddTunnel)))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right),
        button("Refresh").on_press(Message::TunnelList(TunnelListMessage::Refresh)),
    ]
    .spacing(10)
    .padding(10)
    .align_y(Alignment::Center);

    let mut main_column = column![header, scrollable_content].spacing(0);

    if let Some(error_message) = state.error_message {
        let error_bar = container(
            row![
                text(error_message).color(Color::from_rgb(0.8, 0.0, 0.0)),
                button("Dismiss").on_press(Message::TunnelList(TunnelListMessage::DismissError))
            ]
            .spacing(10)
            .padding(10),
        )
        .width(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color::from_rgb(1.0, 0.9, 0.9))),
            border: iced::Border {
                color: Color::from_rgb(0.8, 0.0, 0.0),
                width: 2.0,
                radius: 5.0.into(),
            },
            ..Default::default()
        });
        main_column = main_column.push(error_bar);
    }

    container(main_column)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub fn confirm_delete_view(state: ConfirmDeleteState) -> Element<'static, Message> {
    let content = column![
        text("Delete Tunnel?").size(32),
        text(format!("Tunnel: {}", state.tunnel_name)).size(20),
        text("This will stop the tunnel if running and remove the configuration.")
            .size(14)
            .color(Color::from_rgb(0.6, 0.0, 0.0)),
        row![
            button("Cancel")
                .on_press(Message::ConfirmDelete(ConfirmDeleteMessage::Cancel))
                .padding(10),
            button("Delete")
                .on_press(Message::ConfirmDelete(ConfirmDeleteMessage::Confirm))
                .padding(10)
                .style(|theme: &iced::Theme, status| {
                    let _palette = theme.extended_palette();
                    match status {
                        button::Status::Active => button::Style {
                            background: Some(iced::Background::Color(Color::from_rgb(
                                0.8, 0.0, 0.0,
                            ))),
                            text_color: Color::WHITE,
                            border: iced::Border {
                                color: Color::from_rgb(0.6, 0.0, 0.0),
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..button::Style::default()
                        },
                        button::Status::Hovered => button::Style {
                            background: Some(iced::Background::Color(Color::from_rgb(
                                0.9, 0.0, 0.0,
                            ))),
                            text_color: Color::WHITE,
                            border: iced::Border {
                                color: Color::from_rgb(0.7, 0.0, 0.0),
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..button::Style::default()
                        },
                        _ => button::primary(theme, status),
                    }
                }),
        ]
        .spacing(20)
        .align_y(Alignment::Center),
    ]
    .spacing(20)
    .padding(20)
    .align_x(Alignment::Center);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
