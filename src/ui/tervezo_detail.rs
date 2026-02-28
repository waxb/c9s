use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui::Frame;

use crate::app::{TervezoDetailState, TervezoTab};
use crate::tervezo::models::{format_duration_secs, FileChange, TestReport};
use crate::tervezo::ImplementationStatus;
use crate::ui::theme::Theme;

pub fn render_tervezo_detail(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let has_steps = state
        .status_info
        .as_ref()
        .map(|s| !s.steps.is_empty())
        .unwrap_or(false);
    let header_height = if has_steps { 4 } else { 3 };
    let footer_height = if state.action_result.is_some() { 2 } else { 1 };

    let chunks = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(5),
        Constraint::Length(footer_height),
    ])
    .split(area);

    render_header(f, state, chunks[0]);
    render_body(f, state, chunks[1]);
    render_footer(f, state, chunks[2]);

    // Step detail overlay
    if state.steps_expanded {
        if let Some(ref status) = state.status_info {
            if !status.steps.is_empty() {
                render_step_overlay(f, state, area);
            }
        }
    }
}

pub fn render_tervezo_detail_with_prompt(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let has_steps = state
        .status_info
        .as_ref()
        .map(|s| !s.steps.is_empty())
        .unwrap_or(false);
    let header_height = if has_steps { 4 } else { 3 };

    let chunks = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(5),
        Constraint::Length(3),
    ])
    .split(area);

    render_header(f, state, chunks[0]);
    render_body(f, state, chunks[1]);
    render_prompt_input(f, state, chunks[2]);
}

pub fn render_tervezo_action_menu(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let item_count = state.action_menu_items.len();
    let popup_height = (item_count as u16) + 4;
    let popup_width = 30;
    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    for (i, action) in state.action_menu_items.iter().enumerate() {
        let is_selected = i == state.action_menu_cursor;
        let marker = if is_selected { " > " } else { "   " };
        let style = if is_selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::from(vec![
            Span::styled(marker.to_string(), style),
            Span::styled(action.label().to_string(), style),
        ]));
    }

    let border_style = Style::default().fg(Color::Cyan);
    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" Actions ")
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    f.render_widget(dialog, popup_area);
}

pub fn render_tervezo_confirm(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let action_label = state.confirm_action.map(|a| a.label()).unwrap_or("Action");

    let popup_area = centered_rect(45, 7, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}?", action_label),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                " y/Enter: confirm ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("   ", Style::default()),
            Span::styled(
                " n/Esc: cancel ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let border_style = Style::default().fg(Color::Yellow);
    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" Confirm ")
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    f.render_widget(dialog, popup_area);
}

