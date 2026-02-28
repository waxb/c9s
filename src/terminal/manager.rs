use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use super::notifier::JsonlNotifier;
use super::EmbeddedTerminal;

fn kill_process(pid: u32) {
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(500) {
        unsafe {
            if libc::kill(pid as i32, 0) != 0 {
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

pub struct TerminalManager {
    terminals: HashMap<String, EmbeddedTerminal>,
    notifiers: HashMap<String, JsonlNotifier>,
    active_id: Option<String>,
    order: Vec<String>,
    check_count: u64,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
            notifiers: HashMap::new(),
            active_id: None,
            order: Vec::new(),
            check_count: 0,
        }
    }

    pub fn attach(
        &mut self,
        session_id: &str,
        project_name: &str,
        cwd: &Path,
        existing_pid: Option<u32>,
        rows: u16,
        cols: u16,
    ) -> Result<()> {
        self.clear_active_bells();
        if !self.terminals.contains_key(session_id) {
            if let Some(pid) = existing_pid {
                kill_process(pid);
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            let term = EmbeddedTerminal::spawn_resume(session_id, project_name, cwd, rows, cols)?;
            self.order.push(session_id.to_string());
            self.terminals.insert(session_id.to_string(), term);
            self.notifiers
                .insert(session_id.to_string(), JsonlNotifier::new(cwd, session_id));
        }
        self.active_id = Some(session_id.to_string());
        self.clear_active_bells();
        Ok(())
    }

    pub fn attach_new(&mut self, cwd: &Path, rows: u16, cols: u16) -> Result<String> {
        self.clear_active_bells();
        let term = EmbeddedTerminal::spawn_new(cwd, rows, cols)?;
        let id = term.session_id().to_string();
        self.notifiers
            .insert(id.clone(), JsonlNotifier::new(cwd, &id));
        self.order.push(id.clone());
        self.terminals.insert(id.clone(), term);
        self.active_id = Some(id.clone());
        Ok(id)
    }

    pub fn attach_ssh(
        &mut self,
        impl_id: &str,
        project_name: &str,
        ssh_command: &str,
        rows: u16,
        cols: u16,
    ) -> Result<()> {
        self.clear_active_bells();
        if !self.terminals.contains_key(impl_id) {
            let term = EmbeddedTerminal::spawn_ssh(impl_id, project_name, ssh_command, rows, cols)?;
            self.order.push(impl_id.to_string());
            self.terminals.insert(impl_id.to_string(), term);
        }
        self.active_id = Some(impl_id.to_string());
        self.clear_active_bells();
        Ok(())
    }

    pub fn detach(&mut self) {
        self.active_id = None;
    }

    pub fn active_terminal(&self) -> Option<&EmbeddedTerminal> {
        self.active_id
            .as_ref()
            .and_then(|id| self.terminals.get(id))
    }

    pub fn active_terminal_mut(&mut self) -> Option<&mut EmbeddedTerminal> {
        let id = self.active_id.clone()?;
        self.terminals.get_mut(&id)
    }

    pub fn active_session_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    pub fn write_to_active(&mut self, bytes: &[u8]) -> Result<()> {
        if let Some(term) = self.active_terminal_mut() {
            term.write_input(bytes)?;
        }
        Ok(())
    }

    pub fn resize_active(&self, rows: u16, cols: u16) -> Result<()> {
        if let Some(term) = self.active_terminal() {
            term.resize(rows, cols)?;
        }
        Ok(())
    }

    pub fn cycle_next(&mut self) {
        if self.order.is_empty() {
            return;
        }
        self.clear_active_bells();
        let current_idx = self
            .active_id
            .as_ref()
            .and_then(|id| self.order.iter().position(|o| o == id))
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.order.len();
        self.active_id = Some(self.order[next_idx].clone());
        self.clear_active_bells();
    }

    pub fn cycle_prev(&mut self) {
        if self.order.is_empty() {
            return;
        }
        self.clear_active_bells();
        let current_idx = self
            .active_id
            .as_ref()
            .and_then(|id| self.order.iter().position(|o| o == id))
            .unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            self.order.len() - 1
        } else {
            current_idx - 1
        };
        self.active_id = Some(self.order[prev_idx].clone());
        self.clear_active_bells();
    }

    pub fn cleanup_inactive_exited(&mut self) {
        let active = self.active_id.clone();
        let dead: Vec<String> = self
            .terminals
            .iter()
            .filter(|(id, t)| t.is_exited() && active.as_deref() != Some(id.as_str()))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &dead {
            self.terminals.remove(id);
            self.notifiers.remove(id);
            self.order.retain(|o| o != id);
        }
    }

    pub fn remove_active(&mut self) {
        if let Some(id) = self.active_id.take() {
            self.terminals.remove(&id);
            self.notifiers.remove(&id);
            self.order.retain(|o| o != &id);
        }
    }

    pub fn active_is_exited(&self) -> bool {
        self.active_terminal().is_some_and(|t| t.is_exited())
    }

    pub fn is_attached(&self, session_id: &str) -> bool {
        self.terminals.contains_key(session_id)
    }

    pub fn has_bell_for(&self, session_id: &str) -> bool {
        self.terminals
            .get(session_id)
            .map(|t| t.has_bell())
            .unwrap_or(false)
    }

    pub fn tab_info(&self) -> Vec<TabEntry> {
        self.order
            .iter()
            .filter_map(|id| {
                let term = self.terminals.get(id)?;
                let is_active = self.active_id.as_deref() == Some(id.as_str());
                Some(TabEntry {
                    session_id: id.clone(),
                    name: term.project_name().to_string(),
                    is_active,
                    has_bell: !is_active && term.has_bell(),
                    bell_blink: !is_active && term.has_bell_blink(),
                })
            })
            .collect()
    }

    pub fn check_and_forward_notifications(&mut self, _viewing_terminal: bool) {
        self.check_count += 1;
        for (id, notifier) in &mut self.notifiers {
            if notifier.check() {
                notifier.debug_log_ext(&format!("BELL: fired for {}", &id[..8.min(id.len())]));
                if let Some(term) = self.terminals.get(id) {
                    term.set_bell();
                }
                let _ = std::io::Write::write_all(&mut std::io::stderr(), b"\x07");
                return;
            }
        }
    }

    fn clear_active_bells(&self) {
        if let Some(term) = self.active_terminal() {
            term.clear_bell();
        }
    }
}

pub struct TabEntry {
    #[allow(dead_code)]
    pub session_id: String,
    pub name: String,
    pub is_active: bool,
    pub has_bell: bool,
    pub bell_blink: bool,
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        self.terminals.clear();
    }
}
