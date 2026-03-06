use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::NewSessionMenuState;

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}

pub fn render_new_session_menu(f: &mut Frame, state: &Option<NewSessionMenuState>, area: Rect) {
    let state = match state {
        Some(s) => s,
        None => return,
    };

    let item_count = state.items.len();
    let popup_height = (item_count as u16) + 4;
    let popup_width = 28;
    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    for (i, option) in state.items.iter().enumerate() {
        let is_selected = i == state.cursor;
        let marker = if is_selected { " > " } else { "   " };
        let style = if is_selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::from(vec![
            Span::styled(marker.to_string(), style),
            Span::styled(option.label().to_string(), style),
        ]));
    }

    let border_style = Style::default().fg(Color::Cyan);
    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" New Session ")
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    f.render_widget(dialog, popup_area);
}
