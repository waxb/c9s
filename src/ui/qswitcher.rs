use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::session::Session;
use crate::ui::theme::Theme;

const POPUP_WIDTH: u16 = 65;
const NAME_COL: usize = 20;
const STATUS_COL: usize = 9;
const BRANCH_COL: usize = 20;
const MAX_ROWS: usize = 9;

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        format!("{:<width$}", s, width = max)
    } else {
        let head: String = chars[..max.saturating_sub(1)].iter().collect();
        format!("{}~", head)
    }
}

fn marker_for(is_attached: bool, has_bell: bool, base_mod: Modifier) -> (&'static str, Style) {
    let cyan = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | base_mod);
    match (is_attached, has_bell) {
        (true, true) => (">>*", cyan),
        (true, false) => (">> ", cyan),
        (false, true) => (" * ", Theme::bell().add_modifier(base_mod)),
        (false, false) => (
            "   ",
            Style::default().fg(Color::DarkGray).add_modifier(base_mod),
        ),
    }
}

fn session_line(app: &App, index: usize, session: &Session, is_selected: bool) -> Line<'static> {
    let is_attached = app.attached_session_id() == Some(session.id.as_str());
    let has_bell = app.has_bell(&session.id);

    let base_mod = if is_selected {
        Modifier::REVERSED
    } else {
        Modifier::empty()
    };

    let status_style = Theme::status(&session.status).add_modifier(base_mod);
    let (marker, marker_style) = marker_for(is_attached, has_bell, base_mod);

    let name_mod = if is_selected {
        Modifier::BOLD | base_mod
    } else {
        base_mod
    };

    Line::from(vec![
        Span::styled(format!(" {}", marker), marker_style),
        Span::styled(
            format!("{} ", index + 1),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | base_mod),
        ),
        Span::styled(
            truncate(&session.project_name, NAME_COL),
            Style::default().fg(Color::White).add_modifier(name_mod),
        ),
        Span::styled(truncate(session.status.label(), STATUS_COL), status_style),
        Span::styled(
            truncate(session.git_branch.as_deref().unwrap_or(""), BRANCH_COL),
            Style::default().fg(Color::DarkGray).add_modifier(base_mod),
        ),
    ])
}

pub fn render_qswitcher(f: &mut Frame, app: &App, area: Rect) {
    let sessions = app.filtered_sessions();
    if sessions.is_empty() {
        return;
    }

    let count = sessions.len().min(MAX_ROWS);
    let popup_height = (count as u16) + 4;
    let popup_area = centered_rect(POPUP_WIDTH, popup_height, area);
    f.render_widget(Clear, popup_area);

    let selected = app.selected_index();
    let lines: Vec<Line> = sessions
        .iter()
        .take(MAX_ROWS)
        .enumerate()
        .map(|(i, s)| session_line(app, i, s, i == selected))
        .collect();

    let block = Block::default()
        .title(" Quick Switch [1-9/Enter: attach  Esc: close] ")
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(Paragraph::new(lines).block(block), popup_area);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_pads_and_cuts() {
        assert_eq!(truncate("ab", 4), "ab  ");
        assert_eq!(truncate("abcdef", 4), "abc~");
        assert_eq!(
            truncate("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}", 3),
            "\u{e9}\u{e9}~"
        );
    }
}
