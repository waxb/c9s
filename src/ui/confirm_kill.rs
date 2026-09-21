use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn render_confirm_kill(f: &mut Frame, session_name: &str, area: Rect) {
    render_confirm(
        f,
        " Discard Session? ",
        "Discard session",
        " y/Enter: discard ",
        Color::Red,
        session_name,
        area,
    );
}

pub fn render_confirm_hide(f: &mut Frame, session_name: &str, area: Rect) {
    render_confirm(
        f,
        " Hide Session? ",
        "Hide session",
        " y/Enter: hide ",
        Color::Yellow,
        session_name,
        area,
    );
}

fn render_confirm(
    f: &mut Frame,
    title: &str,
    verb: &str,
    accept: &str,
    color: Color,
    session_name: &str,
    area: Rect,
) {
    let popup_width = 50;
    let popup_height = 7;

    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let border_style = Style::default().fg(color);
    let title_style = Style::default().fg(color).add_modifier(Modifier::BOLD);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}: {}", verb, session_name),
            title_style,
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                accept.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("   ", Style::default()),
            Span::styled(
                " n/Esc: cancel ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(title.to_string())
            .borders(Borders::ALL)
            .border_style(border_style),
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
