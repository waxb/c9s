use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use super::notifier::JsonlNotifier;
use super::EmbeddedTerminal;
use crate::session::registry::{process_alive, RegistryStatus};
use crate::tlog;

const TERM_GRACE: Duration = Duration::from_millis(500);
const KILL_GRACE: Duration = Duration::from_millis(200);
const NOTIFY_INTERVAL: Duration = Duration::from_millis(250);

fn wait_for_exit(pid: u32, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !process_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    !process_alive(pid)
}

fn kill_process(pid: u32) {
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    if wait_for_exit(pid, TERM_GRACE) {
        return;
    }
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
    wait_for_exit(pid, KILL_GRACE);
}

fn ring_terminal_bell() {
    let _ = std::io::Write::write_all(&mut std::io::stderr(), b"\x07");
}

fn finished_turn(previous: Option<&RegistryStatus>, current: &RegistryStatus) -> bool {
    let was_working = matches!(
        previous,
        Some(RegistryStatus::Busy) | Some(RegistryStatus::Compacting)
    );
    was_working && matches!(current, RegistryStatus::Idle | RegistryStatus::Waiting)
}

pub struct TerminalManager {
    terminals: HashMap<String, EmbeddedTerminal>,
    notifiers: HashMap<String, JsonlNotifier>,
    registry_status: HashMap<String, RegistryStatus>,
    active_id: Option<String>,
    order: Vec<String>,
    side_terminals: HashMap<String, EmbeddedTerminal>,
    last_notify_check: Option<Instant>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
            notifiers: HashMap::new(),
            registry_status: HashMap::new(),
            active_id: None,
            order: Vec::new(),
            side_terminals: HashMap::new(),
            last_notify_check: None,
        }
    }

    pub fn attach(
        &mut self,
        session_id: &str,
        project_name: &str,
        cwd: &Path,
        takeover_pid: Option<u32>,
        rows: u16,
        cols: u16,
    ) -> Result<()> {
        self.clear_active_bells();
        if !self.terminals.contains_key(session_id) {
            if let Some(pid) = takeover_pid {
                tlog!(info, "taking over session {} from pid {}", session_id, pid);
                kill_process(pid);
            }
            let term = EmbeddedTerminal::spawn_resume(session_id, project_name, cwd, rows, cols)?;
            self.register(term, Some(JsonlNotifier::new(cwd, session_id)));
        }
        self.active_id = Some(session_id.to_string());
        self.clear_active_bells();
        Ok(())
    }

    pub fn attach_new(&mut self, cwd: &Path, rows: u16, cols: u16) -> Result<String> {
        self.clear_active_bells();
        let term = EmbeddedTerminal::spawn_new(cwd, rows, cols)?;
        let id = term.session_id().to_string();
        self.register(term, Some(JsonlNotifier::new(cwd, &id)));
        self.active_id = Some(id.clone());
        Ok(id)
    }

    pub fn attach_resume_picker(&mut self, cwd: &Path, rows: u16, cols: u16) -> Result<String> {
        self.clear_active_bells();
        let term = EmbeddedTerminal::spawn_resume_picker(cwd, rows, cols)?;
        let id = term.session_id().to_string();
        self.register(term, Some(JsonlNotifier::new(cwd, &id)));
        self.active_id = Some(id.clone());
        Ok(id)
    }

    fn register(&mut self, term: EmbeddedTerminal, notifier: Option<JsonlNotifier>) {
        let id = term.session_id().to_string();
        if let Some(n) = notifier {
            self.notifiers.insert(id.clone(), n);
        }
        self.order.push(id.clone());
        self.terminals.insert(id, term);
    }

    fn unregister(&mut self, id: &str) {
        self.terminals.remove(id);
        self.notifiers.remove(id);
        self.registry_status.remove(id);
        self.order.retain(|o| o != id);
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

    fn cycle(&mut self, step: isize) {
        if self.order.is_empty() {
            return;
        }
        self.clear_active_bells();
        let len = self.order.len() as isize;
        let current = self
            .active_id
            .as_ref()
            .and_then(|id| self.order.iter().position(|o| o == id))
            .unwrap_or(0) as isize;
        let next = (current + step).rem_euclid(len) as usize;
        self.active_id = Some(self.order[next].clone());
        self.clear_active_bells();
    }

    pub fn cycle_next(&mut self) {
        self.cycle(1);
    }

    pub fn cycle_prev(&mut self) {
        self.cycle(-1);
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
            self.unregister(id);
        }
    }

    pub fn remove_active(&mut self) {
        if let Some(id) = self.active_id.take() {
            self.unregister(&id);
        }
    }

    pub fn active_is_exited(&self) -> bool {
        self.active_terminal().is_some_and(|t| t.is_exited())
    }

    pub fn live_session_names(&self) -> Vec<String> {
        self.order
            .iter()
            .filter_map(|id| self.terminals.get(id))
            .filter(|t| !t.is_exited())
            .map(|t| t.project_name().to_string())
            .collect()
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
                    name: term.project_name().to_string(),
                    is_active,
                    has_bell: !is_active && term.has_bell(),
                    bell_blink: !is_active && term.has_bell_blink(),
                })
            })
            .collect()
    }

    fn notify(&self, id: &str, viewing_active: bool) {
        let is_focused = viewing_active && self.active_id.as_deref() == Some(id);
        if !is_focused {
            if let Some(term) = self.terminals.get(id) {
                term.set_bell();
            }
        }
        ring_terminal_bell();
    }

    pub fn observe_registry(
        &mut self,
        statuses: &HashMap<String, RegistryStatus>,
        viewing_active: bool,
    ) -> bool {
        let ids: Vec<String> = self.terminals.keys().cloned().collect();
        let mut notified = false;
        for id in ids {
            match statuses.get(&id) {
                Some(current) => {
                    let previous = self.registry_status.get(&id);
                    if finished_turn(previous, current) {
                        self.notify(&id, viewing_active);
                        notified = true;
                    }
                    self.registry_status.insert(id, current.clone());
                }
                None => {
                    self.registry_status.remove(&id);
                }
            }
        }
        notified
    }

    pub fn check_and_forward_notifications(&mut self, viewing_active: bool) -> bool {
        if self
            .last_notify_check
            .is_some_and(|t| t.elapsed() < NOTIFY_INTERVAL)
        {
            return false;
        }
        self.last_notify_check = Some(Instant::now());

        let fired: Vec<String> = self
            .notifiers
            .iter_mut()
            .filter(|(id, _)| !self.registry_status.contains_key(id.as_str()))
            .filter_map(|(id, notifier)| notifier.check().then(|| id.clone()))
            .collect();
        for id in &fired {
            self.notify(id, viewing_active);
        }
        !fired.is_empty()
    }

    fn clear_active_bells(&self) {
        if let Some(term) = self.active_terminal() {
            term.clear_bell_blink();
        }
    }

    pub fn active_side_terminal(&self) -> Option<&EmbeddedTerminal> {
        let id = self.active_id.as_ref()?;
        self.side_terminals.get(id)
    }

    pub fn active_side_terminal_mut(&mut self) -> Option<&mut EmbeddedTerminal> {
        let id = self.active_id.clone()?;
        self.side_terminals.get_mut(&id)
    }

    pub fn open_side_terminal(&mut self, cwd: &Path, rows: u16, cols: u16) -> bool {
        let id = match &self.active_id {
            Some(id) => id.clone(),
            None => return false,
        };
        if let Some(existing) = self.side_terminals.get(&id) {
            if !existing.is_exited() {
                return true;
            }
            self.side_terminals.remove(&id);
        }
        match EmbeddedTerminal::spawn_shell(cwd, rows, cols) {
            Ok(term) => {
                self.side_terminals.insert(id, term);
                true
            }
            Err(e) => {
                tlog!(error, "failed to open side shell: {}", e);
                false
            }
        }
    }

    pub fn close_side_terminal(&mut self) {
        if let Some(id) = &self.active_id {
            if self.side_terminals.get(id).is_some_and(|t| t.is_exited()) {
                self.side_terminals.remove(id);
            }
        }
    }

    pub fn has_active_side_terminal(&self) -> bool {
        self.active_side_terminal().is_some()
    }
}

pub struct TabEntry {
    pub name: String,
    pub is_active: bool,
    pub has_bell: bool,
    pub bell_blink: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finished_turn_transitions() {
        assert!(finished_turn(
            Some(&RegistryStatus::Busy),
            &RegistryStatus::Idle
        ));
        assert!(finished_turn(
            Some(&RegistryStatus::Compacting),
            &RegistryStatus::Waiting
        ));
        assert!(!finished_turn(
            Some(&RegistryStatus::Idle),
            &RegistryStatus::Idle
        ));
        assert!(!finished_turn(None, &RegistryStatus::Idle));
        assert!(!finished_turn(
            Some(&RegistryStatus::Busy),
            &RegistryStatus::Busy
        ));
    }

    #[test]
    fn test_cycle_wraps_in_both_directions() {
        let mut m = TerminalManager::new();
        m.order = vec!["a".into(), "b".into(), "c".into()];
        m.active_id = Some("c".into());
        m.cycle_next();
        assert_eq!(m.active_id.as_deref(), Some("a"));
        m.cycle_prev();
        assert_eq!(m.active_id.as_deref(), Some("c"));
    }
}
