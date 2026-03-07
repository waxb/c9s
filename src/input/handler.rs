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
    CommandTab,
    ScrollUp(usize),
    ScrollDown(usize),
    ConfirmQuit,
    CancelQuit,
    KillSession,
    ConfirmKill,
    CancelKill,
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
    TervezoToggleRaw,
    TervezoToggleSteps,
    TervezoOpenActionMenu,
    TervezoActionMenuUp,
    TervezoActionMenuDown,
    TervezoActionMenuSelect,
    TervezoActionMenuClose,
    TervezoConfirmYes,
    TervezoConfirmNo,
    TervezoOpenPrompt,
    TervezoPromptChar(char),
    TervezoPromptBackspace,
    TervezoPromptSubmit,
    TervezoPromptCancel,
    NewSessionMenuUp,
    NewSessionMenuDown,
    NewSessionMenuSelect,
    NewSessionMenuClose,
    FixCi,
    TervezoCreateClose,
    TervezoCreateFieldNext,
    TervezoCreateFieldPrev,
    TervezoCreateToggleMode,
    TervezoCreateChar(char),
    TervezoCreateBackspace,
    TervezoCreateSubmit,
    ToggleLog,
    ClearLog,
    ToggleSideTerminal,
    SideTerminalInput(Vec<u8>),
    None,
}

pub fn handle_event(event: &Event, mode: &ViewMode, side_focused: bool) -> Action {
    match event {
        Event::Key(key) => handle_key(key, mode, side_focused),
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
            | ViewMode::TervezoQSwitcher
            | ViewMode::Detail
            | ViewMode::Log => Action::MoveUp,
            _ => Action::None,
        },
        MouseEventKind::ScrollDown => match mode {
            ViewMode::Terminal | ViewMode::TerminalQSwitcher => Action::ScrollDown(3),
            ViewMode::List
            | ViewMode::Filter
            | ViewMode::QSwitcher
            | ViewMode::TervezoQSwitcher
            | ViewMode::Detail
            | ViewMode::Log => Action::MoveDown,
            _ => Action::None,
        },
        _ => Action::None,
    }
}

fn handle_key(key: &KeyEvent, mode: &ViewMode, side_focused: bool) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('t') {
        return Action::ToggleSideTerminal;
    }

    if side_focused {
        return Action::SideTerminalInput(key_event_to_bytes(key));
    }

    match mode {
        ViewMode::Terminal | ViewMode::TerminalQSwitcher => {}
        _ => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Action::Quit;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('k') {
                return Action::KillSession;
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
        ViewMode::ConfirmKillSession => handle_confirm_kill_key(key),
        ViewMode::TervezoDetail => handle_tervezo_detail_key(key),
        ViewMode::TervezoQSwitcher => handle_qswitcher_key(key),
        ViewMode::TervezoActionMenu => handle_tervezo_action_menu_key(key),
        ViewMode::TervezoConfirm => handle_tervezo_confirm_key(key),
        ViewMode::TervezoPromptInput => handle_tervezo_prompt_key(key),
        ViewMode::TervezoCreateDialog => handle_tervezo_create_key(key),
        ViewMode::NewSessionMenu => handle_new_session_menu_key(key),
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
        KeyCode::Char('c') => Action::FixCi,
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

fn handle_confirm_kill_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => Action::ConfirmKill,
        KeyCode::Char('n') | KeyCode::Esc => Action::CancelKill,
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
        KeyCode::Char('m') => Action::TervezoToggleRaw,
        KeyCode::Char('w') => Action::TervezoToggleSteps,
        KeyCode::Char('a') => Action::TervezoOpenActionMenu,
        KeyCode::Char('p') => Action::TervezoOpenPrompt,
        KeyCode::Char(' ') => Action::ToggleQSwitcher,
        KeyCode::Char(c @ '1'..='9') => Action::AttachByIndex((c as usize) - ('1' as usize)),
        _ => Action::None,
    }
}

