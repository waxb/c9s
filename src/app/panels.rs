use std::path::{Path, PathBuf};
use std::time::Instant;

use super::support;
use super::{App, CiStatus, CI_CHECK_INTERVAL};
use crate::session::config::ConfigItem;
use crate::session::SessionStatus;
use crate::terminal::EmbeddedTerminal;

impl App {
    pub fn detail_items(&self) -> &[ConfigItem] {
        &self.detail_items
    }

    pub fn detail_cursor(&self) -> usize {
        self.detail_cursor
    }

    pub fn detail_preview(&self) -> Option<&(String, String)> {
        self.detail_preview.as_ref()
    }

    pub fn detail_preview_scroll(&self) -> usize {
        self.detail_preview_scroll
    }

    pub fn detail_open_preview(&mut self) {
        if let Some(item) = self.detail_items.get(self.detail_cursor) {
            if let Some(ref path) = item.path {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let content = std::fs::read_to_string(path)
                    .unwrap_or_else(|e| format!("(error reading file: {})", e));
                self.detail_preview = Some((name, content));
                self.detail_preview_scroll = 0;
            }
        }
    }

    pub fn detail_close_preview(&mut self) {
        self.detail_preview = None;
        self.detail_preview_scroll = 0;
    }

    pub fn log_scroll(&self) -> usize {
        self.log_scroll
    }

    pub fn log_scroll_up(&mut self) {
        self.log_scroll = self.log_scroll.saturating_sub(1);
    }

    pub fn log_scroll_down(&mut self) {
        self.log_scroll += 1;
    }

    pub fn log_scroll_to_top(&mut self) {
        self.log_scroll = 0;
    }

    pub fn log_scroll_to_bottom(&mut self) {
        let count = crate::log::entry_count();
        self.log_scroll = count.saturating_sub(1);
    }

    pub fn clear_log(&mut self) {
        crate::log::clear();
        self.log_scroll = 0;
    }

    pub fn side_terminal(&self) -> Option<&EmbeddedTerminal> {
        self.terminal_manager.active_side_terminal()
    }

    pub fn side_terminal_mut(&mut self) -> Option<&mut EmbeddedTerminal> {
        self.terminal_manager.active_side_terminal_mut()
    }

    pub fn is_side_panel_open(&self) -> bool {
        self.side_panel_open && self.terminal_manager.has_active_side_terminal()
    }

    pub fn is_side_panel_focused(&self) -> bool {
        self.side_panel_focused
    }

    pub fn open_side_panel(&mut self, rows: u16, cols: u16) {
        let cwd = self
            .terminal_manager
            .active_root()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
        if !self.terminal_manager.open_side_terminal(&cwd, rows, cols) {
            return;
        }
        self.side_panel_open = true;
        self.side_panel_focused = true;
    }

    pub fn close_side_panel(&mut self) {
        self.terminal_manager.close_side_terminal();
        self.side_panel_open = false;
        self.side_panel_focused = false;
    }

    pub fn drain_ci_statuses(&mut self) {
        while let Ok((id, status)) = self.ci_rx.try_recv() {
            self.ci_statuses.insert(id, status);
        }
    }

    pub fn check_ci_statuses(&mut self) {
        if self
            .ci_last_check
            .is_some_and(|t| t.elapsed() < CI_CHECK_INTERVAL)
        {
            return;
        }
        self.ci_last_check = Some(Instant::now());

        let targets: Vec<(String, Option<String>, PathBuf)> = self
            .filtered_sessions()
            .iter()
            .filter(|s| s.status != SessionStatus::Dead)
            .map(|s| (s.id.clone(), s.git_branch.clone(), s.cwd.clone()))
            .collect();

        for (id, branch, cwd) in targets {
            let tx = self.ci_tx.clone();
            match branch {
                None => {
                    let _ = tx.send((id, CiStatus::NoBranch));
                }
                Some(branch) => {
                    std::thread::spawn(move || {
                        let status = support::check_ci_for_local(&cwd, &branch);
                        let _ = tx.send((id, status));
                    });
                }
            }
        }
    }
}
