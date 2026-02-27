use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::session::Session;
use crate::usage::UsageData;

pub fn render_usage_panel(
    f: &mut Frame,
    usage: &UsageData,
    sessions: &[&Session],
    area: Rect,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Usage ")
        .title_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    if usage.api_available {
        if let Some(pct) = usage.five_hour {
            let label = match &usage.plan_name {
                Some(p) => format!("5-hour window ({})", p),
                None => "5-hour window".to_string(),
            };
            lines.push(Line::from(Span::styled(
                format!(" {}", label),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )));
            lines.push(render_bar(pct, inner.width.saturating_sub(2)));
            if let Some(ref reset) = usage.five_hour_reset {
                lines.push(Line::from(Span::styled(
                    format!(" Resets {}", reset),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        if let Some(pct) = usage.seven_day {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Weekly (all models)".to_string(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )));
            lines.push(render_bar(pct, inner.width.saturating_sub(2)));
            if let Some(ref reset) = usage.seven_day_reset {
                lines.push(Line::from(Span::styled(
                    format!(" Resets {}", reset),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        if lines.len() < inner.height as usize {
            lines.push(Line::from(""));
        }
    }

    let today_cost: f64 = sessions.iter().map(|s| s.estimated_cost_usd()).sum();
    let today_tokens: u64 = sessions.iter().map(|s| s.total_tokens()).sum();
    let live_count = sessions.iter().filter(|s| s.pid.is_some()).count();
    let total_count = sessions.len();

    let mut model_tokens: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    for s in sessions {
        let model_key = match s.model.as_deref() {
            Some(m) if m.contains("opus") => "opus",
            Some(m) if m.contains("sonnet") => "sonnet",
            Some(m) if m.contains("haiku") => "haiku",
            _ => "other",
        };
        *model_tokens.entry(model_key).or_default() += s.total_tokens();
    }

    let stats_line = format!(
        " ${:.2} | {} tokens | {} live / {} total",
        today_cost,
        format_tokens(today_tokens),
        live_count,
        total_count,
    );
    lines.push(Line::from(Span::styled(
        stats_line,
        Style::default().fg(Color::Yellow),
    )));

    let mut model_parts: Vec<String> = Vec::new();
    for key in &["opus", "sonnet", "haiku"] {
        if let Some(&t) = model_tokens.get(key) {
            if t > 0 {
                model_parts.push(format!("{} {}", key, format_tokens(t)));
            }
        }
    }
    if let Some(&t) = model_tokens.get("other") {
        if t > 0 {
            model_parts.push(format!("other {}", format_tokens(t)));
        }
    }
    if !model_parts.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" Models: {}", model_parts.join(" | ")),
            Style::default().fg(Color::DarkGray),
        )));
    }

    let chunks = Layout::vertical([Constraint::Min(0)]).split(inner);
    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, chunks[0]);
}

fn render_bar(pct: u8, width: u16) -> Line<'static> {
    let bar_width = (width as usize).saturating_sub(14);
    let filled = (bar_width as f64 * pct as f64 / 100.0).round() as usize;
    let empty = bar_width.saturating_sub(filled);

    let color = if pct >= 80 {
        Color::Red
    } else if pct >= 50 {
        Color::Yellow
    } else {
        Color::Green
    };

    let bar_filled = "\u{2588}".repeat(filled);
    let bar_empty = " ".repeat(empty);
    let label = format!(" {}% used", pct);

    Line::from(vec![
        Span::raw(" "),
        Span::styled(bar_filled, Style::default().fg(color)),
        Span::styled(bar_empty, Style::default()),
        Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ])
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
