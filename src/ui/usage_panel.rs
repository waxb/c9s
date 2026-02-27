use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::session::Session;
use crate::usage::UsageData;

pub fn render_usage_panel(f: &mut Frame, usage: &UsageData, sessions: &[&Session], area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Usage ")
        .title_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let bar_width = inner.width.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = Vec::new();

    if usage.api_available {
        if let Some(pct) = usage.five_hour {
            let title = match &usage.plan_name {
                Some(p) => format!("Current session ({})", p),
                None => "Current session".to_string(),
            };
            lines.push(make_title(&title));
            lines.push(make_bar(pct, bar_width));
            if let Some(ref reset) = usage.five_hour_reset {
                lines.push(make_reset(reset));
            }
        }

        if let Some(pct) = usage.seven_day {
            lines.push(make_title("Current week (all models)"));
            lines.push(make_bar(pct, bar_width));
            if let Some(ref reset) = usage.seven_day_reset {
                lines.push(make_reset(reset));
            }
        }
    }

    let today_cost: f64 = sessions.iter().map(|s| s.estimated_cost_usd()).sum();
    let today_tokens: u64 = sessions.iter().map(|s| s.total_tokens()).sum();
    let live_count = sessions.iter().filter(|s| s.pid.is_some()).count();
    let total_count = sessions.len();

    let mut model_tokens: [(&str, u64); 4] =
        [("opus", 0), ("sonnet", 0), ("haiku", 0), ("other", 0)];
    for s in sessions {
        let idx = match s.model.as_deref() {
            Some(m) if m.contains("opus") => 0,
            Some(m) if m.contains("sonnet") => 1,
            Some(m) if m.contains("haiku") => 2,
            _ => 3,
        };
        model_tokens[idx].1 += s.total_tokens();
    }

    lines.push(make_title("Sessions"));

    let stats = format!(
        " ${:.2} | {} tokens | {} live / {} total",
        today_cost,
        format_tokens(today_tokens),
        live_count,
        total_count,
    );
    lines.push(Line::from(Span::styled(
        stats,
        Style::default().fg(Color::White),
    )));

    let model_parts: Vec<String> = model_tokens
        .iter()
        .filter(|(_, t)| *t > 0)
        .map(|(name, t)| format!("{} {}", name, format_tokens(*t)))
        .collect();
    if !model_parts.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" Models: {}", model_parts.join(" | ")),
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn make_title(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!(" {}", text),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn make_bar(pct: u8, total_width: usize) -> Line<'static> {
    let label = format!("{}% used", pct);
    let label_len = label.len() + 1;
    let bar_max = total_width.saturating_sub(label_len + 2);
    let filled = (bar_max as f64 * pct as f64 / 100.0).round() as usize;
    let empty = bar_max.saturating_sub(filled);

    let color = bar_color(pct);
    let bar_filled = "\u{2588}".repeat(filled);
    let bar_empty = "\u{2591}".repeat(empty);

    Line::from(vec![
        Span::raw(" "),
        Span::styled(bar_filled, Style::default().fg(color)),
        Span::styled(bar_empty, Style::default().fg(Color::Indexed(238))),
        Span::raw(" "),
        Span::styled(label, Style::default().fg(Color::White)),
    ])
}

fn make_reset(reset: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!(" Resets {}", reset),
        Style::default().fg(Color::DarkGray),
    ))
}

fn bar_color(pct: u8) -> Color {
    if pct >= 80 {
        Color::Red
    } else if pct >= 50 {
        Color::Yellow
    } else {
        Color::Indexed(75)
    }
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
