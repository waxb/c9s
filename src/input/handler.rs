use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};

use crate::app::ViewMode;
use crate::config::Keys;

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
    CommandTab,
    ScrollUp(usize),
    ScrollDown(usize),
    ConfirmQuit,
    CancelQuit,
    ToggleLog,
    ClearLog,
    KillSession,
    ConfirmKill,
    CancelKill,
    UnfollowSession,
    ResumeSessionPicker,
    OpenSessionFiles,
    SessionFileUp,
    SessionFileDown,
    SessionFileSelect,
    SessionFileClose,
    ToggleSideTerminal,
    SideTerminalInput(Vec<u8>),
    ToggleSessionPane,
    ForkSession,
    RenameSession,
    MessageSession,
    CoordinateSession,
    OpenUnhideDialog,
    UnhideSelect,
    UnhideClose,
    NameChar(char),
    NameBackspace,
    NameSubmit,
    NameCancel,
    MessageChar(char),
    MessageBackspace,
    MessageSubmit,
    MessageCancel,
    TakeoverConfirm,
    TakeoverFork,
    TakeoverCancel,
    ToggleExpand,
    ExpandRow,
    CollapseRow,
    None,
}

pub fn handle_event(event: &Event, mode: &ViewMode, side_focused: bool, keys: &Keys) -> Action {
    match event {
        Event::Key(key) => handle_key(key, mode, side_focused, keys),
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
            | ViewMode::SessionPane
            | ViewMode::Log => Action::MoveUp,
            _ => Action::None,
        },
        MouseEventKind::ScrollDown => match mode {
            ViewMode::Terminal | ViewMode::TerminalQSwitcher => Action::ScrollDown(3),
            ViewMode::List
            | ViewMode::Filter
            | ViewMode::QSwitcher
            | ViewMode::Detail
            | ViewMode::SessionPane
            | ViewMode::Log => Action::MoveDown,
            _ => Action::None,
        },
        _ => Action::None,
    }
}

fn is_ctrl(key: &KeyEvent, c: char) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(c)
}

fn handle_key(key: &KeyEvent, mode: &ViewMode, side_focused: bool, keys: &Keys) -> Action {
    if is_ctrl(key, 't') {
        return Action::ToggleSideTerminal;
    }
    if keys.session_pane.matches(key) {
        return Action::ToggleSessionPane;
    }

    if side_focused {
        return Action::SideTerminalInput(key_event_to_bytes(key));
    }

    let in_terminal = matches!(
        mode,
        ViewMode::Terminal | ViewMode::TerminalQSwitcher | ViewMode::SessionPane
    );
    if !in_terminal {
        if is_ctrl(key, 'c') {
            return Action::Quit;
        }
        if is_ctrl(key, 'b') {
            return Action::ResumeSessionPicker;
        }
        if is_ctrl(key, ' ') {
            return Action::ToggleQSwitcher;
        }
    }

    match mode {
        ViewMode::Filter => handle_filter_key(key),
        ViewMode::QSwitcher => handle_qswitcher_key(key),
        ViewMode::Terminal => handle_terminal_key(key),
        ViewMode::TerminalQSwitcher => handle_terminal_qswitcher_key(key),
        ViewMode::Command => handle_command_key(key),
        ViewMode::ConfirmQuit => handle_confirm_quit_key(key),
        ViewMode::ConfirmKill => handle_confirm_kill_key(key),
        ViewMode::SessionFilePicker => handle_session_file_picker_key(key),
        ViewMode::Log => handle_log_key(key),
        ViewMode::SessionPane => handle_session_pane_key(key),
        ViewMode::NameInput => handle_name_input_key(key),
        ViewMode::MessageInput => handle_message_input_key(key),
        ViewMode::ConfirmTakeover => handle_takeover_key(key),
        ViewMode::UnhideDialog => handle_unhide_key(key),
        ViewMode::List | ViewMode::Detail | ViewMode::Help => handle_normal_key(key),
    }
}

