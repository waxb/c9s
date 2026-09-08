use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::session::Session;
use crate::ui::session_list::session_title;
use crate::ui::theme::Theme;

pub fn render_unhide_dialog(f: &mut Frame, hidden: &[&Session], cursor: usize, area: Rect) {
    let popup_height = (hidden.len().max(1) as u16 + 4).min(area.height.saturating_sub(2));
    let popup_area = centered_rect(64.min(area.width), popup_height, area);
    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    if hidden.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no hidden sessions",
            Theme::footer(),
        )));
    }
    for (i, session) in hidden.iter().enumerate() {
        let style = if i == cursor {
            Theme::selected()
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(
            format!(
                "  {}  {}  {}",
                session_title(session),
                session.status.label(),
                session.last_activity_display()
            ),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Enter:unhide  Esc:close",
        Theme::footer(),
    )));

    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" Hidden sessions ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(dialog, popup_area);
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
