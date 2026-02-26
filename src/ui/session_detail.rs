use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::session::Session;
use crate::session::config::{ConfigItem, ConfigItemKind};
use crate::ui::theme::Theme;

pub fn render_session_detail(
    f: &mut Frame,
    session: &Session,
    items: &[ConfigItem],
    cursor: usize,
    preview: Option<&(String, String)>,
    preview_scroll: usize,
    area: Rect,
) {
    let branch = session.git_branch.as_deref().unwrap_or("");
    let title = if branch.is_empty() {
        format!(" Session: {} ", session.project_name)
    } else {
        format!(" Session: {} [{}] ", session.project_name, branch)
    };

    if let Some((name, content)) = preview {
        render_preview_layout(f, session, &title, name, content, preview_scroll, area);
    } else {
        render_tree_layout(f, session, &title, items, cursor, area);
    }
}

fn render_tree_layout(
    f: &mut Frame,
    session: &Session,
    title: &str,
    items: &[ConfigItem],
    cursor: usize,
    area: Rect,
) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(1),
    ])
    .split(area);

    let header = Paragraph::new(Line::from(Span::styled(title.to_string(), Theme::title())))
        .block(Block::default().borders(Borders::ALL).style(Theme::border()));
    f.render_widget(header, chunks[0]);

    let columns = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(30),
        Constraint::Percentage(40),
    ])
    .split(chunks[1]);

    render_info_column(f, session, columns[0]);
    render_usage_column(f, session, columns[1]);
    render_config_tree(f, items, cursor, columns[2]);

    let footer = Paragraph::new(Line::from(Span::styled(
        " Esc:back  a:attach  Up/Dn:navigate  Enter:preview file",
        Theme::footer(),
    )));
    f.render_widget(footer, chunks[2]);
}

fn render_preview_layout(
    f: &mut Frame,
    session: &Session,
    title: &str,
    filename: &str,
    content: &str,
    scroll: usize,
    area: Rect,
) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(1),
    ])
    .split(area);

    let header = Paragraph::new(Line::from(Span::styled(title.to_string(), Theme::title())))
        .block(Block::default().borders(Borders::ALL).style(Theme::border()));
    f.render_widget(header, chunks[0]);

    let columns = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(70),
    ])
    .split(chunks[1]);

    render_info_column(f, session, columns[0]);

    let lines: Vec<Line> = content.lines().map(|l| {
        Line::from(Span::styled(l.to_string(), Style::default().fg(Color::White)))
    }).collect();

    let total_lines = lines.len();
    let visible_height = columns[1].height.saturating_sub(2) as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    let effective_scroll = scroll.min(max_scroll);

    let visible: Vec<Line> = lines
        .into_iter()
        .skip(effective_scroll)
        .take(visible_height)
        .collect();

    let scroll_indicator = if max_scroll > 0 {
        format!(" {}/{} ", effective_scroll + 1, max_scroll + 1)
    } else {
        String::new()
    };

    let block = Block::default()
        .title(format!(" {} {}", filename, scroll_indicator))
        .borders(Borders::ALL)
        .style(Theme::border());
    let para = Paragraph::new(visible).block(block).wrap(Wrap { trim: false });
    f.render_widget(para, columns[1]);

    let footer = Paragraph::new(Line::from(Span::styled(
        " Esc:close preview  Up/Dn:scroll",
        Theme::footer(),
    )));
    f.render_widget(footer, chunks[2]);
}

fn render_config_tree(
    f: &mut Frame,
    items: &[ConfigItem],
    cursor: usize,
    area: Rect,
) {
    let visible_height = area.height.saturating_sub(2) as usize;
    if visible_height == 0 {
        return;
    }

    let scroll_offset = if cursor >= visible_height {
        cursor - visible_height + 1
    } else {
        0
    };

    let section = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let category = Style::default().fg(Color::White);
    let exists = Style::default().fg(Color::Green);
    let missing = Style::default().fg(Color::DarkGray);
    let memory = Style::default().fg(Color::Magenta);
    let selected_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, item)| {
            let is_selected = i == cursor;
            let label = format!("  {}", item.label);

            if is_selected {
                let has_file = item.path.is_some();
                let marker = if has_file { ">" } else { " " };
                Line::from(Span::styled(
                    format!("{} {}", marker, item.label),
                    selected_style,
                ))
            } else {
                let base_style = match item.kind {
                    ConfigItemKind::SectionHeader => section,
                    ConfigItemKind::Category => category,
                    ConfigItemKind::FileExists => exists,
                    ConfigItemKind::FileMissing => missing,
                    ConfigItemKind::MemoryFile => memory,
                };
                Line::from(Span::styled(label, base_style))
            }
        })
        .collect();

    let has_more_above = scroll_offset > 0;
    let has_more_below = scroll_offset + visible_height < items.len();
    let scroll_hint = match (has_more_above, has_more_below) {
        (true, true) => " [...]",
        (true, false) => " [top...]",
        (false, true) => " [...more]",
        _ => "",
    };

    let block = Block::default()
        .title(format!(" Config{}", scroll_hint))
        .borders(Borders::ALL)
        .style(Theme::border());
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

fn render_info_column(f: &mut Frame, session: &Session, area: Rect) {
    let mut lines = vec![
        kv_line("ID", &session.id[..8.min(session.id.len())]),
        kv_line("CWD", &session.cwd.to_string_lossy()),
        kv_line("Project", &session.project_name),
        kv_line(
            "Branch",
            session.git_branch.as_deref().unwrap_or("-"),
        ),
        kv_line(
            "Model",
            session.model.as_deref().unwrap_or("-"),
        ),
        kv_line("Status", session.status.label()),
        kv_line(
            "PID",
            &session.pid.map_or("-".to_string(), |p| p.to_string()),
        ),
        kv_line(
            "Version",
            session.claude_version.as_deref().unwrap_or("-"),
        ),
        kv_line(
            "Perm",
            session.permission_mode.as_deref().unwrap_or("-"),
        ),
    ];

    if !session.plan_slugs.is_empty() {
        let slugs = session.plan_slugs.join(", ");
        lines.push(kv_line("Plans", &slugs));
    }

    let block = Block::default()
        .title(" Info ")
        .borders(Borders::ALL)
        .style(Theme::border());
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

fn render_usage_column(f: &mut Frame, session: &Session, area: Rect) {
    let lines = vec![
        kv_line("In Tokens", &format_number(session.input_tokens)),
        kv_line("Out Tokens", &format_number(session.output_tokens)),
        kv_line("Cache Read", &format_number(session.cache_read_tokens)),
        kv_line("Cache Write", &format_number(session.cache_write_tokens)),
        kv_line("Messages", &session.message_count.to_string()),
        kv_line("Tool Calls", &session.tool_call_count.to_string()),
        cost_line("Cost", session.estimated_cost_usd()),
        kv_line("Compactions", &session.compaction_count.to_string()),
        kv_line(
            "Hooks",
            &format!(
                "{}/{}err",
                session.hook_run_count, session.hook_error_count
            ),
        ),
        kv_line("Last Active", &session.last_activity_display()),
        kv_line("Duration", &session.duration_display()),
    ];

    let block = Block::default()
        .title(" Usage ")
        .borders(Borders::ALL)
        .style(Theme::border());
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

fn kv_line(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<14}", label), Theme::label()),
        Span::styled(value.to_string(), Theme::value()),
    ])
}

fn cost_line(label: &str, cost: f64) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<14}", label), Theme::label()),
        Span::styled(format!("${:.4}", cost), Theme::cost()),
    ])
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
