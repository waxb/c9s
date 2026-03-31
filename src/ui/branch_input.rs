use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::BranchInputState;

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}

pub fn render_branch_input(f: &mut Frame, state: &BranchInputState, area: Rect) {
    let popup_area = centered_rect(50, 8, area);
    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    let input_spans = vec![
        Span::styled("  Branch: ", Style::default().fg(Color::Cyan)),
        Span::styled(
            &state.input,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("_", Style::default().fg(Color::White)),
    ];
    lines.push(Line::from(input_spans));

    if let Some(ref suggestion) = state.suggestion {
        lines.push(Line::from(vec![
            Span::raw("           "),
            Span::styled(suggestion.as_str(), Style::default().fg(Color::DarkGray)),
        ]));
    } else {
        lines.push(Line::from(""));
    }

    if let Some(ref error) = state.error {
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", error),
            Style::default().fg(Color::Red),
        )]));
    } else {
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  Enter:create  Tab:complete  Esc:cancel",
        Style::default().fg(Color::DarkGray),
    )]));

    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" New Branch Session ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(dialog, popup_area);
}
