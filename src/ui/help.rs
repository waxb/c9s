use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;

const BINDINGS: &[(&str, &str)] = &[
    ("j / k", "Move down / up"),
    ("g / G", "Jump to top / bottom"),
    ("Space, l / h", "Expand / collapse project"),
    ("Enter / a", "Attach (project row: current session)"),
    ("1-9", "Attach Nth project's current session"),
    ("Ctrl+Space", "Quick switcher (l expands a project)"),
    ("n", "New session (directory, then name)"),
    ("F", "Fork selected session"),
    ("x", "Discard session (terminate + hide)"),
    ("u / U", "Hide session / unhide dialog"),
    ("Ctrl+b", "Resume picker for selected project"),
    ("d", "Session detail (f: transcript picker)"),
    ("/", "Filter sessions"),
    ("s", "Cycle sort column"),
    ("r", "Refresh session list"),
    ("L", "Toggle log panel"),
    ("Esc", "Back / clear filter"),
    ("q", "Quit"),
    ("Ctrl+t", "Toggle side shell pane"),
    ("?", "Toggle this help"),
    ("", ""),
    ("In terminal", ""),
    ("Ctrl+d", "Detach (back to list)"),
    ("Ctrl+Space", "Quick switcher"),
    ("Ctrl+n / Ctrl+p", "Next / prev project tab"),
    ("Ctrl+]", "Toggle session pane (configurable)"),
    ("Ctrl+q", "Toggle keep-warm pings (configurable)"),
    ("Ctrl+k / Ctrl+j", "Scroll history up / down"),
    ("", ""),
    ("Session pane", ""),
    ("Enter / 1-9", "Switch to session"),
    ("n / f / x / r", "New / fork / discard / rename"),
    ("m / M", "Message session / coordinate with it"),
    ("w", "Toggle keep-warm (red X marker)"),
];

pub fn render_help(f: &mut Frame, area: Rect) {
    let popup_width = 60;
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
                    Span::styled(format!("  {:<18}", key), Theme::help_key()),
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