fn render_header(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let status_style = match state.implementation.status {
        ImplementationStatus::Running => Theme::status_active(),
        ImplementationStatus::Pending | ImplementationStatus::Queued => Theme::status_idle(),
        ImplementationStatus::Completed | ImplementationStatus::Merged => Theme::tzv_status_done(),
        ImplementationStatus::Failed => Theme::tzv_status_failed(),
        ImplementationStatus::Stopped | ImplementationStatus::Cancelled => Theme::status_dead(),
    };

    let branch_str = state.implementation.branch.as_deref().unwrap_or("-");

    let waiting = state
        .status_info
        .as_ref()
        .map(|s| s.waiting_for_input)
        .unwrap_or(false);

    let status_label = if waiting && state.implementation.status.is_running() {
        format!("[{} - Awaiting reply]", state.implementation.status.label())
    } else {
        format!("[{}]", state.implementation.status.label())
    };

    let mut title_spans = vec![
        Span::styled(" [T] ", Theme::tzv_remote_marker()),
        Span::styled(
            state.implementation.display_name().to_string(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(status_label, status_style),
        Span::raw("  "),
        Span::styled(branch_str.to_string(), Style::default().fg(Color::DarkGray)),
    ];

    if let Some(ref pr_url) = state.implementation.pr_url {
        title_spans.push(Span::raw("  "));
        title_spans.push(Span::styled(
            pr_url.clone(),
            Style::default().fg(Color::Cyan),
        ));
    }

    let has_steps = state
        .status_info
        .as_ref()
        .map(|s| !s.steps.is_empty())
        .unwrap_or(false);

    let mut header_lines = vec![Line::from(title_spans)];

    if has_steps {
        if let Some(ref status) = state.status_info {
            header_lines.push(render_step_bar_line(&status.steps));
        }
    }

    let header = Paragraph::new(header_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .style(Theme::border()),
    );
    f.render_widget(header, area);
}

fn render_step_bar_line(steps: &[crate::tervezo::models::StatusStep]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(" "));

    for (i, step) in steps.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" → ", Style::default().fg(Color::DarkGray)));
        }

        let (icon, color) = match step.status.as_str() {
            "completed" => ("✓", Color::Green),
            "running" => ("●", Color::Cyan),
            "failed" => ("✗", Color::Red),
            "skipped" => ("○", Color::DarkGray),
            _ => ("○", Color::DarkGray), // pending
        };

        let duration_str = step
            .duration
            .map(|d| format!(" ({})", format_duration_secs(d)))
            .unwrap_or_default();

        spans.push(Span::styled(
            format!("{} {}{}", icon, step.name, duration_str),
            Style::default().fg(color),
        ));
    }

    Line::from(spans)
}

fn render_step_overlay(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let status = match state.status_info.as_ref() {
        Some(s) => s,
        None => return,
    };

    let line_count = status.steps.len() + 4;
    let popup_height = (line_count as u16).min(area.height.saturating_sub(4));
    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();

    // Overall duration
    if let Some(d) = status.duration {
        lines.push(Line::from(Span::styled(
            format!("  Total: {}", format_duration_secs(d)),
            Style::default().fg(Color::White),
        )));
    }
    lines.push(Line::from(""));

    for step in &status.steps {
        let (icon, color) = match step.status.as_str() {
            "completed" => ("✓", Color::Green),
            "running" => ("●", Color::Cyan),
            "failed" => ("✗", Color::Red),
            "skipped" => ("○", Color::DarkGray),
            _ => ("○", Color::DarkGray),
        };

        let duration_str = step
            .duration
            .map(|d| format!("  {}", format_duration_secs(d)))
            .unwrap_or_default();

        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", icon), Style::default().fg(color)),
            Span::styled(
                step.name.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(duration_str, Style::default().fg(Color::DarkGray)),
        ]));

        if let Some(ref err) = step.error {
            for err_line in err.lines().take(3) {
                lines.push(Line::from(Span::styled(
                    format!("      {}", err_line),
                    Style::default().fg(Color::Red),
                )));
            }
        }
    }

    let border_style = Style::default().fg(Color::Cyan);
    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" Steps ")
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    f.render_widget(dialog, popup_area);
}

fn render_body(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let body_chunks =
        Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)]).split(area);

    render_timeline_panel(f, state, body_chunks[0]);
    render_tab_panel(f, state, body_chunks[1]);
}

