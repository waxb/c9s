use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;

const POPUP_WIDTH: u16 = 60;

pub fn render_input_dialog(
    f: &mut Frame,
    title: &str,
    label: &str,
    value: &str,
    hint: &str,
    area: Rect,
) {
    let popup_area = centered_rect(POPUP_WIDTH.min(area.width), 4, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(vec![
            Span::styled(format!("{} ", label), Theme::command_bar_label()),
            Span::styled(value.to_string(), Theme::command_bar()),
            Span::styled("_", Theme::command_bar()),
        ]),
        Line::from(Span::styled(hint.to_string(), Theme::footer())),
    ];

    let block = Block::default()
        .title(title.to_string())
        .borders(Borders::ALL)
        .border_style(Theme::title());

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