fn handle_session_pane_key(key: &KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => return Action::Detach,
            KeyCode::Char(' ') => return Action::TerminalQSwitcher,
            KeyCode::Char('n') => return Action::CycleNextSession,
            KeyCode::Char('p') => return Action::CyclePrevSession,
            _ => return Action::None,
        }
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Enter => Action::Select,
        KeyCode::Char(c @ '1'..='9') => Action::AttachByIndex(digit_index(c)),
        KeyCode::Char('n') => Action::LaunchNew,
        KeyCode::Char('f') => Action::ForkSession,
        KeyCode::Char('x') => Action::KillSession,
        KeyCode::Char('u') => Action::UnfollowSession,
        KeyCode::Char('r') => Action::RenameSession,
        KeyCode::Char('m') => Action::MessageSession,
        KeyCode::Char('M') => Action::CoordinateSession,
        KeyCode::Esc | KeyCode::Char('q') => Action::Back,
        _ => Action::None,
    }
}

fn handle_name_input_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::NameCancel,
        KeyCode::Enter => Action::NameSubmit,
        KeyCode::Backspace => Action::NameBackspace,
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => Action::NameChar(c),
        _ => Action::None,
    }
}

fn handle_message_input_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::MessageCancel,
        KeyCode::Enter => Action::MessageSubmit,
        KeyCode::Backspace => Action::MessageBackspace,
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::MessageChar(c)
        }
        _ => Action::None,
    }
}

fn handle_takeover_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('t') => Action::TakeoverConfirm,
        KeyCode::Char('f') => Action::TakeoverFork,
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => Action::TakeoverCancel,
        _ => Action::None,
    }
}

fn handle_unhide_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Enter => Action::UnhideSelect,
        KeyCode::Esc | KeyCode::Char('q') => Action::UnhideClose,
        _ => Action::None,
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
        KeyCode::Char('f') => Action::OpenSessionFiles,
        KeyCode::Char('a') => Action::AttachSession,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Char('/') => Action::ToggleFilter,
        KeyCode::Char('s') => Action::CycleSort,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('n') => Action::LaunchNew,
        KeyCode::Char('u') => Action::UnfollowSession,
        KeyCode::Char('x') => Action::KillSession,
        KeyCode::Char('L') => Action::ToggleLog,
        KeyCode::Char('F') => Action::ForkSession,
        KeyCode::Char('U') => Action::OpenUnhideDialog,
        KeyCode::Char(' ') => Action::ToggleExpand,
        KeyCode::Char('l') | KeyCode::Right => Action::ExpandRow,
        KeyCode::Char('h') | KeyCode::Left => Action::CollapseRow,
        KeyCode::Char(c @ '1'..='9') => Action::AttachByIndex(digit_index(c)),
        _ => Action::None,
    }
}

fn digit_index(c: char) -> usize {
    (c as usize) - ('1' as usize)
}

fn handle_qswitcher_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char(c @ '1'..='9') => Action::AttachByIndex(digit_index(c)),
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('l') | KeyCode::Right => Action::ExpandRow,
        KeyCode::Char('h') | KeyCode::Left => Action::CollapseRow,
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
    if is_ctrl(key, 'd') {
        return Action::Detach;
    }
    match key.code {
        KeyCode::Char(c @ '1'..='9') => Action::AttachByIndex(digit_index(c)),
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('l') | KeyCode::Right => Action::ExpandRow,
        KeyCode::Char('h') | KeyCode::Left => Action::CollapseRow,
        KeyCode::Enter => Action::AttachSession,
        KeyCode::Esc | KeyCode::Char(' ') => Action::Back,
        _ => Action::None,
    }
}

fn handle_confirm_quit_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter | KeyCode::Char('y') => Action::ConfirmQuit,
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('n') => Action::CancelQuit,
        _ => Action::None,
    }
}

fn handle_session_file_picker_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::SessionFileDown,
        KeyCode::Char('k') | KeyCode::Up => Action::SessionFileUp,
        KeyCode::Enter => Action::SessionFileSelect,
        KeyCode::Esc | KeyCode::Char('q') => Action::SessionFileClose,
        _ => Action::None,
    }
}

