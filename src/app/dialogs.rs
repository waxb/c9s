use std::path::Path;

use super::pane::PaneRow;
use super::{App, MessageInput, NameInput, TakeoverPrompt, UnhideDialog, ViewMode};
use crate::session::Session;

impl App {
    pub fn in_terminal_context(&self) -> bool {
        let base = if self.view_mode.is_dialog() {
            &self.dialog_return
        } else {
            &self.view_mode
        };
        matches!(
            base,
            ViewMode::Terminal | ViewMode::TerminalQSwitcher | ViewMode::SessionPane
        )
    }

    pub fn open_dialog(&mut self, mode: ViewMode) {
        if !self.view_mode.is_dialog() {
            self.dialog_return = self.view_mode.clone();
        }
        self.view_mode = mode;
    }

    pub fn close_dialog(&mut self) {
        let target = self.dialog_return.clone();
        self.set_view_mode(target);
    }

    pub fn enter_terminal(&mut self) {
        self.sync_pane_cursor();
        self.set_view_mode(ViewMode::Terminal);
    }

    pub fn is_session_pane_open(&self) -> bool {
        self.pane.open
    }

    pub fn is_session_pane_focused(&self) -> bool {
        self.view_mode == ViewMode::SessionPane
            || (self.view_mode.is_dialog() && self.dialog_return == ViewMode::SessionPane)
    }

    pub fn open_session_pane(&mut self) {
        self.pane.open = true;
        self.pane.status = None;
        self.sync_pane_cursor();
    }

    pub fn close_session_pane(&mut self) {
        self.pane.open = false;
        self.pane.status = None;
    }

    pub fn pane_cursor(&self) -> usize {
        self.pane.cursor
    }

    pub fn pane_status(&self) -> Option<&str> {
        self.pane.status.as_deref()
    }

    pub fn set_pane_status(&mut self, message: &str) {
        self.pane.status = Some(message.to_string());
    }

    pub fn sync_pane_cursor(&mut self) {
        let rows = self.pane_rows();
        if let Some(pos) = rows.iter().position(|r| r.current) {
            self.pane.cursor = pos;
        }
        self.pane.clamp(rows.len());
    }

    pub fn pane_rows(&self) -> Vec<PaneRow> {
        let Some(tab) = self.terminal_manager.active_tab() else {
            return Vec::new();
        };
        let current = tab.current_session_id().map(|s| s.to_string());
        let attached = tab.session_ids();
        let mut rows: Vec<PaneRow> = attached
            .iter()
            .map(|id| PaneRow {
                session_id: id.clone(),
                attached: true,
                current: current.as_deref() == Some(id.as_str()),
            })
            .collect();
        if let Some(project) = self.projects.iter().find(|p| p.root == tab.root()) {
            for session in self.list.visible_sessions(project) {
                if !attached.contains(&session.id) {
                    rows.push(PaneRow {
                        session_id: session.id.clone(),
                        attached: false,
                        current: false,
                    });
                }
            }
        }
        rows
    }

    pub fn pane_selected_id(&self) -> Option<String> {
        self.pane_rows()
            .get(self.pane.cursor)
            .map(|r| r.session_id.clone())
    }

    pub fn sender_name(&self) -> String {
        self.attached_session_id()
            .and_then(|id| self.find_session(id))
            .and_then(|s| s.session_name.clone())
            .or_else(|| {
                self.terminal_manager
                    .active_root()
                    .map(crate::session::project::project_name_for)
            })
            .unwrap_or_else(|| "c9s".to_string())
    }

    pub fn name_input(&self) -> Option<&NameInput> {
        self.name_input.as_ref()
    }

    pub fn name_input_mut(&mut self) -> Option<&mut NameInput> {
        self.name_input.as_mut()
    }

    pub fn open_name_input(&mut self, input: NameInput) {
        self.name_input = Some(input);
        self.open_dialog(ViewMode::NameInput);
    }

    pub fn take_name_input(&mut self) -> Option<NameInput> {
        self.name_input.take()
    }

    pub fn message_input(&self) -> Option<&MessageInput> {
        self.message_input.as_ref()
    }

    pub fn message_input_mut(&mut self) -> Option<&mut MessageInput> {
        self.message_input.as_mut()
    }

    pub fn open_message_input(&mut self, input: MessageInput) {
        self.message_input = Some(input);
        self.open_dialog(ViewMode::MessageInput);
    }

    pub fn take_message_input(&mut self) -> Option<MessageInput> {
        self.message_input.take()
    }

    pub fn takeover(&self) -> Option<&TakeoverPrompt> {
        self.takeover.as_ref()
    }

    pub fn open_takeover(&mut self, prompt: TakeoverPrompt) {
        self.takeover = Some(prompt);
        self.open_dialog(ViewMode::ConfirmTakeover);
    }

    pub fn take_takeover(&mut self) -> Option<TakeoverPrompt> {
        self.takeover.take()
    }

    pub fn unhide(&self) -> Option<&UnhideDialog> {
        self.unhide.as_ref()
    }

    pub fn open_unhide(&mut self, dialog: UnhideDialog) {
        self.unhide = Some(dialog);
        self.open_dialog(ViewMode::UnhideDialog);
    }

    pub fn unhide_selected_id(&self) -> Option<String> {
        let dialog = self.unhide.as_ref()?;
        self.hidden_sessions(&dialog.root)
            .get(dialog.cursor)
            .map(|s| s.id.clone())
    }

    pub fn hidden_sessions(&self, root: &Path) -> Vec<&Session> {
        self.projects
            .iter()
            .filter(|p| p.root == root)
            .flat_map(|p| p.sessions.iter())
            .filter(|s| !self.list.is_visible(s))
            .collect()
    }

    pub fn unhide_session(&mut self, id: &str) {
        self.list.hidden.remove(id);
        if let Some(ref store) = self.store {
            let _ = store.set_hidden(id, false);
        }
        self.rebuild_rows_keeping_selection();
    }
}
