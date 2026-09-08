use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::terminal::TabEntry;
use crate::ui::theme::Theme;
use crate::usage::UsageData;

pub(crate) fn tab_label(index: usize, tab: &TabEntry) -> String {
    let depth = if tab.count > 1 {
        format!(" [{}/{}]", tab.current, tab.count)
    } else {
        String::new()
    };
    let marker = match (tab.waiting, tab.has_bell) {
        (true, _) => "!",
        (false, true) => "*",
        (false, false) => "",
    };
    format!("{}: {}{}{}", index + 1, tab.name, depth, marker)
}

pub struct TerminalViewData<'a> {
    pub tabs: &'a [TabEntry],
    pub exited: bool,
    pub scrolled: bool,
    pub usage: &'a UsageData,
    pub pane_key: &'a str,
}

pub fn render_terminal(f: &mut Frame, screen: &vt100::Screen, data: &TerminalViewData, area: Rect) {
    let TerminalViewData {
        tabs,
        exited,
        scrolled,
        usage,
        pane_key,
    } = *data;
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    render_tab_bar(f, tabs, chunks[0]);

    render_screen(f, screen, chunks[1]);

    let active = tabs.iter().find(|t| t.is_active);
    let project = active.map(|t| t.name.as_str()).unwrap_or("");
    let scroll_indicator = if scrolled {
        Span::styled(
            " [scroll] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("", Style::default())
    };

    let version = env!("CARGO_PKG_VERSION");

    let status_line = if exited {
        Line::from(vec![
            Span::styled(
                " [exited]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  C-d:dismiss  {}", project), Theme::footer()),
        ])
    } else {
        Line::from(vec![
            Span::styled(format!(" c9s {}", version), Theme::attached_marker()),
            scroll_indicator,
            Span::styled(
                format!(
                    "  C-d:list  C-Space:switch  C-n/p:cycle  {}:sessions  C-j/k:scroll  C-t:shell  {}",
                    pane_key, project
                ),
                Theme::footer(),
            ),
        ])
    };
    f.render_widget(Paragraph::new(status_line), chunks[2]);

    if usage.api_available {
        if let Some(pct) = usage.five_hour {
            render_usage_bar_right(f, usage, pct, chunks[2]);
        }
    }
}

pub(crate) fn render_screen(f: &mut Frame, screen: &vt100::Screen, area: Rect) {
    let buf = f.buffer_mut();
    for row in 0..area.height {
        for col in 0..area.width {
            if let Some(cell) = screen.cell(row, col) {
                let buf_cell = &mut buf[(col + area.x, row + area.y)];
                if cell.has_contents() {
                    buf_cell.set_symbol(cell.contents());
                }
                let fg = convert_color(cell.fgcolor());
                let bg = convert_color(cell.bgcolor());
                let mut modifier = Modifier::empty();
                if cell.bold() {
                    modifier |= Modifier::BOLD;
                }
                if cell.dim() {
                    modifier |= Modifier::DIM;
                }
                if cell.italic() {
                    modifier |= Modifier::ITALIC;
                }
                if cell.underline() {
                    modifier |= Modifier::UNDERLINED;
                }
                if cell.inverse() {
                    modifier |= Modifier::REVERSED;
                }

                let style = Style::reset().add_modifier(modifier);
                buf_cell.set_style(style);
                buf_cell.set_fg(fg);
                buf_cell.set_bg(bg);
            }
        }
    }

    if !screen.hide_cursor() {
        let (c_row, c_col) = screen.cursor_position();
        let r = c_row + area.y;
        let c = c_col + area.x;
        if r < area.y + area.height && c < area.x + area.width {
            let buf_cell = &mut buf[(c, r)];
            if let Some(cell) = screen.cell(c_row, c_col) {
                if cell.has_contents() {
                    buf_cell.set_style(Style::default().add_modifier(Modifier::REVERSED));
                } else {
                    buf_cell.set_symbol("\u{2588}");
                    buf_cell.set_style(Style::default().fg(Color::Gray));
                }
            }
        }
    }
}

