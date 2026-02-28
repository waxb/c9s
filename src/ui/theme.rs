use ratatui::style::{Color, Modifier, Style};

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

    pub fn status_active() -> Style {
        Style::default().fg(Color::Green)
    }

    pub fn status_idle() -> Style {
        Style::default().fg(Color::Yellow)
    }

    pub fn status_thinking() -> Style {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    }

    pub fn status_dead() -> Style {
        Style::default().fg(Color::DarkGray)
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

    #[allow(dead_code)]
    pub fn filter_active() -> Style {
        Style::default().fg(Color::Yellow)
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

    pub fn command_bar() -> Style {
        Style::default().fg(Color::Yellow)
    }

    pub fn command_bar_label() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    // Tervezo-specific styles

    pub fn tzv_tab_active() -> Style {
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    }

    pub fn tzv_tab_inactive() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn tzv_diff_add() -> Style {
        Style::default().fg(Color::Green)
    }

    pub fn tzv_diff_remove() -> Style {
        Style::default().fg(Color::Red)
    }

    pub fn tzv_diff_header() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn tzv_status_done() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn tzv_status_failed() -> Style {
        Style::default().fg(Color::Red)
    }

    pub fn tzv_loading() -> Style {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::ITALIC)
    }

    pub fn tzv_remote_marker() -> Style {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    }

    pub fn tzv_timeline_icon() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn tzv_timeline_text() -> Style {
        Style::default().fg(Color::White)
    }
}
