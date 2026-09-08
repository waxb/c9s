use ratatui::layout::{Constraint, Layout, Rect};

const TAB_BAR_ROWS: u16 = 1;
const STATUS_ROWS: u16 = 1;
const SESSION_PANE_COLS: u16 = 34;
const SESSION_PANE_MIN_COLS: u16 = 28;
const SHELL_PANE_PERCENT: u16 = 40;
const SIDE_HEADER_ROWS: u16 = 1;
const SIDE_BORDER_COLS: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Panes {
    pub session: bool,
    pub shell: bool,
}

pub struct Areas {
    pub session_pane: Option<Rect>,
    pub main: Rect,
    pub shell_pane: Option<Rect>,
}

fn session_pane_width(area: Rect) -> u16 {
    SESSION_PANE_COLS
        .min(area.width / 3)
        .max(SESSION_PANE_MIN_COLS)
        .min(area.width)
}

pub fn split_areas(area: Rect, panes: Panes) -> Areas {
    let mut constraints = Vec::new();
    if panes.session {
        constraints.push(Constraint::Length(session_pane_width(area)));
    }
    constraints.push(Constraint::Min(1));
    if panes.shell {
        constraints.push(Constraint::Percentage(SHELL_PANE_PERCENT));
    }
    let chunks = Layout::horizontal(constraints).split(area);
    let mut iter = chunks.iter().copied();
    let session_pane = panes.session.then(|| iter.next()).flatten();
    let main = iter.next().unwrap_or(area);
    let shell_pane = panes.shell.then(|| iter.next()).flatten();
    Areas {
        session_pane,
        main,
        shell_pane,
    }
}

pub fn pty_size(area: Rect, panes: Panes) -> (u16, u16) {
    let main = split_areas(area, panes).main;
    (
        main.height.saturating_sub(TAB_BAR_ROWS + STATUS_ROWS),
        main.width,
    )
}

pub fn shell_pty_size(area: Rect, panes: Panes) -> (u16, u16) {
    let shell = split_areas(
        area,
        Panes {
            shell: true,
            ..panes
        },
    )
    .shell_pane
    .unwrap_or(area);
    (
        shell.height.saturating_sub(SIDE_HEADER_ROWS),
        shell.width.saturating_sub(SIDE_BORDER_COLS),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_size_without_panes_uses_full_width() {
        let area = Rect::new(0, 0, 100, 40);
        assert_eq!(pty_size(area, Panes::default()), (38, 100));
    }

    #[test]
    fn test_shell_pane_takes_forty_percent() {
        let area = Rect::new(0, 0, 100, 40);
        let panes = Panes {
            session: false,
            shell: true,
        };
        let areas = split_areas(area, panes);
        assert_eq!(areas.main.width, 60);
        assert_eq!(areas.shell_pane.unwrap().width, 40);
        assert_eq!(pty_size(area, panes), (38, 60));
        assert_eq!(shell_pty_size(area, panes), (39, 39));
    }

    #[test]
    fn test_session_pane_sits_left_of_pty_and_shell() {
        let area = Rect::new(0, 0, 120, 40);
        let panes = Panes {
            session: true,
            shell: true,
        };
        let areas = split_areas(area, panes);
        let shell = areas.shell_pane.unwrap();
        let pane = areas.session_pane.unwrap();
        assert_eq!(pane.x, 0);
        assert_eq!(pane.width, 34);
        assert_eq!(areas.main.x, 34);
        assert_eq!(shell.x, areas.main.x + areas.main.width);
        assert_eq!(pty_size(area, panes).1, areas.main.width);
        assert_eq!(areas.main.x + areas.main.width + shell.width, 120);
    }

    #[test]
    fn test_session_pane_shrinks_on_narrow_terminals() {
        let area = Rect::new(0, 0, 60, 20);
        let panes = Panes {
            session: true,
            shell: false,
        };
        let areas = split_areas(area, panes);
        assert_eq!(areas.main.x, 28);
        assert_eq!(areas.main.width, 32);
    }
}
