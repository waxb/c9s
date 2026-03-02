use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, SessionEntry};
use crate::session::SessionStatus;
use crate::tervezo::ImplementationStatus;

const POPUP_WIDTH: u16 = 65;
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
        .map(|(i, entry)| {
            let is_attached = attached_id == Some(entry.id());
            let is_selected = i == selected;

            let base_mod = if is_selected {
                Modifier::REVERSED
            } else {
                Modifier::empty()
            };

            let (status_fg, status_mod) = match entry {
                SessionEntry::Local(s) => {
                    let fg = match s.status {
                        SessionStatus::Active => Color::Green,
                        SessionStatus::Idle => Color::Yellow,
                        SessionStatus::Thinking => Color::Magenta,
                        SessionStatus::Dead => Color::DarkGray,
                    };
                    let m = if matches!(s.status, SessionStatus::Thinking) {
                        base_mod | Modifier::BOLD
                    } else {
                        base_mod
                    };
                    (fg, m)
                }
                SessionEntry::Remote(imp) => {
                    let fg = match imp.status {
                        ImplementationStatus::Running => Color::Green,
                        ImplementationStatus::Pending | ImplementationStatus::Queued => {
                            Color::Yellow
                        }
                        ImplementationStatus::Completed | ImplementationStatus::Merged => {
                            Color::Cyan
                        }
                        ImplementationStatus::Failed => Color::Red,
                        ImplementationStatus::Stopped | ImplementationStatus::Cancelled => {
                            Color::DarkGray
                        }
                    };
                    (fg, base_mod)
                }
            };

            let (marker, marker_style) = if entry.is_remote() {
                (
                    "[T]",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD | base_mod),
                )
            } else if is_attached {
                (
                    ">> ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | base_mod),
                )
            } else {
                (
                    "   ",
                    Style::default().fg(Color::DarkGray).add_modifier(base_mod),
                )
            };

            let branch = entry.branch().unwrap_or("");
            let branch_display = truncate(branch, BRANCH_COL);

            let name_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD | base_mod)
            } else {
                Style::default().fg(Color::White).add_modifier(base_mod)
            };

            Line::from(vec![
                Span::styled(format!(" {}", marker), marker_style),
                Span::styled(
                    format!("{} ", i + 1),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | base_mod),
                ),
                Span::styled(truncate(entry.display_name(), NAME_COL), name_style),
                Span::styled(
                    truncate(entry.status_label(), STATUS_COL),
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