fn render_tab_bar(f: &mut Frame, tabs: &[TabEntry], area: Rect) {
    let bg = Color::Indexed(236);
    let buf = f.buffer_mut();

    for x in area.x..area.x + area.width {
        let cell = &mut buf[(x, area.y)];
        cell.set_style(Style::default().bg(bg));
        cell.set_symbol(" ");
    }

    let mut col = area.x + 1;
    let max_col = area.x + area.width;

    for (i, tab) in tabs.iter().enumerate() {
        if i > 0 {
            let sep_style = Style::default().fg(Color::DarkGray).bg(bg);
            for ch in " | ".chars() {
                if col >= max_col {
                    break;
                }
                let cell = &mut buf[(col, area.y)];
                cell.set_style(sep_style);
                cell.set_symbol(&ch.to_string());
                col += 1;
            }
        }

        let (fg, modifier) = if tab.waiting {
            (Color::Red, Modifier::BOLD)
        } else if tab.is_active {
            (Color::Cyan, Modifier::BOLD)
        } else if tab.bell_blink {
            (Color::Yellow, Modifier::BOLD | Modifier::SLOW_BLINK)
        } else if tab.has_bell {
            (Color::Yellow, Modifier::BOLD)
        } else {
            (Color::White, Modifier::empty())
        };

        let tab_style = Style::default().fg(fg).bg(bg).add_modifier(modifier);
        let text = tab_label(i, tab);

        if tab.is_active && col < max_col {
            let cell = &mut buf[(col, area.y)];
            cell.set_style(Style::default().fg(Color::Cyan).bg(bg));
            cell.set_symbol("[");
            col += 1;
        }

        for ch in text.chars() {
            if col >= max_col {
                break;
            }
            let cell = &mut buf[(col, area.y)];
            cell.set_style(tab_style);
            cell.set_symbol(&ch.to_string());
            col += 1;
        }

        if tab.is_active && col < max_col {
            let cell = &mut buf[(col, area.y)];
            cell.set_style(Style::default().fg(Color::Cyan).bg(bg));
            cell.set_symbol("]");
            col += 1;
        }
    }
}

fn render_usage_bar_right(f: &mut Frame, usage: &UsageData, pct: u8, area: Rect) {
    let label = match &usage.plan_name {
        Some(p) => format!(" {} {}% ", p, pct),
        None => format!(" {}% ", pct),
    };
    let bar_width: usize = 20;
    let filled = (bar_width as f64 * pct as f64 / 100.0).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let total_len = bar_width + label.len();

    let color = if pct >= 80 {
        Color::Red
    } else if pct >= 50 {
        Color::Yellow
    } else {
        Color::Indexed(75)
    };

    let start_col = (area.x + area.width).saturating_sub(total_len as u16);
    let row = area.y;
    let buf = f.buffer_mut();
    let max_col = area.x + area.width;

    let mut col = start_col;
    for _ in 0..filled {
        if col >= max_col {
            break;
        }
        let cell = &mut buf[(col, row)];
        cell.set_symbol("\u{2588}");
        cell.set_style(Style::default().fg(color));
        col += 1;
    }
    for _ in 0..empty {
        if col >= max_col {
            break;
        }
        let cell = &mut buf[(col, row)];
        cell.set_symbol("\u{2591}");
        cell.set_style(Style::default().fg(Color::Indexed(238)));
        col += 1;
    }
    for ch in label.chars() {
        if col >= max_col {
            break;
        }
        let cell = &mut buf[(col, row)];
        cell.set_symbol(&ch.to_string());
        cell.set_style(Style::default().fg(Color::White));
        col += 1;
    }
}

pub(crate) fn convert_color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(current: usize, count: usize, waiting: bool, has_bell: bool) -> TabEntry {
        TabEntry {
            name: "c9s".into(),
            is_active: false,
            has_bell,
            bell_blink: false,
            waiting,
            current,
            count,
        }
    }

    #[test]
    fn test_tab_label_shows_depth_and_markers() {
        assert_eq!(tab_label(0, &tab(1, 1, false, false)), "1: c9s");
        assert_eq!(tab_label(1, &tab(2, 3, false, false)), "2: c9s [2/3]");
        assert_eq!(tab_label(0, &tab(1, 2, false, true)), "1: c9s [1/2]*");
        assert_eq!(tab_label(0, &tab(1, 2, true, true)), "1: c9s [1/2]!");
    }
}
