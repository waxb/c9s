use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::TakeoverPrompt;

pub fn render_confirm_takeover(f: &mut Frame, prompt: &TakeoverPrompt, area: Rect) {
    let popup_area = centered_rect(64.min(area.width), 9, area);
    f.render_widget(Clear, popup_area);

    let title_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let key = |text: &str, color: Color| {
        Span::styled(
            text.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )
    };

    let (headline, choices) = match prompt.verified_pid {
        Some(pid) => (
            format!("  {} runs in another terminal (pid {})", prompt.label, pid),
            Line::from(vec![
                Span::raw("    "),
                key(" t: take over here ", Color::Red),
                Span::raw("  "),
                key(" f: fork instead ", Color::Cyan),
                Span::raw("  "),
                key(" Esc: cancel ", Color::Green),
            ]),
        ),
        None => (
            format!("  {} may be running elsewhere (unverified)", prompt.label),
            Line::from(vec![
                Span::raw("    "),
                key(" f: fork instead ", Color::Cyan),
                Span::raw("  "),
                key(" Esc: cancel ", Color::Green),
            ]),
        ),
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(headline, title_style)),
        Line::from(Span::styled(
            "  Resuming one session in two processes corrupts its transcript.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        choices,
    ];

    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" Session is live ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
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
