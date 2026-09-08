use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use crate::app::{App, CiStatus, ViewMode};
use crate::session::{Session, SessionStatus};
use crate::ui::theme::Theme;
use crate::ui::usage_panel::render_usage_panel;

pub fn render_session_list(f: &mut Frame, app: &App, area: Rect) {
    let show_command_bar = app.is_filtering() || app.attached_session_id().is_some();
    let command_bar_height = if show_command_bar { 1 } else { 0 };
    let usage_height = if app.usage().api_available { 12 } else { 6 };

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(command_bar_height),
        Constraint::Min(5),
        Constraint::Length(usage_height),
        Constraint::Length(1),
    ])
    .split(area);

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
        .filter(|s| s.status != SessionStatus::Dead)
        .count();

    let title = format!(
        " c9s - Claude Code Sessions [{}/{}]",
        live_count,
        sessions.len()
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
    f.render_widget(Paragraph::new(header).block(block), area);
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
            .all_sessions()
            .iter()
            .find(|s| s.id == sid)
            .map(|s| s.project_name.clone())
            .unwrap_or_else(|| sid[..8.min(sid.len())].to_string());

        spans.push(Span::styled(" >> ", Theme::attached_marker()));
        spans.push(Span::styled(
            format!("attached: {}", attached_name),
            Theme::attached_bar(),
        ));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn ci_cell(status: Option<&CiStatus>) -> Cell<'static> {
    let (symbol, style) = match status {
        Some(CiStatus::Passing) => ("v", Style::default().fg(Color::Green)),
        Some(CiStatus::Failing) => (
            "x",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Some(CiStatus::Running) => ("~", Style::default().fg(Color::Yellow)),
        _ => ("-", Style::default().fg(Color::DarkGray)),
    };
    Cell::from(symbol).style(style)
}

fn session_row<'a>(app: &App, session: &'a Session) -> Row<'a> {
    let is_attached = app.attached_session_id() == Some(session.id.as_str());
    let has_bell = app.has_bell(&session.id);

    let marker = match (is_attached, has_bell) {
        (true, true) => ">>*",
        (true, false) => ">>",
        (false, true) => " *",
        (false, false) => "",
    };
    let marker_style = if has_bell {
        Theme::bell()
    } else {
        Theme::attached_marker()
    };
    let name_style = if has_bell {
        Theme::bell()
    } else {
        Style::default()
    };

    let model_short = session
        .model
        .as_deref()
        .map(shorten_model)
        .unwrap_or_else(|| "-".to_string());

    let cells = vec![
        Cell::from(marker).style(marker_style),
        ci_cell(app.ci_statuses.get(&session.id)),
        Cell::from(project_label(session)).style(name_style),
        Cell::from(
            session
                .git_branch
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        ),
        Cell::from(model_short),
        Cell::from(session.status.label()).style(Theme::status(&session.status)),
        Cell::from(format!("{}%", session.context_percent())).style(context_style(session)),
        Cell::from(format_count(session.message_count as u64)),
        Cell::from(format_tokens(
            session.input_tokens + session.cache_read_tokens,
        )),
        Cell::from(format_tokens(session.output_tokens)),
        Cell::from(format!("${:.2}", session.estimated_cost_usd())).style(Theme::cost()),
        Cell::from(session.last_activity_display()),
    ];
    Row::new(cells)
}

fn render_table(f: &mut Frame, app: &App, area: Rect) {
    let header_cells = [
        "",
        "CI",
        "Project",
        "Branch",
        "Model",
        "Status",
        "Ctx",
        "Msgs",
        "Tokens In",
        "Tokens Out",
        "Cost",
        "Last Active",
    ]
    .iter()
    .map(|h| Cell::from(*h).style(Theme::header()));

    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = app
        .filtered_sessions()
        .into_iter()
        .map(|s| session_row(app, s))
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(20),
        Constraint::Length(15),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(4),
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

    let stats = format!(
        " c9s {} | ${:.2} | {} tokens",
        env!("CARGO_PKG_VERSION"),
        total_cost,
        format_tokens(total_tokens)
    );
    let keys =
        "  a:attach  C-b:resume  d:detail  x:kill  u:unfollow  n:new  /:filter  s:sort  ?:help";

    let footer = Line::from(vec![
        Span::styled(stats, Theme::cost()),
        Span::styled(keys, Theme::footer()),
    ]);
    f.render_widget(Paragraph::new(footer), area);
}

fn project_label(session: &Session) -> String {
    match session.title.as_deref() {
        Some(title) if !title.is_empty() => format!("{}: {}", session.project_name, title),
        _ => session.project_name.clone(),
    }
}

fn context_style(session: &Session) -> Style {
    let pct = session.context_percent();
    let color = if pct >= 80 {
        Color::Red
    } else if pct >= 50 {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    Style::default().fg(color)
}

fn shorten_model(model: &str) -> String {
    for family in ["fable", "mythos", "opus", "sonnet", "haiku"] {
        if model.contains(family) {
            return family.to_string();
        }
    }
    model.split('-').next_back().unwrap_or(model).to_string()
}

pub(crate) fn format_tokens(n: u64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shorten_model() {
        assert_eq!(shorten_model("claude-opus-4-6"), "opus");
        assert_eq!(shorten_model("claude-fable-5-1"), "fable");
        assert_eq!(shorten_model("claude-sonnet-5"), "sonnet");
        assert_eq!(shorten_model("something-else"), "else");
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(999), "999");
        assert_eq!(format_tokens(1_500), "1.5K");
        assert_eq!(format_tokens(2_000_000), "2.0M");
    }
}
