use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{TervezoCreateField, TervezoCreateState};

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}

pub fn render_tervezo_create_dialog(f: &mut Frame, state: &Option<TervezoCreateState>, area: Rect) {
    let state = match state {
        Some(s) => s,
        None => return,
    };

    let popup_width = 80u16.min(area.width.saturating_sub(4));
    let popup_height = 26u16.min(area.height.saturating_sub(2));
    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let border_style = Style::default().fg(Color::Cyan);
    let outer_block = Block::default()
        .title(" New Implementation ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner_area = outer_block.inner(popup_area);
    f.render_widget(outer_block, popup_area);

    let active = state.active_field;

    let chunks = Layout::vertical([
        Constraint::Length(1), // 0: Workspace label
        Constraint::Length(1), // 1: Workspace value
        Constraint::Length(1), // 2: spacer
        Constraint::Length(1), // 3: Prompt label
        Constraint::Length(5), // 4: Prompt box
        Constraint::Length(1), // 5: spacer
        Constraint::Length(1), // 6: Mode
        Constraint::Length(1), // 7: spacer
        Constraint::Length(1), // 8: Repo label
        Constraint::Length(1), // 9: Repo value
        Constraint::Length(1), // 10: spacer
        Constraint::Length(1), // 11: Branch label
        Constraint::Length(1), // 12: Branch value
        Constraint::Length(1), // 13: spacer
        Constraint::Length(1), // 14: status
        Constraint::Length(1), // 15: footer
    ])
    .split(inner_area);

    let inner_width = inner_area.width.saturating_sub(2) as usize;

    render_workspace_field(f, state, active, chunks[0], chunks[1]);

    render_prompt_field(f, state, active, inner_area, chunks[3], chunks[4]);

    render_mode_field(f, state, active, chunks[6]);

    render_repo_field(f, state, active, inner_width, chunks[8], chunks[9]);

    render_branch_field(f, state, active, inner_width, chunks[11], chunks[12]);

    render_status(f, state, chunks[14]);

    render_footer(f, chunks[15]);
}

fn render_workspace_field(
    f: &mut Frame,
    state: &TervezoCreateState,
    active: TervezoCreateField,
    label_area: Rect,
    value_area: Rect,
) {
    let ws_style = field_label_style(active == TervezoCreateField::Workspace);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(" Workspace:", ws_style))),
        label_area,
    );

    let ws_line = if state.workspaces_loading {
        Line::from(vec![Span::styled(
            " Loading workspaces...",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        )])
    } else if let Some(ref err) = state.workspaces_error {
        Line::from(vec![Span::styled(
            format!(" Error: {}", err),
            Style::default().fg(Color::Red),
        )])
    } else if state.workspaces.is_empty() {
        Line::from(vec![Span::styled(
            " No workspaces found",
            Style::default().fg(Color::Red),
        )])
    } else {
        let ws = &state.workspaces[state.selected_workspace];
        let indicator = if active == TervezoCreateField::Workspace {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let mut spans = vec![
            Span::styled(" ", Style::default()),
            Span::styled(format!("[ {} ]", ws.name), indicator),
        ];
        if active == TervezoCreateField::Workspace && state.workspaces.len() > 1 {
            spans.push(Span::styled(
                "  (Enter to switch)",
                Style::default().fg(Color::DarkGray),
            ));
        }
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(ws_line), value_area);
}

fn render_prompt_field(
    f: &mut Frame,
    state: &TervezoCreateState,
    active: TervezoCreateField,
    inner_area: Rect,
    label_area: Rect,
    box_area: Rect,
) {
    let prompt_style = field_label_style(active == TervezoCreateField::Prompt);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(" Prompt:", prompt_style))),
        label_area,
    );

    let prompt_border_style = if active == TervezoCreateField::Prompt {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let prompt_text = if active == TervezoCreateField::Prompt {
        format!("{}\u{2588}", state.prompt)
    } else {
        state.prompt.clone()
    };
    let _ = inner_area;
    let prompt_box = Paragraph::new(prompt_text)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(prompt_border_style),
        );
    f.render_widget(prompt_box, box_area);
}

fn render_mode_field(
    f: &mut Frame,
    state: &TervezoCreateState,
    active: TervezoCreateField,
    area: Rect,
) {
    let mode_style = field_label_style(active == TervezoCreateField::Mode);
    let mode_indicator = if active == TervezoCreateField::Mode {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let mode_line = Line::from(vec![
        Span::styled(" Mode:     ", mode_style),
        Span::styled(format!("[ {} ]", state.mode.label()), mode_indicator),
        if active == TervezoCreateField::Mode {
            Span::styled("  (Enter to toggle)", Style::default().fg(Color::DarkGray))
        } else {
            Span::raw("")
        },
    ]);
    f.render_widget(Paragraph::new(mode_line), area);
}

fn render_repo_field(
    f: &mut Frame,
    state: &TervezoCreateState,
    active: TervezoCreateField,
    inner_width: usize,
    label_area: Rect,
    value_area: Rect,
) {
    let repo_style = field_label_style(active == TervezoCreateField::RepoUrl);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Repository (owner/repo):",
            repo_style,
        ))),
        label_area,
    );
    let repo_display = format_field_value(
        &state.repo_url,
        inner_width,
        active == TervezoCreateField::RepoUrl,
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(repo_display, Style::default().fg(Color::White)),
        ])),
        value_area,
    );
}

fn render_branch_field(
    f: &mut Frame,
    state: &TervezoCreateState,
    active: TervezoCreateField,
    inner_width: usize,
    label_area: Rect,
    value_area: Rect,
) {
    let branch_style = field_label_style(active == TervezoCreateField::BaseBranch);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(" Base branch:", branch_style))),
        label_area,
    );
    let branch_display = format_field_value(
        &state.base_branch,
        inner_width,
        active == TervezoCreateField::BaseBranch,
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(branch_display, Style::default().fg(Color::White)),
        ])),
        value_area,
    );
}

fn render_status(f: &mut Frame, state: &TervezoCreateState, area: Rect) {
    let status_widget = if let Some(ref error) = state.error {
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", error),
            Style::default().fg(Color::Red),
        )))
    } else if state.submitting {
        Paragraph::new(Line::from(Span::styled(
            " Submitting...",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        )))
    } else {
        Paragraph::new(Line::from(""))
    };
    f.render_widget(status_widget, area);
}

fn render_footer(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " Tab",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": next  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "Ctrl+Enter",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": submit  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "Esc",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": cancel", Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(footer, area);
}

fn field_label_style(is_active: bool) -> Style {
    if is_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn format_field_value(value: &str, max_width: usize, is_active: bool) -> String {
    let cursor = if is_active { "\u{2588}" } else { "" };
    let display = format!("{}{}", value, cursor);
    if display.len() > max_width {
        let start = display.len().saturating_sub(max_width);
        format!("\u{2026}{}", &display[start + 1..])
    } else {
        display
    }
}
