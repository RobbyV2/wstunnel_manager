use crate::ui::messages::{EditTunnelMessage, Message};
use crate::ui::state::{EditMode, EditTunnelState};
use iced::widget::{Column, button, checkbox, column, container, row, text, text_input};
use iced::{Alignment, Color, Element, Length};

// T049-T050: edit_tunnel_view with validation error display
pub fn edit_tunnel_view(state: EditTunnelState) -> Element<'static, Message> {
    let title = match state.mode {
        EditMode::Create => "Add New Tunnel",
        EditMode::Edit { .. } => "Edit Tunnel",
    };

    let mut form_content = Column::new().spacing(15).padding(20);

    form_content = form_content.push(text(title).size(24));

    // Validation errors display
    if !state.validation_errors.is_empty() {
        let mut error_list = Column::new().spacing(5);
        for error in state.validation_errors.clone() {
            error_list = error_list.push(text(error).color(Color::from_rgb(0.8, 0.0, 0.0)));
        }
        let error_container =
            container(error_list)
                .padding(10)
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
        form_content = form_content.push(error_container);
    }

    // Tag input
    let tag_input = column![
        text("Tag/Name:").size(14),
        text_input(
            "Enter tunnel name (optional - generated ID if empty)",
            &state.tag_input
        )
        .on_input(|s| Message::EditTunnel(EditTunnelMessage::TagChanged(s)))
        .padding(8)
    ]
    .spacing(5);
    form_content = form_content.push(tag_input);

    // CLI args input
    let cli_args_input = column![
        text("CLI Arguments:").size(14),
        text_input("Enter wstunnel CLI arguments", &state.cli_args_input)
            .on_input(|s| Message::EditTunnel(EditTunnelMessage::CliArgsChanged(s)))
            .padding(8)
    ]
    .spacing(5);
    form_content = form_content.push(cli_args_input);

    // Autostart checkbox
    let autostart_cb = checkbox(
        "Start tunnel automatically on application startup",
        state.autostart_checkbox,
    )
    .on_toggle(|checked| Message::EditTunnel(EditTunnelMessage::AutostartToggled(checked)));
    form_content = form_content.push(autostart_cb);

    // Buttons
    let buttons = row![
        button("Save")
            .on_press(Message::EditTunnel(EditTunnelMessage::Save))
            .padding(10),
        button("Cancel")
            .on_press(Message::EditTunnel(EditTunnelMessage::Cancel))
            .padding(10)
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    form_content = form_content.push(buttons);

    container(form_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(20)
        .into()
}
