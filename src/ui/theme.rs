use ratatui::style::{Color, Modifier, Style};

use crate::session::SessionStatus;

pub struct Theme;

impl Theme {
    pub fn header() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected() -> Style {
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    }

    pub fn status_color(status: &SessionStatus) -> Color {
        match status {
            SessionStatus::Active => Color::Green,
            SessionStatus::Idle => Color::Yellow,
            SessionStatus::Thinking => Color::Magenta,
            SessionStatus::Waiting => Color::Yellow,
            SessionStatus::Dead => Color::DarkGray,
        }
    }

    pub fn status(status: &SessionStatus) -> Style {
        let style = Style::default().fg(Self::status_color(status));
        match status {
            SessionStatus::Thinking | SessionStatus::Waiting => style.add_modifier(Modifier::BOLD),
            _ => style,
        }
    }

    pub fn cost() -> Style {
        Style::default().fg(Color::Yellow)
    }

    pub fn title() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn footer() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn label() -> Style {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    }

    pub fn value() -> Style {
        Style::default().fg(Color::White)
    }

    pub fn border() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn help_key() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn help_desc() -> Style {
        Style::default().fg(Color::White)
    }

    pub fn attached_marker() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn attached_bar() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn project_row() -> Style {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    }

    pub fn warm() -> Style {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    }

    pub fn bell() -> Style {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }

    pub fn command_bar() -> Style {
        Style::default().fg(Color::Yellow)
    }

    pub fn command_bar_label() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }
}
