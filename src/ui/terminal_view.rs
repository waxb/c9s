use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::terminal::TabEntry;
use crate::ui::theme::Theme;

pub fn render_terminal(
    f: &mut Frame,
    screen: &vt100::Screen,
    tabs: &[TabEntry],
    exited: bool,
    scrolled: bool,
    area: Rect,
) {
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
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  C-d:dismiss  {}", project),
                Theme::footer(),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(format!(" c9s {}", version), Theme::attached_marker()),
            scroll_indicator,
            Span::styled(
                format!(
                    "  C-d:list  C-Space:harpoon  C-n/p:cycle  C-j/k:scroll  {}",
                    project
                ),
                Theme::footer(),
            ),
        ])
    };
    f.render_widget(Paragraph::new(status_line), chunks[2]);
}

fn render_screen(f: &mut Frame, screen: &vt100::Screen, area: Rect) {
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
                if col >= max_col { break; }
                let cell = &mut buf[(col, area.y)];
                cell.set_style(sep_style);
                cell.set_symbol(&ch.to_string());
                col += 1;
            }
        }

        let (fg, modifier) = if tab.is_active {
            (Color::Cyan, Modifier::BOLD)
        } else if tab.bell_blink {
            (Color::Yellow, Modifier::BOLD | Modifier::SLOW_BLINK)
        } else if tab.has_bell {
            (Color::Yellow, Modifier::BOLD)
        } else {
            (Color::White, Modifier::empty())
        };

        let tab_style = Style::default().fg(fg).bg(bg).add_modifier(modifier);

        let has_star = tab.has_bell && !tab.is_active;
        let text = if has_star {
            format!("{}: {}*", i + 1, tab.name)
        } else {
            format!("{}: {}", i + 1, tab.name)
        };

        for ch in text.chars() {
            if col >= max_col { break; }
            let cell = &mut buf[(col, area.y)];
            cell.set_style(tab_style);
            cell.set_symbol(&ch.to_string());
            col += 1;
        }
    }
}

fn convert_color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
