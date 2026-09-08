use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, Row};
use crate::session::{Project, Session};
use crate::ui::session_list::session_title;
use crate::ui::theme::Theme;

const POPUP_WIDTH: u16 = 72;
const NAME_COL: usize = 26;
const STATUS_COL: usize = 9;
const BRANCH_COL: usize = 20;
const MAX_ROWS: usize = 12;

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

fn base_modifier(is_selected: bool) -> Modifier {
    if is_selected {
        Modifier::REVERSED
    } else {
        Modifier::empty()
    }
}

fn project_line(
    app: &App,
    index: usize,
    project: &Project,
    is_selected: bool,
    expanded: bool,
) -> Line<'static> {
    let visible = app.visible_sessions(project);
    let is_attached = visible
        .iter()
        .any(|s| app.terminal_manager().is_attached(&s.id));
    let has_bell = visible.iter().any(|s| app.has_bell(&s.id));
    let base_mod = base_modifier(is_selected);
    let (marker, marker_style) = marker_for(is_attached, has_bell, base_mod);
    let current = app.current_visible_session(project);
    let current_pos = current
        .and_then(|c| visible.iter().position(|s| s.id == c.id))
        .map(|p| p + 1)
        .unwrap_or(0);
    let arrow = if expanded { "v" } else { ">" };
    let label = format!(
        "{} {} [{}/{}]",
        arrow,
        project.name,
        current_pos,
        visible.len()
    );
    let status_span = match current {
        Some(s) => Span::styled(
            truncate(s.status.label(), STATUS_COL),
            Theme::status(&s.status).add_modifier(base_mod),
        ),
        None => Span::styled(truncate("", STATUS_COL), Style::default()),
    };
    let branch = current
        .and_then(|s| s.git_branch.clone())
        .unwrap_or_default();

    Line::from(vec![
        Span::styled(format!(" {}", marker), marker_style),
        Span::styled(
            format!("{} ", index + 1),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | base_mod),
        ),
        Span::styled(
            truncate(&label, NAME_COL),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | base_mod),
        ),
        status_span,
        Span::styled(
            truncate(&branch, BRANCH_COL),
            Style::default().fg(Color::DarkGray).add_modifier(base_mod),
        ),
    ])
}

fn session_line(app: &App, session: &Session, is_selected: bool) -> Line<'static> {
    let is_attached = app.terminal_manager().is_attached(&session.id);
    let has_bell = app.has_bell(&session.id);
    let base_mod = base_modifier(is_selected);
    let (marker, marker_style) = marker_for(is_attached, has_bell, base_mod);
    let label = format!("    {}", session_title(session));

    Line::from(vec![
        Span::styled(format!(" {}", marker), marker_style),
        Span::styled("  ", Style::default().add_modifier(base_mod)),
        Span::styled(
            truncate(&label, NAME_COL),
            Style::default().fg(Color::White).add_modifier(base_mod),
        ),
        Span::styled(
            truncate(session.status.label(), STATUS_COL),
            Theme::status(&session.status).add_modifier(base_mod),
        ),
        Span::styled(
            truncate(session.git_branch.as_deref().unwrap_or(""), BRANCH_COL),
            Style::default().fg(Color::DarkGray).add_modifier(base_mod),
        ),
    ])
}

pub fn render_qswitcher(f: &mut Frame, app: &App, area: Rect) {
    let rows = app.quick_switch_rows();
    if rows.is_empty() {
        return;
    }

    let selected = app.quick_switch().selected;
    let window_start = selected.saturating_sub(MAX_ROWS.saturating_sub(1));
    let visible_rows: Vec<(usize, Row)> = rows
        .iter()
        .copied()
        .enumerate()
        .skip(window_start)
        .take(MAX_ROWS)
        .collect();

    let popup_height = (visible_rows.len() as u16) + 4;
    let popup_area = centered_rect(POPUP_WIDTH, popup_height, area);
    f.render_widget(Clear, popup_area);

    let mut project_counter = rows
        .iter()
        .take(window_start)
        .filter(|r| matches!(r, Row::Project(_)))
        .count();
    let lines: Vec<Line> = visible_rows
        .iter()
        .filter_map(|(i, row)| {
            let project = app.project(row.project_index())?;
            let is_selected = *i == selected;
            Some(match row {
                Row::Project(_) => {
                    let index = project_counter;
                    project_counter += 1;
                    let expanded =
                        app.quick_switch().expanded.as_deref() == Some(project.root.as_path());
                    project_line(app, index, project, is_selected, expanded)
                }
                Row::Session(_, s) => session_line(app, project.sessions.get(*s)?, is_selected),
            })
        })
        .collect();

    let block = Block::default()
        .title(" Quick Switch [1-9/Enter: attach  l/h: expand/collapse  Esc: close] ")
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
