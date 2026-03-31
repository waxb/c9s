use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn render_confirm_kill(
    f: &mut Frame,
    session_name: &str,
    branch: Option<&str>,
    worktree_path: Option<&str>,
    area: Rect,
) {
    let has_extra = branch.is_some() || worktree_path.is_some();
    let popup_width = 80;
    let popup_height = if has_extra { 9 } else { 7 };

    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let border_style = Style::default().fg(Color::Red);
    let title_style = Style::default()
        .fg(Color::Red)
        .add_modifier(Modifier::BOLD);
    let info_style = Style::default().fg(Color::Yellow);

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  Kill session: {}", session_name),
            title_style,
        )),
    ];

    if let Some(b) = branch {
        let display = if b.len() > 60 { &b[..57] } else { b };
        let suffix = if b.len() > 60 { "..." } else { "" };
        lines.push(Line::from(Span::styled(
            format!("  Branch: {}{}", display, suffix),
            info_style,
        )));
    }
    if let Some(wt) = worktree_path {
        let display = if wt.len() > 55 { &wt[wt.len()-55..] } else { wt };
        let prefix = if wt.len() > 55 { "..." } else { "" };
        lines.push(Line::from(Span::styled(
            format!("  Worktree: {}{}", prefix, display),
            info_style,
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("    ", Style::default()),
        Span::styled(
            " y/Enter: kill ",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ", Style::default()),
        Span::styled(
            " n/Esc: cancel ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" Kill Session? ")
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