fn render_timeline_panel(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let block = Block::default()
        .title(" Timeline ")
        .title_style(Theme::title())
        .borders(Borders::ALL)
        .style(Theme::border());

    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.timeline.is_empty() {
        let loading = Paragraph::new(Line::from(Span::styled(
            "  Loading timeline...",
            Theme::tzv_loading(),
        )));
        f.render_widget(loading, inner);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    for msg in &state.timeline {
        let msg_type = msg.msg_type.as_deref().unwrap_or("");
        let eff_status = msg.effective_status();

        let (icon, icon_style, text_style) = match msg_type {
            "tool_call" => {
                let tool = msg.tool_name.as_deref().unwrap_or("tool");
                let ico = match tool {
                    "Read" => "◇",
                    "Write" | "Edit" => "◆",
                    "Bash" => "$",
                    "Grep" | "Glob" => "⌕",
                    _ => "⚙",
                };
                (
                    ico,
                    Style::default().fg(Color::Blue),
                    Style::default().fg(Color::DarkGray),
                )
            }
            "assistant_text" => (
                "▸",
                Style::default().fg(Color::Magenta),
                Theme::tzv_timeline_text(),
            ),
            "file_change" => (
                "±",
                Style::default().fg(Color::Yellow),
                Style::default().fg(Color::DarkGray),
            ),
            "thinking" | "assistant_thinking" => (
                "◎",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                Style::default().fg(Color::Cyan),
            ),
            "todo" => (
                "☐",
                Style::default().fg(Color::Yellow),
                Style::default().fg(Color::DarkGray),
            ),
            "iteration_marker" => (
                "─",
                Style::default().fg(Color::DarkGray),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ),
            "status_change" => {
                let ico = match eff_status {
                    Some("completed") | Some("success") | Some("merged") => "✓",
                    Some("running") | Some("in_progress") => "●",
                    Some("queued") | Some("pending") => "○",
                    Some("failed") | Some("error") => "✗",
                    Some("stopped") | Some("cancelled") => "■",
                    _ => "●",
                };
                let sty = match eff_status {
                    Some("completed") | Some("success") | Some("merged") => {
                        Style::default().fg(Color::Green)
                    }
                    Some("running") | Some("in_progress") => Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                    Some("queued") | Some("pending") => Style::default().fg(Color::Yellow),
                    Some("failed") | Some("error") => Style::default().fg(Color::Red),
                    Some("stopped") | Some("cancelled") => Style::default().fg(Color::DarkGray),
                    _ => Theme::tzv_timeline_icon(),
                };
                (ico, sty, Theme::tzv_timeline_text())
            }
            "pr_created" => (
                "⇡",
                Style::default().fg(Color::Cyan),
                Style::default().fg(Color::Cyan),
            ),
            "git_operation" => (
                "⎇",
                Style::default().fg(Color::Magenta),
                Style::default().fg(Color::DarkGray),
            ),
            "error" => {
                let sev = msg.severity.as_deref().unwrap_or("error");
                let color = match sev {
                    "fatal" => Color::Red,
                    "warning" => Color::Yellow,
                    _ => Color::Red,
                };
                ("✗", Style::default().fg(color), Style::default().fg(color))
            }
            "test_report" => (
                "⊘",
                Style::default().fg(Color::Green),
                Style::default().fg(Color::White),
            ),
            "tool_result" => (
                "←",
                Style::default().fg(Color::Blue),
                Style::default().fg(Color::DarkGray),
            ),
            _ => ("·", Theme::tzv_timeline_icon(), Theme::tzv_timeline_text()),
        };

        let display = msg.display_text();

        // Header line for this message
        // No `.wrap()` on the paragraph — ratatui clips at panel edge naturally.
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", icon), icon_style),
            Span::styled(display, text_style),
        ]));

        // Inline diff/content for file_change messages
        if msg.has_inline_code() {
            if let Some(ref diff) = msg.diff {
                for diff_line in diff.lines() {
                    let style = if diff_line.starts_with("+++") || diff_line.starts_with("---") {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else if diff_line.starts_with('+') {
                        Theme::tzv_diff_add()
                    } else if diff_line.starts_with('-') {
                        Theme::tzv_diff_remove()
                    } else if diff_line.starts_with("@@") {
                        Theme::tzv_diff_header()
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    lines.push(Line::from(Span::styled(
                        format!("      {}", diff_line),
                        style,
                    )));
                }
            } else if let Some(ref content) = msg.content {
                // New file: render content as additions (limit to 30 lines)
                for (i, content_line) in content.lines().enumerate() {
                    if i >= 30 {
                        lines.push(Line::from(Span::styled(
                            format!(
                                "      ... ({} more lines)",
                                content.lines().count().saturating_sub(30)
                            ),
                            Style::default().fg(Color::DarkGray),
                        )));
                        break;
                    }
                    lines.push(Line::from(Span::styled(
                        format!("      +{}", content_line),
                        Theme::tzv_diff_add(),
                    )));
                }
            }
        }
    }

    let total_lines = lines.len();
    let visible = inner.height as usize;

    // Store visible height for half-page scroll calculations
    state.timeline_visible_height.set(visible);

    let max_scroll = total_lines.saturating_sub(visible);
    let scroll = if state.timeline_at_bottom && total_lines > visible {
        max_scroll
    } else {
        state.timeline_scroll.min(max_scroll)
    };

    // Keep rendered scroll in sync so action handlers can use it
    // when transitioning from autoscroll to manual scroll.
    state.timeline_rendered_scroll.set(scroll);

    let paragraph = Paragraph::new(lines).scroll((scroll as u16, 0));
    f.render_widget(paragraph, inner);

    // Scrollbar
    if total_lines > visible {
        let mut scrollbar_state = ScrollbarState::new(max_scroll).position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray));
        f.render_stateful_widget(scrollbar, inner, &mut scrollbar_state);
    }
}

