use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::session::Session;
use crate::ui::session_list::format_tokens;
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

    let total_cost: f64 = sessions.iter().map(|s| s.estimated_cost_usd()).sum();
    let total_tokens: u64 = sessions.iter().map(|s| s.total_tokens()).sum();
    let live_count = sessions.iter().filter(|s| s.pid.is_some()).count();

    lines.push(make_title("Sessions"));
    lines.push(Line::from(Span::styled(
        format!(
            " ${:.2} | {} tokens | {} live / {} total",
            total_cost,
            format_tokens(total_tokens),
            live_count,
            sessions.len(),
        ),
        Style::default().fg(Color::White),
    )));

    let model_parts = model_breakdown(sessions);
    if !model_parts.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" Models: {}", model_parts.join(" | ")),
            Style::default().fg(Color::DarkGray),
        )));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn model_breakdown(sessions: &[&Session]) -> Vec<String> {
    let families = ["fable", "opus", "sonnet", "haiku"];
    let mut totals: Vec<(&str, u64)> = families.iter().map(|f| (*f, 0)).collect();
    let mut other: u64 = 0;
    for s in sessions {
        let model = s.model.as_deref().unwrap_or("");
        match totals.iter_mut().find(|(f, _)| model.contains(f)) {
            Some(entry) => entry.1 += s.total_tokens(),
            None => other += s.total_tokens(),
        }
    }
    totals.push(("other", other));
    totals
        .iter()
        .filter(|(_, t)| *t > 0)
        .map(|(name, t)| format!("{} {}", name, format_tokens(*t)))
        .collect()
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