fn handle_confirm_kill_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => Action::ConfirmKill,
        KeyCode::Char('n') | KeyCode::Esc => Action::CancelKill,
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
        KeyCode::Tab | KeyCode::BackTab => Action::CommandTab,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        key_with_mod(code, KeyModifiers::NONE)
    }

    fn key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_normal_keys() {
        assert_eq!(handle_normal_key(&key(KeyCode::Char('q'))), Action::Quit);
        assert_eq!(
            handle_normal_key(&key(KeyCode::Char('n'))),
            Action::LaunchNew
        );
        assert_eq!(
            handle_normal_key(&key(KeyCode::Char('x'))),
            Action::KillSession
        );
        assert_eq!(
            handle_normal_key(&key(KeyCode::Char('u'))),
            Action::UnfollowSession
        );
        assert_eq!(handle_normal_key(&key(KeyCode::Char('c'))), Action::None);
        assert_eq!(
            handle_normal_key(&key(KeyCode::Char(' '))),
            Action::ToggleExpand
        );
        assert_eq!(handle_normal_key(&key(KeyCode::Right)), Action::ExpandRow);
        assert_eq!(
            handle_normal_key(&key(KeyCode::Char('h'))),
            Action::CollapseRow
        );
    }

    #[test]
    fn test_ctrl_space_opens_quick_switcher_in_list_and_terminal() {
        let ev = key_with_mod(KeyCode::Char(' '), KeyModifiers::CONTROL);
        assert_eq!(
            handle_key(&ev, &ViewMode::List, false, &Keys::default()),
            Action::ToggleQSwitcher
        );
        assert_eq!(
            handle_key(&ev, &ViewMode::Terminal, false, &Keys::default()),
            Action::TerminalQSwitcher
        );
    }

    #[test]
    fn test_session_pane_keys() {
        let keys = Keys::default();
        let pane = ViewMode::SessionPane;
        assert_eq!(
            handle_key(&key(KeyCode::Char('n')), &pane, false, &keys),
            Action::LaunchNew
        );
        assert_eq!(
            handle_key(&key(KeyCode::Char('f')), &pane, false, &keys),
            Action::ForkSession
        );
        assert_eq!(
            handle_key(&key(KeyCode::Char('x')), &pane, false, &keys),
            Action::KillSession
        );
        assert_eq!(
            handle_key(&key(KeyCode::Char('r')), &pane, false, &keys),
            Action::RenameSession
        );
        assert_eq!(
            handle_key(&key(KeyCode::Char('m')), &pane, false, &keys),
            Action::MessageSession
        );
        assert_eq!(
            handle_key(&key(KeyCode::Char('M')), &pane, false, &keys),
            Action::CoordinateSession
        );
        assert_eq!(
            handle_key(&key(KeyCode::Char('3')), &pane, false, &keys),
            Action::AttachByIndex(2)
        );
        assert_eq!(
            handle_key(&key(KeyCode::Esc), &pane, false, &keys),
            Action::Back
        );
        let ctrl_c = key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(
            handle_key(&ctrl_c, &pane, false, &keys),
            Action::None,
            "no quit from pane"
        );
        let ctrl_d = key_with_mod(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(handle_key(&ctrl_d, &pane, false, &keys), Action::Detach);
    }

    #[test]
    fn test_dialog_keys() {
        let keys = Keys::default();
        assert_eq!(
            handle_key(&key(KeyCode::Char('a')), &ViewMode::NameInput, false, &keys),
            Action::NameChar('a')
        );
        assert_eq!(
            handle_key(&key(KeyCode::Enter), &ViewMode::NameInput, false, &keys),
            Action::NameSubmit
        );
        assert_eq!(
            handle_key(
                &key(KeyCode::Backspace),
                &ViewMode::MessageInput,
                false,
                &keys
            ),
            Action::MessageBackspace
        );
        assert_eq!(
            handle_key(
                &key(KeyCode::Char('t')),
                &ViewMode::ConfirmTakeover,
                false,
                &keys
            ),
            Action::TakeoverConfirm
        );
        assert_eq!(
            handle_key(
                &key(KeyCode::Char('f')),
                &ViewMode::ConfirmTakeover,
                false,
                &keys
            ),
            Action::TakeoverFork
        );
        assert_eq!(
            handle_key(&key(KeyCode::Esc), &ViewMode::ConfirmTakeover, false, &keys),
            Action::TakeoverCancel
        );
        assert_eq!(
            handle_key(&key(KeyCode::Enter), &ViewMode::UnhideDialog, false, &keys),
            Action::UnhideSelect
        );
        assert_eq!(
            handle_normal_key(&key(KeyCode::Char('U'))),
            Action::OpenUnhideDialog
        );
    }

    #[test]
    fn test_session_pane_key_from_config() {
        let ev = key_with_mod(KeyCode::Char('5'), KeyModifiers::CONTROL);
        assert_eq!(
            handle_key(&ev, &ViewMode::Terminal, false, &Keys::default()),
            Action::ToggleSessionPane
        );
        assert_eq!(
            handle_key(&ev, &ViewMode::List, true, &Keys::default()),
            Action::ToggleSessionPane
        );
    }

    #[test]
    fn test_number_keys_attach_by_index() {
        for n in 1..=9u8 {
            let c = (b'0' + n) as char;
            assert_eq!(
                handle_normal_key(&key(KeyCode::Char(c))),
                Action::AttachByIndex((n as usize) - 1)
            );
        }
    }

    #[test]
    fn test_qswitcher_routing() {
        assert_eq!(
            handle_key(
                &key(KeyCode::Char(' ')),
                &ViewMode::QSwitcher,
                false,
                &Keys::default()
            ),
            Action::Back
        );
        assert_eq!(
            handle_key(
                &key(KeyCode::Char('3')),
                &ViewMode::QSwitcher,
                false,
                &Keys::default()
            ),
            Action::AttachByIndex(2)
        );
        assert_eq!(
            handle_key(
                &key(KeyCode::Enter),
                &ViewMode::QSwitcher,
                false,
                &Keys::default()
            ),
            Action::AttachSession
        );
        assert_eq!(
            handle_key(
                &key(KeyCode::Char('l')),
                &ViewMode::QSwitcher,
                false,
                &Keys::default()
            ),
            Action::ExpandRow
        );
    }

    #[test]
    fn test_terminal_control_keys() {
        let ctrl = KeyModifiers::CONTROL;
        let keys = Keys::default();
        assert_eq!(
            handle_key(
                &key_with_mod(KeyCode::Char('d'), ctrl),
                &ViewMode::Terminal,
                false,
                &keys
            ),
            Action::Detach
        );
        assert_eq!(
            handle_key(
                &key_with_mod(KeyCode::Char('n'), ctrl),
                &ViewMode::Terminal,
                false,
                &keys
            ),
            Action::CycleNextSession
        );
        assert_eq!(
            handle_key(
                &key_with_mod(KeyCode::Char('c'), ctrl),
                &ViewMode::Terminal,
                false,
                &keys
            ),
            Action::TerminalInput(vec![3])
        );
        assert_eq!(
            handle_key(&key(KeyCode::Char('a')), &ViewMode::Terminal, false, &keys),
            Action::TerminalInput(b"a".to_vec())
        );
    }

    #[test]
    fn test_ctrl_c_quits_outside_terminal() {
        let ev = key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let keys = Keys::default();
        assert_eq!(handle_key(&ev, &ViewMode::List, false, &keys), Action::Quit);
        assert_eq!(
            handle_key(&ev, &ViewMode::Detail, false, &keys),
            Action::Quit
        );
    }

    #[test]
    fn test_side_panel_focus_routes_input() {
        let ev = key(KeyCode::Char('l'));
        let keys = Keys::default();
        assert_eq!(
            handle_key(&ev, &ViewMode::List, true, &keys),
            Action::SideTerminalInput(b"l".to_vec())
        );
        let toggle = key_with_mod(KeyCode::Char('t'), KeyModifiers::CONTROL);
        assert_eq!(
            handle_key(&toggle, &ViewMode::List, true, &keys),
            Action::ToggleSideTerminal
        );
    }

    #[test]
    fn test_key_event_to_bytes_special_keys() {
        assert_eq!(key_event_to_bytes(&key(KeyCode::Enter)), vec![b'\r']);
        assert_eq!(
            key_event_to_bytes(&key_with_mod(KeyCode::Enter, KeyModifiers::SHIFT)),
            b"\x1b[13;2u".to_vec()
        );
        assert_eq!(
            key_event_to_bytes(&key_with_mod(KeyCode::Char('x'), KeyModifiers::ALT)),
            vec![0x1b, b'x']
        );
        assert_eq!(
            key_event_to_bytes(&key(KeyCode::F(5))),
            vec![0x1b, b'[', b'1', b'5', b'~']
        );
    }

    #[test]
    fn test_command_keys() {
        assert_eq!(handle_command_key(&key(KeyCode::Tab)), Action::CommandTab);
        assert_eq!(
            handle_command_key(&key(KeyCode::Esc)),
            Action::CommandCancel
        );
        assert_eq!(
            handle_command_key(&key(KeyCode::Char('/'))),
            Action::CommandInput('/')
        );
    }
}