fn render_tab_panel(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let tab_chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).split(area);

    render_tab_bar(f, state, tab_chunks[0]);
    render_tab_content(f, state, tab_chunks[1]);
}

fn render_tab_bar(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let tabs: Vec<Span> = TervezoTab::all()
        .iter()
        .flat_map(|&tab| {
            let style = if tab == state.active_tab {
                Theme::tzv_tab_active()
            } else {
                Theme::tzv_tab_inactive()
            };
            let loading = state.loading.contains(&tab);
            let label = if loading {
                format!(" {}… ", tab.label())
            } else {
                format!(" {} ", tab.label())
            };
            vec![Span::styled(label, style), Span::raw(" ")]
        })
        .collect();

    let line = Line::from(tabs);
    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}

fn render_tab_content(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Theme::border());
    let inner = block.inner(area);
    f.render_widget(block, area);

    match state.active_tab {
        TervezoTab::Plan => render_plan_tab(f, state, inner),
        TervezoTab::Changes => render_changes_tab(f, state, inner),
        TervezoTab::TestOutput => render_test_tab(f, state, inner),
        TervezoTab::Analysis => render_analysis_tab(f, state, inner),
    }
}

fn render_plan_tab(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    match &state.plan_content {
        Some(content) => {
            let text = render_markdown_or_raw(content, state.raw_markdown);
            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((state.plan_scroll as u16, 0));
            f.render_widget(paragraph, area);
        }
        None => {
            if state.loading.contains(&TervezoTab::Plan) {
                render_loading(f, area);
            } else {
                render_empty(f, "No plan available", area);
            }
        }
    }
}

fn render_changes_tab(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    match &state.changes {
        Some(changes) if !changes.is_empty() => {
            let is_expanded = state
                .changes_expanded
                .contains(&state.changes_selected_file);

            if is_expanded {
                // Split: file list on top, diff below
                let file_list_height = (changes.len() as u16 + 1).min(area.height / 3);
                let chunks =
                    Layout::vertical([Constraint::Length(file_list_height), Constraint::Min(3)])
                        .split(area);

                render_file_list(f, state, changes, chunks[0]);
                render_diff_view(f, state, changes, chunks[1]);
            } else {
                // Full area for file list
                render_file_list(f, state, changes, area);
            }
        }
        Some(_) => {
            render_empty(f, "No file changes", area);
        }
        None => {
            if state.loading.contains(&TervezoTab::Changes) {
                render_loading(f, area);
            } else {
                render_empty(f, "Press Tab to load changes", area);
            }
        }
    }
}

