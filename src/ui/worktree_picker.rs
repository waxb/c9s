use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::WorktreePickerState;

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}

pub fn render_worktree_picker(f: &mut Frame, state: &WorktreePickerState, area: Rect) {
    let item_count = state.worktrees.len();
    let popup_height = (item_count as u16).min(15) + 5;
    let popup_width = 60;
    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    for (i, wt) in state.worktrees.iter().enumerate() {
        let is_selected = i == state.cursor;
        let marker = if is_selected { " > " } else { "   " };
        let style = if is_selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let path_short = wt.path.to_string_lossy();
        let label = format!("{:<20} {}", wt.branch, path_short);
        lines.push(Line::from(vec![
            Span::styled(marker.to_string(), style),
            Span::styled(label, style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  Enter:attach  Esc:cancel",
        Style::default().fg(Color::DarkGray),
    )]));

    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" Existing Worktrees ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(dialog, popup_area);
}
