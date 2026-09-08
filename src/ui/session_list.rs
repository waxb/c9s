use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row as TableRow, Table, TableState};
use ratatui::Frame;

use crate::app::{App, CiStatus, Row, ViewMode};
use crate::session::{Project, Session, SessionStatus};
use crate::ui::theme::Theme;
use crate::ui::usage_panel::render_usage_panel;

const CONTEXT_YELLOW_PERCENT: u16 = 70;
const CONTEXT_RED_PERCENT: u16 = 85;
const TITLE_MAX: usize = 48;

pub fn render_session_list(f: &mut Frame, app: &App, area: Rect) {
    let show_command_bar = app.is_filtering() || app.attached_session_id().is_some();
    let command_bar_height = if show_command_bar { 1 } else { 0 };
    let usage_height = crate::ui::usage_panel::usage_panel_height(app.usage());

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
    let project_count = app
        .rows()
        .iter()
        .filter(|r| matches!(r, Row::Project(_)))
        .count();

    let title = format!(
        " c9s - {} projects, {} live / {} sessions",
        project_count,
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
            .find_session(sid)
            .map(|s| format!("{}: {}", s.project_name, session_title(s)))
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

fn marker_cell(is_attached: bool, has_bell: bool) -> Cell<'static> {
    let marker = match (is_attached, has_bell) {
        (true, true) => ">>*",
        (true, false) => ">>",
        (false, true) => " *",
        (false, false) => "",
    };
    let style = if has_bell {
        Theme::bell()
    } else {
        Theme::attached_marker()
    };
    Cell::from(marker).style(style)
}

fn project_row(app: &App, project: &Project) -> TableRow<'static> {
    let visible = app.visible_sessions(project);
    let current = app.current_visible_session(project);
    let is_attached = visible
        .iter()
        .any(|s| app.terminal_manager().is_attached(&s.id));
    let has_bell = visible.iter().any(|s| app.has_bell(&s.id));
    let marker = if app.is_expanded(project) { "v" } else { ">" };
    let label = format!(
        "{} {}  [{} live / {}]",
        marker,
        project.name,
        project.live_count(),
        visible.len()
    );
    let name_style = if has_bell {
        Theme::bell()
    } else {
        Theme::project_row()
    };
    let (branch, model, status, ctx) = match current {
        Some(s) => (
            s.git_branch.clone().unwrap_or_else(|| "-".to_string()),
            s.model
                .as_deref()
                .map(shorten_model)
                .unwrap_or_else(|| "-".to_string()),
            Cell::from(s.status.label().to_string()).style(Theme::status(&s.status)),
            Cell::from(format!("{}%", s.context_percent())).style(context_style(s)),
        ),
        None => (
            "-".to_string(),
            "-".to_string(),
            Cell::from("-"),
            Cell::from("-"),
        ),
    };
    let last = visible
        .iter()
        .max_by_key(|s| s.last_activity)
        .map(|s| s.last_activity_display())
        .unwrap_or_default();

    TableRow::new(vec![
        marker_cell(is_attached, has_bell),
        ci_cell(current.and_then(|s| app.ci_statuses.get(&s.id))),
        Cell::from(label).style(name_style),
        Cell::from(branch),
        Cell::from(model),
        status,
        ctx,
        Cell::from(format_count(project.total_messages())),
        Cell::from(format_tokens(project.total_input_tokens())),
        Cell::from(format_tokens(project.total_output_tokens())),
        Cell::from(format!("${:.2}", project.total_cost())).style(Theme::cost()),
        Cell::from(last),
    ])
}