fn render_file_list(f: &mut Frame, state: &TervezoDetailState, changes: &[FileChange], area: Rect) {
    let mut lines = Vec::new();

    for (i, change) in changes.iter().enumerate() {
        let is_selected = i == state.changes_selected_file;
        let is_expanded = state.changes_expanded.contains(&i);

        let chevron = if is_expanded { "▼" } else { "▶" };
        let status_str = change.status.as_deref().unwrap_or("modified");
        let status_color = match status_str {
            "added" => Color::Green,
            "removed" | "deleted" => Color::Red,
            "renamed" => Color::Cyan,
            _ => Color::Yellow,
        };

        let add_del = match (change.additions, change.deletions) {
            (Some(a), Some(d)) => format!(" +{} -{}", a, d),
            (Some(a), None) => format!(" +{}", a),
            (None, Some(d)) => format!(" -{}", d),
            _ => String::new(),
        };

        let row_style = if is_selected {
            Theme::selected()
        } else {
            Style::default()
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} {} ", if is_selected { ">" } else { " " }, chevron),
                row_style,
            ),
            Span::styled(
                change.display_path().to_string(),
                row_style.fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}", status_str),
                Style::default().fg(status_color),
            ),
            Span::styled(add_del, Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Scroll to keep selected file visible
    let visible = area.height as usize;
    let scroll = if state.changes_selected_file >= visible {
        state.changes_selected_file - visible + 1
    } else {
        0
    };

    let paragraph = Paragraph::new(lines).scroll((scroll as u16, 0));
    f.render_widget(paragraph, area);
}

