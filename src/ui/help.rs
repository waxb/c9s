use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;

const BINDINGS: &[(&str, &str)] = &[
    ("j / Down", "Move down"),
    ("k / Up", "Move up"),
    ("g", "Jump to top"),
    ("G", "Jump to bottom"),
    ("d", "Session detail"),
    ("f", "Session files (in detail)"),
    ("a / Enter", "Attach (open terminal)"),
    ("1-9", "Attach to session by #"),
    ("Space", "Quick switcher"),
    ("n", "New session (enter path)"),
    ("Ctrl+b", "Resume picker for selected project"),
    ("x", "Kill selected session"),
    ("u", "Unfollow (hide) session"),
    ("/", "Filter sessions"),
    ("s", "Cycle sort column"),
    ("r", "Refresh session list"),
    ("L", "Toggle log panel"),
    ("Esc", "Back / clear filter"),
    ("q", "Quit"),
    ("Ctrl+c", "Force quit"),
    ("Ctrl+t", "Toggle side shell panel"),
    ("?", "Toggle this help"),
    ("", ""),
    ("In terminal", ""),
    ("Ctrl+d", "Detach (back to list)"),
    ("Ctrl+Space", "Quick switcher"),
    ("Ctrl+n / Ctrl+p", "Cycle next / prev session"),
    ("Ctrl+K / Ctrl+J", "Scroll history up / down"),
    ("Ctrl+t", "Toggle side shell panel"),
];

pub fn render_help(f: &mut Frame, area: Rect) {
    let popup_width = 52;
    let popup_height = (BINDINGS.len() as u16) + 4;

    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let lines: Vec<Line> = BINDINGS
        .iter()
        .map(|(key, desc)| {
            if key.is_empty() && desc.is_empty() {
                Line::from("")
            } else if desc.is_empty() {
                Line::from(Span::styled(format!("  -- {} --", key), Theme::footer()))
            } else {
                Line::from(vec![
                    Span::styled(format!("  {:<16}", key), Theme::help_key()),
                    Span::styled(*desc, Theme::help_desc()),
                ])
            }
        })
        .collect();

    let help = Paragraph::new(lines).block(
        Block::default()
            .title(" Keybindings ")
            .borders(Borders::ALL)
            .border_style(Theme::title()),
    );

    f.render_widget(help, popup_area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}
