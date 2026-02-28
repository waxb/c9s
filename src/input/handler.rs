use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};

use crate::app::ViewMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    MoveUp,
    MoveDown,
    MoveToTop,
    MoveToBottom,
    Select,
    Back,
    ShowDetail,
    ShowHelp,
    ToggleFilter,
    FilterInput(char),
    FilterBackspace,
    FilterSubmit,
    CycleSort,
    AttachSession,
    AttachByIndex(usize),
    ToggleQSwitcher,
    Refresh,
    LaunchNew,
    TerminalInput(Vec<u8>),
    Detach,
    TerminalQSwitcher,
    CycleNextSession,
    CyclePrevSession,
    CommandInput(char),
    CommandBackspace,
    CommandSubmit,
    CommandCancel,
    ScrollUp(usize),
    ScrollDown(usize),
    ConfirmQuit,
    CancelQuit,
    TervezoTabNext,
    TervezoTabPrev,
    TervezoScrollUp,
    TervezoScrollDown,
    TervezoScrollHalfPageUp,
    TervezoScrollHalfPageDown,
    TervezoScrollToTop,
    TervezoScrollToBottom,
    TervezoSsh,
    TervezoRefreshDetail,
    TervezoToggleExpand,
    ToggleLog,
    ClearLog,
    None,
}

pub fn handle_event(event: &Event, mode: &ViewMode) -> Action {
    match event {
        Event::Key(key) => handle_key(key, mode),
        Event::Mouse(mouse) => handle_mouse(mouse.kind, mode),
        _ => Action::None,
    }
}

fn handle_mouse(kind: MouseEventKind, mode: &ViewMode) -> Action {
    match kind {
        MouseEventKind::ScrollUp => match mode {
            ViewMode::Terminal | ViewMode::TerminalQSwitcher => Action::ScrollUp(3),
            ViewMode::List
            | ViewMode::Filter
            | ViewMode::QSwitcher
            | ViewMode::Detail
            | ViewMode::Log => Action::MoveUp,
            _ => Action::None,
        },
        MouseEventKind::ScrollDown => match mode {
            ViewMode::Terminal | ViewMode::TerminalQSwitcher => Action::ScrollDown(3),
            ViewMode::List
            | ViewMode::Filter
            | ViewMode::QSwitcher
            | ViewMode::Detail
            | ViewMode::Log => Action::MoveDown,
            _ => Action::None,
        },
        _ => Action::None,
    }
}

fn handle_key(key: &KeyEvent, mode: &ViewMode) -> Action {
    match mode {
        ViewMode::Terminal | ViewMode::TerminalQSwitcher => {}
        _ => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Action::Quit;
            }
        }
    }

    match mode {
        ViewMode::Filter => handle_filter_key(key),
        ViewMode::QSwitcher => handle_qswitcher_key(key),
        ViewMode::Terminal => handle_terminal_key(key),
        ViewMode::TerminalQSwitcher => handle_terminal_qswitcher_key(key),
        ViewMode::Command => handle_command_key(key),
        ViewMode::ConfirmQuit => handle_confirm_quit_key(key),
        ViewMode::TervezoDetail => handle_tervezo_detail_key(key),
        ViewMode::Log => handle_log_key(key),
        _ => handle_normal_key(key),
    }
}

fn handle_normal_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('g') => Action::MoveToTop,
        KeyCode::Char('G') => Action::MoveToBottom,
        KeyCode::Enter => Action::Select,
        KeyCode::Esc => Action::Back,
        KeyCode::Char('d') => Action::ShowDetail,
        KeyCode::Char('a') => Action::AttachSession,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Char('/') => Action::ToggleFilter,
        KeyCode::Char('s') => Action::CycleSort,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('n') => Action::LaunchNew,
        KeyCode::Char('L') => Action::ToggleLog,
        KeyCode::Char(' ') => Action::ToggleQSwitcher,
        KeyCode::Char(c @ '1'..='9') => Action::AttachByIndex((c as usize) - ('1' as usize)),
        _ => Action::None,
    }
}

fn handle_qswitcher_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char(c @ '1'..='9') => Action::AttachByIndex((c as usize) - ('1' as usize)),
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Enter => Action::AttachSession,
        KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => Action::Back,
        _ => Action::None,
    }
}

fn handle_filter_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Enter => Action::FilterSubmit,
        KeyCode::Backspace => Action::FilterBackspace,
        KeyCode::Char(c) => Action::FilterInput(c),
        _ => Action::None,
    }
}

fn handle_terminal_key(key: &KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => return Action::Detach,
            KeyCode::Char(' ') => return Action::TerminalQSwitcher,
            KeyCode::Char('n') => return Action::CycleNextSession,
            KeyCode::Char('p') => return Action::CyclePrevSession,
            KeyCode::Char('k') => return Action::ScrollUp(10),
            KeyCode::Char('j') => return Action::ScrollDown(10),
            _ => {}
        }
    }

    Action::TerminalInput(key_event_to_bytes(key))
}