fn handle_tervezo_action_menu_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::TervezoActionMenuDown,
        KeyCode::Char('k') | KeyCode::Up => Action::TervezoActionMenuUp,
        KeyCode::Enter => Action::TervezoActionMenuSelect,
        KeyCode::Esc | KeyCode::Char('q') => Action::TervezoActionMenuClose,
        _ => Action::None,
    }
}

fn handle_tervezo_confirm_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => Action::TervezoConfirmYes,
        KeyCode::Char('n') | KeyCode::Esc => Action::TervezoConfirmNo,
        _ => Action::None,
    }
}

fn handle_tervezo_prompt_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter => Action::TervezoPromptSubmit,
        KeyCode::Esc => Action::TervezoPromptCancel,
        KeyCode::Backspace => Action::TervezoPromptBackspace,
        KeyCode::Char(c) => Action::TervezoPromptChar(c),
        _ => Action::None,
    }
}

fn handle_tervezo_create_key(key: &KeyEvent) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => Action::TervezoCreateClose,
        KeyCode::Tab => Action::TervezoCreateFieldNext,
        KeyCode::BackTab => Action::TervezoCreateFieldPrev,
        KeyCode::Enter if ctrl => Action::TervezoCreateSubmit,
        KeyCode::Enter => Action::TervezoCreateToggleMode, // handled contextually in process_action
        KeyCode::Backspace => Action::TervezoCreateBackspace,
        KeyCode::Char(c) => Action::TervezoCreateChar(c),
        _ => Action::None,
    }
}

