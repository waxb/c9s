use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{ConfirmRecreateWorktreeState, ConfirmWorktreeCleanupState};

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}

pub fn render_confirm_worktree_cleanup(
    f: &mut Frame,
    state: &ConfirmWorktreeCleanupState,
    area: Rect,
) {
    let popup_width = 55;
    let popup_height = 8;
    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    if state.is_dirty {
        lines.push(Line::from(Span::styled(
            "  WARNING: Worktree has uncommitted changes!",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  Branch: {}", state.branch),
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                " y: delete anyway ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   ", Style::default()),
            Span::styled(
                " N/Esc: keep (default) ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            format!("  Delete worktree for branch '{}'?", state.branch),
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(Span::styled(
            "  Worktree is clean.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                " Y/Enter: delete ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   ", Style::default()),
            Span::styled(
                " n/Esc: keep ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    let border_color = if state.is_dirty {
        Color::Red
    } else {
        Color::Yellow
    };

    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" Delete Worktree? ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)),
    );

    f.render_widget(dialog, popup_area);
}

pub fn render_confirm_recreate_worktree(
    f: &mut Frame,
    state: &ConfirmRecreateWorktreeState,
    area: Rect,
) {
    let popup_width = 55;
    let popup_height = 7;
    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  Worktree for branch '{}' was removed.", state.branch),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            "  Recreate it?",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                " Y/Enter: recreate ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   ", Style::default()),
            Span::styled(
                " n/Esc: cancel ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" Recreate Worktree? ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(dialog, popup_area);
}