fn handle_terminal_qswitcher_key(key: &KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('d') {
        return Action::Detach;
    }
    match key.code {
        KeyCode::Char(c @ '1'..='9') => Action::AttachByIndex((c as usize) - ('1' as usize)),
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Enter => Action::AttachSession,
        KeyCode::Esc | KeyCode::Char(' ') => Action::Back,
        _ => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(' ') {
                Action::Back
            } else {
                Action::None
            }
        }
    }
}

fn handle_confirm_quit_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter => Action::ConfirmQuit,
        KeyCode::Esc | KeyCode::Char('q') => Action::CancelQuit,
        KeyCode::Char('y') => Action::ConfirmQuit,
        KeyCode::Char('n') => Action::CancelQuit,
        _ => Action::None,
    }
}

fn handle_tervezo_detail_key(key: &KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::Back,
        KeyCode::Tab | KeyCode::Char('l') => Action::TervezoTabNext,
        KeyCode::Char('h') => Action::TervezoTabPrev,
        // j/k = timeline scroll, J/K = tab scroll
        KeyCode::Char('j') | KeyCode::Down => Action::TervezoScrollDown,
        KeyCode::Char('k') | KeyCode::Up => Action::TervezoScrollUp,
        KeyCode::Char('J') => Action::MoveDown,
        KeyCode::Char('K') => Action::MoveUp,
        // Half-page / page scrolling for timeline
        KeyCode::Char('d') if ctrl => Action::TervezoScrollHalfPageDown,
        KeyCode::Char('u') if ctrl => Action::TervezoScrollHalfPageUp,
        KeyCode::PageDown => Action::TervezoScrollHalfPageDown,
        KeyCode::PageUp => Action::TervezoScrollHalfPageUp,
        // Top / bottom
        KeyCode::Char('g') => Action::TervezoScrollToTop,
        KeyCode::Char('G') => Action::TervezoScrollToBottom,
        KeyCode::Enter => Action::TervezoToggleExpand,
        KeyCode::Char('s') => Action::TervezoSsh,
        KeyCode::Char('r') => Action::TervezoRefreshDetail,
        _ => Action::None,
    }
}

fn handle_log_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('L') => Action::ToggleLog,
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('g') => Action::MoveToTop,
        KeyCode::Char('G') => Action::MoveToBottom,
        KeyCode::Char('c') => Action::ClearLog,
        _ => Action::None,
    }
}

fn handle_command_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::CommandCancel,
        KeyCode::Enter => Action::CommandSubmit,
        KeyCode::Backspace => Action::CommandBackspace,
        KeyCode::Char(c) => Action::CommandInput(c),
        _ => Action::None,
    }
}

fn key_event_to_bytes(key: &KeyEvent) -> Vec<u8> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    let base = match key.code {
        KeyCode::Char(c) if ctrl => {
            let byte = (c as u8).wrapping_sub(b'a').wrapping_add(1);
            vec![byte]
        }
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            s.as_bytes().to_vec()
        }
        KeyCode::Enter if shift => b"\x1b[13;2u".to_vec(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab if shift => b"\x1b[Z".to_vec(),
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Up => vec![0x1b, b'[', b'A'],
        KeyCode::Down => vec![0x1b, b'[', b'B'],
        KeyCode::Right => vec![0x1b, b'[', b'C'],
        KeyCode::Left => vec![0x1b, b'[', b'D'],
        KeyCode::Home => vec![0x1b, b'[', b'H'],
        KeyCode::End => vec![0x1b, b'[', b'F'],
        KeyCode::PageUp => vec![0x1b, b'[', b'5', b'~'],
        KeyCode::PageDown => vec![0x1b, b'[', b'6', b'~'],
        KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],
        KeyCode::Insert => vec![0x1b, b'[', b'2', b'~'],
        KeyCode::F(n) => f_key_bytes(n),
        KeyCode::Esc => vec![0x1b],
        _ => vec![],
    };

    if alt && !base.is_empty() {
        let mut result = vec![0x1b];
        result.extend_from_slice(&base);
        result
    } else {
        base
    }
}

fn f_key_bytes(n: u8) -> Vec<u8> {
    match n {
        1 => vec![0x1b, b'O', b'P'],
        2 => vec![0x1b, b'O', b'Q'],
        3 => vec![0x1b, b'O', b'R'],
        4 => vec![0x1b, b'O', b'S'],
        5 => vec![0x1b, b'[', b'1', b'5', b'~'],
        6 => vec![0x1b, b'[', b'1', b'7', b'~'],
        7 => vec![0x1b, b'[', b'1', b'8', b'~'],
        8 => vec![0x1b, b'[', b'1', b'9', b'~'],
        9 => vec![0x1b, b'[', b'2', b'0', b'~'],
        10 => vec![0x1b, b'[', b'2', b'1', b'~'],
        11 => vec![0x1b, b'[', b'2', b'3', b'~'],
        12 => vec![0x1b, b'[', b'2', b'4', b'~'],
        _ => vec![],
    }
}
