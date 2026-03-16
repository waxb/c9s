use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::session::SessionFile;

pub fn render_session_file_picker(
    f: &mut Frame,
    files: &[SessionFile],
    cursor: usize,
    area: Rect,
) {
    let popup_height = (files.len() as u16 + 4).min(area.height.saturating_sub(4));
    let popup_width = 72.min(area.width.saturating_sub(4));

    let popup_area = centered_rect(popup_width, popup_height, area);
    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    for (i, file) in files.iter().enumerate() {
        let is_selected = i == cursor;

        let id_short = &file.session_id[..8.min(file.session_id.len())];

        let size = if file.size_bytes >= 1_000_000 {
            format!("{:.1}MB", file.size_bytes as f64 / 1_000_000.0)
        } else {
            format!("{:.0}KB", file.size_bytes as f64 / 1_000.0)
        };

        let age = file
            .last_modified
            .map(|t| {
                let diff = chrono::Utc::now().signed_duration_since(t);
                let secs = diff.num_seconds();
                if secs < 3600 {
                    format!("{}m ago", secs / 60)
                } else if secs < 86400 {
                    format!("{}h ago", secs / 3600)
                } else {
                    format!("{}d ago", secs / 86400)
                }
            })
            .unwrap_or_else(|| "?".to_string());

        let current_marker = if file.is_current { " *" } else { "" };

        let text = format!(
            "  {} | {:>7} | {:>4} msgs | {:>8}{}",
            id_short, size, file.message_count, age, current_marker
        );

        let style = if is_selected {
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if file.is_current {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(Span::styled(text, style)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Enter:select  Esc:cancel  * = current",
        Style::default().fg(Color::DarkGray),
    )));

    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" Session Files ")
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