fn render_diff_view(f: &mut Frame, state: &TervezoDetailState, changes: &[FileChange], area: Rect) {
    let selected = match changes.get(state.changes_selected_file) {
        Some(c) => c,
        None => return,
    };

    let diff_text = match &selected.diff {
        Some(d) => d.as_str(),
        None => "(no diff available)",
    };

    let title = format!(" {} ", selected.display_path());
    let block = Block::default()
        .title(title)
        .title_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .style(Theme::border());

    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines: Vec<Line> = diff_text
        .lines()
        .map(|line| {
            let style = if line.starts_with("+++") || line.starts_with("---") {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if line.starts_with('+') {
                Theme::tzv_diff_add()
            } else if line.starts_with('-') {
                Theme::tzv_diff_remove()
            } else if line.starts_with("@@") {
                Theme::tzv_diff_header()
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(format!(" {}", line), style))
        })
        .collect();

    let paragraph = Paragraph::new(lines).scroll((state.changes_diff_scroll as u16, 0));
    f.render_widget(paragraph, inner);
}

fn render_test_tab(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    match &state.test_output {
        Some(reports) if !reports.is_empty() => {
            let lines = build_test_report_lines(reports);
            let paragraph = Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((state.test_scroll as u16, 0));
            f.render_widget(paragraph, area);
        }
        Some(_) => {
            render_empty(f, "No test reports", area);
        }
        None => {
            if state.loading.contains(&TervezoTab::TestOutput) {
                render_loading(f, area);
            } else {
                render_empty(f, "Press Tab to load test output", area);
            }
        }
    }
}

fn build_test_report_lines(reports: &[TestReport]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for (i, report) in reports.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  ────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
        }

        // Summary header
        if let Some(ref summary) = report.summary {
            let status = summary.status.as_deref().unwrap_or("unknown");
            let (badge, badge_style) = match status {
                "passing" => (
                    " PASSING ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                "failing" => (
                    " FAILING ",
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
                "partial" => (
                    " PARTIAL ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                _ => (
                    " UNKNOWN ",
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
            };

            let mut header_spans = vec![
                Span::raw("  "),
                Span::styled(badge.to_string(), badge_style),
            ];

            // Stats
            if let Some(ref stats) = summary.stats {
                let mut stat_parts: Vec<String> = Vec::new();
                if let Some(n) = stats.new_tests {
                    stat_parts.push(format!("New: {}", n));
                }
                if let (Some(before), Some(after)) = (stats.total_before, stats.total_after) {
                    stat_parts.push(format!("Total: {}>{}", before, after));
                }
                if let Some(pre) = stats.pre_existing_failures {
                    if pre > 0 {
                        stat_parts.push(format!("Pre-existing: {}", pre));
                    }
                }
                if !stat_parts.is_empty() {
                    header_spans.push(Span::styled(
                        format!("  {}", stat_parts.join("  ")),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }

            lines.push(Line::from(header_spans));
            lines.push(Line::from(""));

            // Summary message
            if let Some(ref msg) = summary.message {
                let msg_style = match status {
                    "passing" => Style::default().fg(Color::Green),
                    "failing" => Style::default().fg(Color::Red),
                    "partial" => Style::default().fg(Color::Yellow),
                    _ => Style::default().fg(Color::White),
                };
                lines.push(Line::from(Span::styled(
                    "  Summary",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )));
                for l in msg.lines() {
                    lines.push(Line::from(Span::styled(format!("  {}", l), msg_style)));
                }
                lines.push(Line::from(""));
            }
        }

        // Approach
        if let Some(ref approach) = report.approach {
            lines.push(Line::from(Span::styled(
                "  Approach",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            for l in approach.lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", l),
                    Style::default().fg(Color::White),
                )));
            }
            lines.push(Line::from(""));
        }

        // Tests Added
        if !report.tests_added.is_empty() {
            lines.push(Line::from(Span::styled(
                "  Tests Added",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            for test in &report.tests_added {
                let file = test.file.as_deref().unwrap_or("(unknown)");
                let count_str = test.count.map(|c| format!(" (+{})", c)).unwrap_or_default();
                let critical_str = if test.critical_path == Some(true) {
                    " critical"
                } else {
                    ""
                };
                let critical_style = if test.critical_path == Some(true) {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                lines.push(Line::from(vec![
                    Span::styled("    * ", Style::default().fg(Color::Green)),
                    Span::styled(file.to_string(), Style::default().fg(Color::White)),
                    Span::styled(count_str, Style::default().fg(Color::DarkGray)),
                    Span::styled(critical_str.to_string(), critical_style),
                ]));
            }
            lines.push(Line::from(""));
        }

        // Uncovered Paths
        if !report.uncovered_paths.is_empty() {
            lines.push(Line::from(Span::styled(
                "  Uncovered Paths",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for path in &report.uncovered_paths {
                let name = path.name.as_deref().unwrap_or("(unnamed)");
                lines.push(Line::from(vec![
                    Span::styled("    ! ", Style::default().fg(Color::Yellow)),
                    Span::styled(name.to_string(), Style::default().fg(Color::Yellow)),
                ]));
                if let Some(ref detail) = path.detail {
                    for l in detail.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("      {}", l),
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                }
                if let Some(ref method) = path.verification_method {
                    lines.push(Line::from(Span::styled(
                        format!("      Verify: {}", method),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::ITALIC),
                    )));
                }
            }
        }
    }

    lines
}

fn render_analysis_tab(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    match &state.analysis_content {
        Some(content) => {
            let text = render_markdown_or_raw(content, state.raw_markdown);
            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((state.analysis_scroll as u16, 0));
            f.render_widget(paragraph, area);
        }
        None => {
            if state.loading.contains(&TervezoTab::Analysis) {
                render_loading(f, area);
            } else {
                render_empty(f, "Press Tab to load analysis", area);
            }
        }
    }
}

fn render_raw(content: &str) -> Text<'static> {
    let lines: Vec<Line<'static>> = content
        .lines()
        .map(|line| {
            Line::from(Span::styled(
                format!("  {}", line),
                Style::default().fg(Color::White),
            ))
        })
        .collect();
    Text::from(lines)
}

fn render_markdown_or_raw(content: &str, raw: bool) -> Text<'static> {
    if raw {
        return render_raw(content);
    }

    // tui-markdown can panic on certain markdown inputs (e.g. nested lists).
    // Catch the panic and fall back to raw text rendering.
    // Suppress panic output during catch_unwind to avoid spamming stderr.
    let content_owned = content.to_string();
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| {
        let rendered = tui_markdown::from_str(&content_owned);
        Text::from(
            rendered
                .lines
                .into_iter()
                .map(|line| {
                    Line::from(
                        line.spans
                            .into_iter()
                            .map(|span| Span::styled(span.content.into_owned(), span.style))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
        )
    });
    std::panic::set_hook(prev_hook);

    match result {
        Ok(text) => text,
        Err(_) => render_raw(content),
    }
}

fn render_loading(f: &mut Frame, area: Rect) {
    let paragraph = Paragraph::new(Line::from(Span::styled(
        "  Loading...",
        Theme::tzv_loading(),
    )));
    f.render_widget(paragraph, area);
}

fn render_empty(f: &mut Frame, msg: &str, area: Rect) {
    let paragraph = Paragraph::new(Line::from(Span::styled(
        format!("  {}", msg),
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(paragraph, area);
}

fn render_footer(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    // Flash message (action result)
    if let Some(ref result) = state.action_result {
        let (msg, style) = match result {
            Ok(msg) => (msg.as_str(), Style::default().fg(Color::Green)),
            Err(msg) => (msg.as_str(), Style::default().fg(Color::Red)),
        };
        lines.push(Line::from(Span::styled(format!(" {}", msg), style)));
    }

    let ssh_hint = if state.implementation.status.is_running() {
        if state.ssh_creds.is_some() {
            "s:ssh"
        } else {
            "s:ssh(loading)"
        }
    } else {
        ""
    };

    let tab_hint = if state.active_tab == TervezoTab::Changes && state.changes.is_some() {
        if state
            .changes_expanded
            .contains(&state.changes_selected_file)
        {
            "J/K:scroll(diff)  Enter:collapse"
        } else {
            "J/K:navigate  Enter:expand"
        }
    } else {
        "J/K:scroll(tab)"
    };

    let md_hint = if matches!(state.active_tab, TervezoTab::Plan | TervezoTab::Analysis) {
        if state.raw_markdown {
            "m:md"
        } else {
            "m:raw"
        }
    } else {
        ""
    };

    let waiting = state
        .status_info
        .as_ref()
        .map(|s| s.waiting_for_input)
        .unwrap_or(false);
    let prompt_hint = if waiting {
        "p:reply"
    } else if state.implementation.status.is_terminal() {
        "p:follow-up"
    } else {
        ""
    };

    let steps_hint = if state
        .status_info
        .as_ref()
        .map(|s| !s.steps.is_empty())
        .unwrap_or(false)
    {
        "w:steps"
    } else {
        ""
    };

    let keys = format!(
        " Esc:back  Tab/h/l:tabs  j/k:timeline  ^d/^u:page  g/G:top/btm  {}  {}  r:refresh  {}  a:actions  {}  {}",
        tab_hint, md_hint, ssh_hint, steps_hint, prompt_hint
    );

    lines.push(Line::from(Span::styled(keys, Theme::footer())));

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, area);
}

fn render_prompt_input(f: &mut Frame, state: &TervezoDetailState, area: Rect) {
    let waiting = state
        .status_info
        .as_ref()
        .map(|s| s.waiting_for_input)
        .unwrap_or(false);
    let label = if waiting { "Reply" } else { "Follow-up" };

    let sending_indicator = if state.prompt_sending {
        " (sending...)"
    } else {
        ""
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" {} ", label));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let text = format!(" {}{}", state.prompt_input, sending_indicator);
    let cursor_line = Line::from(vec![
        Span::styled(text, Style::default().fg(Color::White)),
        Span::styled("█", Style::default().fg(Color::Cyan)),
    ]);
    let paragraph = Paragraph::new(cursor_line);
    f.render_widget(paragraph, inner);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}
