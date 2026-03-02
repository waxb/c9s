use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::log::{LogEntry, LogLevel};
use crate::ui::theme::Theme;

pub fn render_log_panel(f: &mut Frame, entries: &[LogEntry], scroll: usize, area: Rect) {
    let inner_height = area.height.saturating_sub(2) as usize; // borders top+bottom

    let lines: Vec<Line> = entries
        .iter()
        .map(|entry| {
            let ts = entry.timestamp.format("%H:%M:%S").to_string();
            let level_style = match entry.level {
                LogLevel::Info => Style::default().fg(Color::DarkGray),
                LogLevel::Warn => Style::default().fg(Color::Yellow),
                LogLevel::Error => Style::default().fg(Color::Red),
            };
            let msg_style = match entry.level {
                LogLevel::Error => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::White),
            };

            Line::from(vec![
                Span::styled(format!(" {} ", ts), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("[{:<5}] ", entry.level.label()), level_style),
                Span::styled(&entry.message, msg_style),
            ])
        })
        .collect();

    let total = lines.len();
    // Auto-scroll: if scroll is at or past the end, clamp to show the last page
    let max_scroll = total.saturating_sub(inner_height);
    let effective_scroll = scroll.min(max_scroll);

    let footer_text = format!(
        " L:back  j/k:scroll  g/G:top/bottom  c:clear  ({} entries) ",
        total
    );

    let block = Block::default()
        .title(" Log ")
        .title_bottom(Line::from(footer_text).centered())
        .borders(Borders::ALL)
        .border_style(Theme::border());

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((effective_scroll as u16, 0));

    f.render_widget(paragraph, area);
}
