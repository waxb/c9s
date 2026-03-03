use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::terminal::EmbeddedTerminal;
use crate::ui::terminal_view::render_screen;

pub fn split_with_side_panel(area: Rect) -> (Rect, Rect) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).split(area);
    (chunks[0], chunks[1])
}

pub fn render_side_panel(f: &mut Frame, terminal: &EmbeddedTerminal, focused: bool, area: Rect) {
    if area.width < 3 || area.height < 2 {
        return;
    }

    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(inner);

    let indicator = if focused { ">" } else { " " };
    let header_style = if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let header = Line::from(vec![
        Span::styled(format!("{}shell", indicator), header_style),
        Span::styled(" C-t:close", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    let guard = terminal.lock_parser();
    let screen = guard.screen();
    render_screen(f, screen, chunks[1]);
}
