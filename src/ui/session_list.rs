use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use crate::app::{App, ViewMode};
use crate::session::SessionStatus;
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
    let live_count = app
        .filtered_sessions()
        .iter()
        .filter(|s| s.status != SessionStatus::Dead)
        .count();
    let total_count = app.filtered_sessions().len();

    let title = format!(
        " c9s - Claude Code Sessions [{}/{}]",
        live_count, total_count,
    );

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
            .find(|s| s.id == sid)
            .map(|s| s.project_name.clone())
            .or_else(|| {
                app.all_sessions()
                    .iter()
                    .find(|s| s.id == sid)
                    .map(|s| s.project_name.clone())
            })
            .unwrap_or_else(|| sid[..8].to_string());

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
        .map(|session| {
            let is_attached = app.is_attached(&session.id);
            let has_bell = app.has_bell(&session.id);

            let status_style = match session.status {
                SessionStatus::Active => Theme::status_active(),
                SessionStatus::Idle => Theme::status_idle(),
                SessionStatus::Thinking => Theme::status_thinking(),
                SessionStatus::Dead => Theme::status_dead(),
            };

            let model_short = session
                .model
                .as_deref()
                .map(shorten_model)
                .unwrap_or("-".to_string());

            let marker = match (is_attached, has_bell) {
                (true, true) => ">>*",
                (true, false) => ">>",
                (false, true) => " *",
                (false, false) => "",
            };

            let marker_style = if has_bell {
                bell_style
            } else {
                Theme::attached_marker()
            };

            let name_style = if has_bell {
                bell_style
            } else {
                Style::default()
            };

            let cells = vec![
                Cell::from(marker).style(marker_style),
                Cell::from(session.project_name.clone()).style(name_style),
                Cell::from(
                    session
                        .git_branch
                        .clone()
                        .unwrap_or_else(|| "-".to_string()),
                ),
                Cell::from(model_short),
                Cell::from(session.status.label()).style(status_style),
                Cell::from(format_count(session.message_count as u64)),
                Cell::from(format_tokens(
                    session.input_tokens + session.cache_read_tokens,
                )),
                Cell::from(format_tokens(session.output_tokens)),
                Cell::from(format!("${:.2}", session.estimated_cost_usd())).style(Theme::cost()),
                Cell::from(session.last_activity_display()),
            ];
            Row::new(cells)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
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
    let total_cost: f64 = sessions.iter().map(|s| s.estimated_cost_usd()).sum();
    let total_tokens: u64 = sessions.iter().map(|s| s.total_tokens()).sum();

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
