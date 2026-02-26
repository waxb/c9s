use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::ui::theme::Theme;

pub fn render_command_input(f: &mut Frame, input: &str, area: Rect) {
    let popup_width = 50;
    let popup_height = 3;

    let popup_area = centered_rect(popup_width, popup_height, area);
    f.render_widget(Clear, popup_area);

    let line = Line::from(vec![
        Span::styled("/new ", Theme::command_bar_label()),
        Span::styled(input.to_string(), Theme::command_bar()),
        Span::styled("_", Theme::command_bar()),
    ]);

    let block = Block::default()
        .title(" New Session ")
        .borders(Borders::ALL)
        .border_style(Theme::title());

    let paragraph = Paragraph::new(line).block(block);
    f.render_widget(paragraph, popup_area);
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
