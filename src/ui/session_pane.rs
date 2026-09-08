use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, PaneRow};
use crate::session::Session;
use crate::ui::session_list::{context_style, format_count, session_title, truncate_chars};
use crate::ui::terminal_view::WARM_MARK;
use crate::ui::theme::Theme;

const ROW_LINES: usize = 2;

fn short_age(session: &Session) -> String {
    session.last_activity_display().replace(" ago", "")
}

fn row_lines(
    app: &App,
    row: &PaneRow,
    index: usize,
    selected: bool,
    width: usize,
) -> Vec<Line<'static>> {
    let session = app.find_session(&row.session_id);
    let title = session
        .map(session_title)
        .unwrap_or_else(|| row.session_id[..8.min(row.session_id.len())].to_string());
    let marker = match (row.current, row.attached) {
        (true, _) => ">",
        (false, true) => "+",
        (false, false) => " ",
    };
    let base = if selected {
        Theme::selected()
    } else {
        Style::default()
    };
    let title_style = if app.has_bell(&row.session_id) {
        base.patch(Theme::bell())
    } else if row.current {
        base.patch(Theme::attached_marker())
    } else {
        base
    };
    let number = if index < 9 {
        format!("{}", index + 1)
    } else {
        " ".to_string()
    };
    let head = format!("{}{}", marker, number);
    let warm = app.is_warm(&row.session_id);
    let warm_mark = if warm { WARM_MARK } else { " " };
    let title_width = width.saturating_sub(head.len() + 2);
    let first = Line::from(vec![
        Span::styled(head, base.patch(Theme::help_key())),
        Span::styled(warm_mark, base.patch(Theme::warm())),
        Span::styled(
            format!(
                "{:<w$}",
                truncate_chars(&title, title_width),
                w = title_width
            ),
            title_style,
        ),
    ]);

    let second = match session {
        Some(s) => {
            let status = Theme::status(&s.status);
            let warm_note = app
                .warm_remaining(&s.id)
                .map(|left| format!(" warm {}h", left.as_secs().div_ceil(3600)))
                .unwrap_or_default();
            let detail = format!(
                " ${:.2} {}m {}{}",
                s.estimated_cost_usd(),
                format_count(s.message_count as u64),
                short_age(s),
                warm_note
            );
            let name = s
                .waiting_for
                .as_deref()
                .map(|w| format!(" needs {}", w))
                .or_else(|| {
                    s.session_name
                        .as_deref()
                        .filter(|n| *n != title)
                        .map(|n| format!(" {}", n))
                })
                .unwrap_or_default();
            let dim = if selected {
                base
            } else {
                base.patch(Theme::footer())
            };
            Line::from(vec![
                Span::styled("   ", base),
                Span::styled(format!("{:<7}", s.status.label()), base.patch(status)),
                Span::styled(
                    format!("{:>4}%", s.context_percent()),
                    base.patch(context_style(s)),
                ),
                Span::styled(detail, dim),
                Span::styled(name, dim),
            ])
        }
        None => Line::from(Span::styled("   starting", base.patch(Theme::footer()))),
    };
    vec![first, second]
}

pub fn render_session_pane(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 10 || area.height < 4 {
        return;
    }
    let focused = app.is_session_pane_focused();
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let project = app
        .terminal_manager()
        .active_root()
        .map(crate::session::project::project_name_for)
        .unwrap_or_default();
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(inner);

    let rows = app.pane_rows();
    let header = Line::from(vec![
        Span::styled(
            format!(
                "{} sessions ({})",
                if focused { ">" } else { " " },
                rows.len()
            ),
            if focused {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
        Span::styled(format!(" {}", project), Theme::footer()),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    let width = chunks[1].width as usize;
    let visible_rows = (chunks[1].height as usize / ROW_LINES).max(1);
    let selected = app.pane_cursor();
    let start = selected.saturating_sub(visible_rows.saturating_sub(1));
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(start)
        .take(visible_rows)
        .flat_map(|(i, row)| row_lines(app, row, i, focused && i == selected, width))
        .collect();
    f.render_widget(Paragraph::new(lines), chunks[1]);

    let status = app
        .pane_status()
        .map(|s| Line::from(Span::styled(s.to_string(), Theme::bell())))
        .unwrap_or_else(|| Line::from(""));
    let keys = Line::from(Span::styled(
        "n:new f:fork x:discard r:rename",
        Theme::footer(),
    ));
    let keys2 = Line::from(Span::styled(
        "m:message M:coordinate Esc:close",
        Theme::footer(),
    ));
    f.render_widget(Paragraph::new(vec![status, keys, keys2]), chunks[2]);
}
