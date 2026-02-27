use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::session::SessionStatus;

const POPUP_WIDTH: u16 = 62;
const NAME_COL: usize = 20;
const STATUS_COL: usize = 9;
const BRANCH_COL: usize = 20;

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        format!("{:<width$}", s, width = max)
    } else {
        format!("{}~", &s[..max - 1])
    }
}

pub fn render_qswitcher(f: &mut Frame, app: &App, area: Rect) {
    let sessions = app.filtered_sessions();

    if sessions.is_empty() {
        return;
    }

    let count = sessions.len().min(9);
    let popup_height = (count as u16) + 4;

    let popup_area = centered_rect(POPUP_WIDTH, popup_height, area);
    f.render_widget(Clear, popup_area);

    let attached_id = app.attached_session_id();
    let selected = app.selected_index();

    let lines: Vec<Line> = sessions
        .iter()
        .take(9)
        .enumerate()
        .map(|(i, session)| {
            let is_attached = attached_id == Some(session.id.as_str());
            let is_selected = i == selected;

            let base_mod = if is_selected {
                Modifier::REVERSED
            } else {
                Modifier::empty()
            };

            let status_fg = match session.status {
                SessionStatus::Active => Color::Green,
                SessionStatus::Idle => Color::Yellow,
                SessionStatus::Thinking => Color::Magenta,
                SessionStatus::Dead => Color::DarkGray,
            };
            let status_mod = if matches!(session.status, SessionStatus::Thinking) {
                base_mod | Modifier::BOLD
            } else {
                base_mod
            };

            let marker = if is_attached { ">>" } else { "  " };
            let marker_style = if is_attached {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD | base_mod)
            } else {
                Style::default().fg(Color::DarkGray).add_modifier(base_mod)
            };

            let branch = session.git_branch.as_deref().unwrap_or("");
            let branch_display = truncate(branch, BRANCH_COL);

            let name_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD | base_mod)
            } else {
                Style::default().fg(Color::White).add_modifier(base_mod)
            };

            Line::from(vec![
                Span::styled(format!(" {} ", marker), marker_style),
                Span::styled(
                    format!("{} ", i + 1),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | base_mod),
                ),
                Span::styled(truncate(&session.project_name, NAME_COL), name_style),
                Span::styled(
                    truncate(session.status.label(), STATUS_COL),
                    Style::default().fg(status_fg).add_modifier(status_mod),
                ),
                Span::styled(
                    branch_display,
                    Style::default().fg(Color::DarkGray).add_modifier(base_mod),
                ),
            ])
        })
        .collect();

    let block = Block::default()
        .title(" Quick Switch [1-9/Enter: attach  Esc: close] ")
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, popup_area);
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