fn handle_new_session_menu_key(key: &KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::NewSessionMenuDown,
        KeyCode::Char('k') | KeyCode::Up => Action::NewSessionMenuUp,
        KeyCode::Enter => Action::NewSessionMenuSelect,
        KeyCode::Esc | KeyCode::Char('q') => Action::NewSessionMenuClose,
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
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
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
    fn test_tervezo_detail_space_opens_qswitcher() {
        let action = handle_tervezo_detail_key(&key(KeyCode::Char(' ')));
        assert_eq!(action, Action::ToggleQSwitcher);
    }

    #[test]
    fn test_tervezo_detail_number_keys_attach_by_index() {
        for n in 1..=9u8 {
            let c = (b'0' + n) as char;
            let action = handle_tervezo_detail_key(&key(KeyCode::Char(c)));
            assert_eq!(action, Action::AttachByIndex((n as usize) - 1));
        }
    }

    #[test]
    fn test_tervezo_qswitcher_routing() {
        // Space in QSwitcher should produce Back (to dismiss)
        let action = handle_key(&key(KeyCode::Char(' ')), &ViewMode::TervezoQSwitcher, false);
        assert_eq!(action, Action::Back);

        // Number keys should produce AttachByIndex
        let action = handle_key(&key(KeyCode::Char('3')), &ViewMode::TervezoQSwitcher, false);
        assert_eq!(action, Action::AttachByIndex(2));

        // Esc should produce Back
        let action = handle_key(&key(KeyCode::Esc), &ViewMode::TervezoQSwitcher, false);
        assert_eq!(action, Action::Back);

        // Enter should produce AttachSession
        let action = handle_key(&key(KeyCode::Enter), &ViewMode::TervezoQSwitcher, false);
        assert_eq!(action, Action::AttachSession);
    }

    #[test]
    fn test_tervezo_detail_existing_keys_unchanged() {
        assert_eq!(handle_tervezo_detail_key(&key(KeyCode::Esc)), Action::Back);
        assert_eq!(
            handle_tervezo_detail_key(&key(KeyCode::Tab)),
            Action::TervezoTabNext
        );
        assert_eq!(
            handle_tervezo_detail_key(&key(KeyCode::Char('j'))),
            Action::TervezoScrollDown
        );
        assert_eq!(
            handle_tervezo_detail_key(&key(KeyCode::Char('k'))),
            Action::TervezoScrollUp
        );
        assert_eq!(
            handle_tervezo_detail_key(&key(KeyCode::Char('h'))),
            Action::TervezoTabPrev
        );
        assert_eq!(
            handle_tervezo_detail_key(&key(KeyCode::Char('q'))),
            Action::Back
        );
        assert_eq!(
            handle_tervezo_detail_key(&key(KeyCode::Char('s'))),
            Action::TervezoSsh
        );
        assert_eq!(
            handle_tervezo_detail_key(&key(KeyCode::Char('r'))),
            Action::TervezoRefreshDetail
        );
        assert_eq!(
            handle_tervezo_detail_key(&key(KeyCode::Char('a'))),
            Action::TervezoOpenActionMenu
        );
    }

    #[test]
    fn test_create_dialog_esc_closes() {
        let action = handle_tervezo_create_key(&key(KeyCode::Esc));
        assert_eq!(action, Action::TervezoCreateClose);
    }

    #[test]
    fn test_create_dialog_tab_navigates_fields() {
        assert_eq!(
            handle_tervezo_create_key(&key(KeyCode::Tab)),
            Action::TervezoCreateFieldNext
        );
        assert_eq!(
            handle_tervezo_create_key(&key(KeyCode::BackTab)),
            Action::TervezoCreateFieldPrev
        );
    }

    #[test]
    fn test_create_dialog_ctrl_enter_submits() {
        let action =
            handle_tervezo_create_key(&key_with_mod(KeyCode::Enter, KeyModifiers::CONTROL));
        assert_eq!(action, Action::TervezoCreateSubmit);
    }

    #[test]
    fn test_create_dialog_enter_toggles_mode() {
        let action = handle_tervezo_create_key(&key(KeyCode::Enter));
        assert_eq!(action, Action::TervezoCreateToggleMode);
    }

    #[test]
    fn test_create_dialog_char_input() {
        let action = handle_tervezo_create_key(&key(KeyCode::Char('a')));
        assert_eq!(action, Action::TervezoCreateChar('a'));
    }

    #[test]
    fn test_create_dialog_backspace() {
        let action = handle_tervezo_create_key(&key(KeyCode::Backspace));
        assert_eq!(action, Action::TervezoCreateBackspace);
    }

    #[test]
    fn test_normal_mode_c_triggers_fix_ci() {
        let action = handle_normal_key(&key(KeyCode::Char('c')));
        assert_eq!(action, Action::FixCi);
    }

    #[test]
    fn test_ctrl_k_in_list_mode_triggers_kill_session() {
        let action = handle_key(
            &key_with_mod(KeyCode::Char('k'), KeyModifiers::CONTROL),
            &ViewMode::List,
            false,
        );
        assert_eq!(action, Action::KillSession);
    }

    #[test]
    fn test_k_without_ctrl_still_moves_up() {
        let action = handle_normal_key(&key(KeyCode::Char('k')));
        assert_eq!(action, Action::MoveUp);
    }

    #[test]
    fn test_confirm_kill_y_confirms() {
        let action = handle_confirm_kill_key(&key(KeyCode::Char('y')));
        assert_eq!(action, Action::ConfirmKill);
    }

    #[test]
    fn test_confirm_kill_enter_confirms() {
        let action = handle_confirm_kill_key(&key(KeyCode::Enter));
        assert_eq!(action, Action::ConfirmKill);
    }

    #[test]
    fn test_confirm_kill_n_cancels() {
        let action = handle_confirm_kill_key(&key(KeyCode::Char('n')));
        assert_eq!(action, Action::CancelKill);
    }

    #[test]
    fn test_confirm_kill_esc_cancels() {
        let action = handle_confirm_kill_key(&key(KeyCode::Esc));
        assert_eq!(action, Action::CancelKill);
    }

    #[test]
    fn test_confirm_kill_mode_routing() {
        // Verify handle_key routes ConfirmKillSession to the kill confirmation handler
        let action = handle_key(
            &key(KeyCode::Char('y')),
            &ViewMode::ConfirmKillSession,
            false,
        );
        assert_eq!(action, Action::ConfirmKill);

        let action = handle_key(
            &key(KeyCode::Esc),
            &ViewMode::ConfirmKillSession,
            false,
        );
        assert_eq!(action, Action::CancelKill);
    }
}
