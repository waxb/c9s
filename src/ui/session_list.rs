use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use crate::app::{App, SessionEntry, ViewMode};
use crate::session::SessionStatus;
use crate::tervezo::ImplementationStatus;
use crate::ui::theme::Theme;
use crate::ui::usage_panel::render_usage_panel;

pub fn render_session_list(f: &mut Frame, app: &App, area: Rect) {
    let show_command_bar = app.is_filtering() || app.attached_session_id().is_some();

    let usage_height = if app.usage().api_available { 12 } else { 6 };

    let chunks = if show_command_bar {
        Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(usage_height),
            Constraint::Length(1),
        ])
        .split(area)
    } else {
        Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(0),
            Constraint::Min(5),
            Constraint::Length(usage_height),
            Constraint::Length(1),
        ])
        .split(area)
    };

    render_header(f, app, chunks[0]);
    if show_command_bar {
        render_command_bar(f, app, chunks[1]);
    }
    render_table(f, app, chunks[2]);
    let sessions = app.filtered_sessions();
    render_usage_panel(f, app.usage(), &sessions, chunks[3]);
    render_footer(f, app, chunks[4]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let sessions = app.filtered_sessions();
    let live_count = sessions
        .iter()
        .filter(|e| match e {
            SessionEntry::Local(s) => s.status != SessionStatus::Dead,
            SessionEntry::Remote(i) => i.status.is_running(),
        })
        .count();
    let total_count = sessions.len();

    let title = if app.has_tervezo() {
        let remote_count = app.remote_count();
        let local_total = sessions.iter().filter(|e| !e.is_remote()).count();
        format!(
            " c9s - Claude Code Sessions [{}/{} + {}T]",
            live_count, local_total, remote_count,
        )
    } else {
        format!(
            " c9s - Claude Code Sessions [{}/{}]",
            live_count, total_count,
        )
    };

    let sort_info = format!(" Sort: {} ", app.sort_label());

    let header = Line::from(vec![
        Span::styled(title, Theme::title()),
        Span::raw("  "),
        Span::styled(sort_info, Theme::footer()),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .style(Theme::border());
    let paragraph = Paragraph::new(header).block(block);
    f.render_widget(paragraph, area);
}

fn render_command_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();

    if app.is_filtering() {
        spans.push(Span::styled(" /", Theme::command_bar_label()));
        spans.push(Span::styled(
            app.filter_query().to_string(),
            Theme::command_bar(),
        ));
        if *app.view_mode() == ViewMode::Filter {
            spans.push(Span::styled("_", Theme::command_bar()));
        }
        spans.push(Span::raw("  "));
    }

    if let Some(sid) = app.attached_session_id() {
        let attached_name = app
            .filtered_sessions()
            .iter()
            .find(|e| e.id() == sid)
            .map(|e| e.display_name().to_string())
            .or_else(|| {
                app.all_sessions()
                    .iter()
                    .find(|s| s.id == sid)
                    .map(|s| s.project_name.clone())
            })
            .unwrap_or_else(|| sid[..8.min(sid.len())].to_string());

        spans.push(Span::styled(" >> ", Theme::attached_marker()));
        spans.push(Span::styled(
            format!("attached: {}", attached_name),
            Theme::attached_bar(),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}

fn render_table(f: &mut Frame, app: &App, area: Rect) {
    let header_cells = [
        "",
        "Project",
        "Branch",
        "Model",
        "Status",
        "Msgs",
        "Tokens In",
        "Tokens Out",
        "Cost",
        "Last Active",
    ]
    .iter()
    .map(|h| Cell::from(*h).style(Theme::header()));

    let header = Row::new(header_cells).height(1);

    let bell_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let sessions = app.filtered_sessions();
    let rows: Vec<Row> = sessions
        .iter()
        .map(|entry| {
            let entry_id = entry.id().to_string();
            let is_attached = app.is_attached(&entry_id);
            let has_bell = app.has_bell(&entry_id);

            let status_style = match entry {
                SessionEntry::Local(s) => match s.status {
                    SessionStatus::Active => Theme::status_active(),
                    SessionStatus::Idle => Theme::status_idle(),
                    SessionStatus::Thinking => Theme::status_thinking(),
                    SessionStatus::Dead => Theme::status_dead(),
                },
                SessionEntry::Remote(i) => match i.status {
                    ImplementationStatus::Running => Theme::status_active(),
                    ImplementationStatus::Pending | ImplementationStatus::Queued => {
                        Theme::status_idle()
                    }
                    ImplementationStatus::Completed | ImplementationStatus::Merged => {
                        Theme::tzv_status_done()
                    }
                    ImplementationStatus::Failed => Theme::tzv_status_failed(),
                    ImplementationStatus::Stopped | ImplementationStatus::Cancelled => {
                        Theme::status_dead()
                    }
                },
            };

            let model_short = match entry {
                SessionEntry::Local(s) => s
                    .model
                    .as_deref()
                    .map(shorten_model)
                    .unwrap_or("-".to_string()),
                SessionEntry::Remote(_) => "tervezo".to_string(),
            };

            let (marker, marker_style) = if entry.is_remote() {
                (
                    "[T]",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                let m = match (is_attached, has_bell) {
                    (true, true) => ">>*",
                    (true, false) => ">>",
                    (false, true) => " *",
                    (false, false) => "",
                };
                let s = if has_bell {
                    bell_style
                } else {
                    Theme::attached_marker()
                };
                (m, s)
            };

            let name_style = if has_bell {
                bell_style
            } else {
                Style::default()
            };

            let (tokens_in, tokens_out) = match entry {
                SessionEntry::Local(s) => (
                    format_tokens(s.input_tokens + s.cache_read_tokens),
                    format_tokens(s.output_tokens),
                ),
                SessionEntry::Remote(_) => ("-".to_string(), "-".to_string()),
            };

            let cost_str = match entry.estimated_cost() {
                Some(c) => format!("${:.2}", c),
                None => "-".to_string(),
            };

            let msg_str = match entry.message_count() {
                Some(m) => format_count(m as u64),
                None => "-".to_string(),
            };

            let cells = vec![
                Cell::from(marker).style(marker_style),
                Cell::from(entry.display_name().to_string()).style(name_style),
                Cell::from(entry.branch().unwrap_or("-").to_string()),
                Cell::from(model_short),
                Cell::from(entry.status_label()).style(status_style),
                Cell::from(msg_str),
                Cell::from(tokens_in),
                Cell::from(tokens_out),
                Cell::from(cost_str).style(Theme::cost()),
                Cell::from(entry.last_activity_display()),
            ];
            Row::new(cells)
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Min(20),
        Constraint::Length(15),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Theme::border()),
        )
        .row_highlight_style(Theme::selected());

    let mut state = TableState::default();
    state.select(Some(app.selected_index()));
    f.render_stateful_widget(table, area, &mut state);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let sessions = app.filtered_sessions();
    let total_cost: f64 = sessions.iter().filter_map(|e| e.estimated_cost()).sum();
    let total_tokens: u64 = sessions.iter().filter_map(|e| e.total_tokens()).sum();

    let version = env!("CARGO_PKG_VERSION");

    let stats = format!(
        " c9s {} | ${:.2} | {} tokens",
        version,
        total_cost,
        format_tokens(total_tokens)
    );

    let keys = "  a:attach  d:detail  Space:switch  1-9:jump  n:new  /:filter  s:sort  ?:help";

    let footer = Line::from(vec![
        Span::styled(stats, Theme::cost()),
        Span::styled(keys, Theme::footer()),
    ]);

    let paragraph = Paragraph::new(footer);
    f.render_widget(paragraph, area);
}

fn shorten_model(model: &str) -> String {
    if model.contains("opus") {
        "opus".to_string()
    } else if model.contains("sonnet") {
        "sonnet".to_string()
    } else if model.contains("haiku") {
        "haiku".to_string()
    } else {
        model.split('-').next_back().unwrap_or(model).to_string()
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

fn format_count(n: u64) -> String {
    if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
