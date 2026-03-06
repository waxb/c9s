use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
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

    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 20u16.min(area.height.saturating_sub(2));
    let popup_area = centered_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let inner_width = popup_width.saturating_sub(4) as usize; // account for borders + padding

    let active = state.active_field;
    let mut lines: Vec<Line> = Vec::new();

    // Prompt field
    let prompt_style = field_label_style(active == TervezoCreateField::Prompt);
    lines.push(Line::from(Span::styled(" Prompt:", prompt_style)));
    let prompt_border = if active == TervezoCreateField::Prompt {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let prompt_display = format_field_value(
        &state.prompt,
        inner_width,
        active == TervezoCreateField::Prompt,
    );
    lines.push(Line::from(vec![
        Span::styled(" ", prompt_border),
        Span::styled(prompt_display, Style::default().fg(Color::White)),
    ]));
    lines.push(Line::from(""));

    // Mode field
    let mode_style = field_label_style(active == TervezoCreateField::Mode);
    let mode_indicator = if active == TervezoCreateField::Mode {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(vec![
        Span::styled(" Mode:     ", mode_style),
        Span::styled(format!("[ {} ]", state.mode.label()), mode_indicator),
        if active == TervezoCreateField::Mode {
            Span::styled("  (Enter to toggle)", Style::default().fg(Color::DarkGray))
        } else {
            Span::raw("")
        },
    ]));
    lines.push(Line::from(""));

    // Repository field
    let repo_style = field_label_style(active == TervezoCreateField::RepoUrl);
    lines.push(Line::from(Span::styled(" Repository:", repo_style)));
    let repo_display = format_field_value(
        &state.repo_url,
        inner_width,
        active == TervezoCreateField::RepoUrl,
    );
    lines.push(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(repo_display, Style::default().fg(Color::White)),
    ]));
    lines.push(Line::from(""));

    // Base branch field
    let branch_style = field_label_style(active == TervezoCreateField::BaseBranch);
    lines.push(Line::from(Span::styled(" Base branch:", branch_style)));
    let branch_display = format_field_value(
        &state.base_branch,
        inner_width,
        active == TervezoCreateField::BaseBranch,
    );
    lines.push(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(branch_display, Style::default().fg(Color::White)),
    ]));
    lines.push(Line::from(""));

    // Error message
    if let Some(ref error) = state.error {
        lines.push(Line::from(Span::styled(
            format!(" {}", error),
            Style::default().fg(Color::Red),
        )));
    } else if state.submitting {
        lines.push(Line::from(Span::styled(
            " Submitting...",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        )));
    } else {
        lines.push(Line::from(""));
    }

    // Footer hints
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
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

    let border_style = Style::default().fg(Color::Cyan);
    let dialog = Paragraph::new(lines).block(
        Block::default()
            .title(" New Implementation ")
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    f.render_widget(dialog, popup_area);
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
    let cursor = if is_active { "█" } else { "" };
    let display = format!("{}{}", value, cursor);
    if display.len() > max_width {
        // Show the end of the string if it's too long
        let start = display.len().saturating_sub(max_width);
        format!("…{}", &display[start + 1..])
    } else {
        display
    }
}