fn session_row(app: &App, project: &Project, session: &Session) -> TableRow<'static> {
    let is_attached = app.terminal_manager().is_attached(&session.id);
    let has_bell = app.has_bell(&session.id);
    let is_current = project.current_session().map(|s| s.id.as_str()) == Some(session.id.as_str());
    let prefix = if is_current { "  * " } else { "    " };
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

    TableRow::new(vec![
        marker_cell(is_attached, has_bell),
        ci_cell(app.ci_statuses.get(&session.id)),
        Cell::from(format!("{}{}", prefix, session_title(session))).style(name_style),
        Cell::from(
            session
                .git_branch
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        ),
        Cell::from(model_short),
        Cell::from(session.status.label().to_string()).style(Theme::status(&session.status)),
        Cell::from(format!("{}%", session.context_percent())).style(context_style(session)),
        Cell::from(format_count(session.message_count as u64)),
        Cell::from(format_tokens(
            session.input_tokens + session.cache_read_tokens,
        )),
        Cell::from(format_tokens(session.output_tokens)),
        Cell::from(format!("${:.2}", session.estimated_cost_usd())).style(Theme::cost()),
        Cell::from(session.last_activity_display()),
    ])
}

fn render_table(f: &mut Frame, app: &App, area: Rect) {
    let header_cells = [
        "",
        "CI",
        "Project / Session",
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

    let header = TableRow::new(header_cells).height(1);

    let rows: Vec<TableRow> = app
        .rows()
        .iter()
        .filter_map(|row| {
            let project = app.project(row.project_index())?;
            Some(match row {
                Row::Project(_) => project_row(app, project),
                Row::Session(_, s) => session_row(app, project, project.sessions.get(*s)?),
            })
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(24),
        Constraint::Length(15),
        Constraint::Length(8),
        Constraint::Length(8),
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
    let keys = "  Enter:attach  Space:expand  n:new  x:discard  u:hide  U:unhide  d:detail  /:filter  s:sort  ?:help";

    let footer = Line::from(vec![
        Span::styled(stats, Theme::cost()),
        Span::styled(keys, Theme::footer()),
    ]);
    f.render_widget(Paragraph::new(footer), area);
}

fn non_empty(text: Option<&str>) -> Option<&str> {
    text.filter(|t| !t.trim().is_empty())
}

pub fn session_title(session: &Session) -> String {
    let custom_name = session
        .session_name_is_custom
        .then(|| non_empty(session.session_name.as_deref()))
        .flatten();
    let raw = non_empty(session.title.as_deref())
        .or(custom_name)
        .or(non_empty(session.last_prompt.as_deref()))
        .or(non_empty(session.session_name.as_deref()))
        .map(|t| t.lines().next().unwrap_or("").to_string())
        .unwrap_or_else(|| session.id[..8.min(session.id.len())].to_string());
    truncate_chars(&raw, TITLE_MAX)
}

pub(crate) fn truncate_chars(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        text.to_string()
    } else {
        let head: String = chars[..max.saturating_sub(1)].iter().collect();
        format!("{}~", head)
    }
}

pub(crate) fn context_style(session: &Session) -> Style {
    let pct = session.context_percent();
    let color = if pct >= CONTEXT_RED_PERCENT {
        Color::Red
    } else if pct >= CONTEXT_YELLOW_PERCENT {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    Style::default().fg(color)
}

pub(crate) fn shorten_model(model: &str) -> String {
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

pub(crate) fn format_count(n: u64) -> String {
    if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::test_support::make_session;

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

    #[test]
    fn test_session_title_prefers_title_then_prompt_then_id() {
        let mut s = make_session("abcdef1234", "claude-opus-5");
        assert_eq!(session_title(&s), "abcdef12");
        s.last_prompt = Some("do the thing\nsecond line".into());
        assert_eq!(session_title(&s), "do the thing");
        s.title = Some("Fix login".into());
        assert_eq!(session_title(&s), "Fix login");
        s.title = Some("x".repeat(60));
        assert_eq!(session_title(&s).chars().count(), TITLE_MAX);
    }

    #[test]
    fn test_session_title_uses_custom_name_before_prompt_and_derived_name_last() {
        let mut s = make_session("abcdef12-0000", "claude-opus-5");
        s.session_name = Some("c9s-c5".into());
        assert_eq!(session_title(&s), "c9s-c5");
        s.last_prompt = Some("fix the login bug".into());
        assert_eq!(session_title(&s), "fix the login bug");
        s.session_name = Some("brave-otter".into());
        s.session_name_is_custom = true;
        assert_eq!(session_title(&s), "brave-otter");
        s.title = Some("Login fix".into());
        assert_eq!(session_title(&s), "Login fix");
    }
}
