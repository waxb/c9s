use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::notifier::JsonlNotifier;
use super::EmbeddedTerminal;
use crate::session::registry::{process_alive, RegistryEntry, RegistryStatus};
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

pub fn kill_process(pid: u32) {
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

pub struct ProjectTab {
    root: PathBuf,
    name: String,
    terminals: Vec<EmbeddedTerminal>,
    current: usize,
    side_terminal: Option<EmbeddedTerminal>,
}

impl ProjectTab {
    fn new(root: &Path, name: &str) -> Self {
        Self {
            root: root.to_path_buf(),
            name: name.to_string(),
            terminals: Vec::new(),
            current: 0,
            side_terminal: None,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn current_terminal(&self) -> Option<&EmbeddedTerminal> {
        self.terminals.get(self.current)
    }

    fn current_terminal_mut(&mut self) -> Option<&mut EmbeddedTerminal> {
        self.terminals.get_mut(self.current)
    }

    pub fn session_ids(&self) -> Vec<String> {
        self.terminals
            .iter()
            .map(|t| t.session_id().to_string())
            .collect()
    }

    pub fn current_session_id(&self) -> Option<&str> {
        self.current_terminal().map(|t| t.session_id())
    }

    fn position(&self, session_id: &str) -> Option<usize> {
        self.terminals
            .iter()
            .position(|t| t.session_id() == session_id)
    }

    fn push(&mut self, term: EmbeddedTerminal) {
        self.terminals.push(term);
        self.current = self.terminals.len() - 1;
    }

    fn remove(&mut self, index: usize) -> EmbeddedTerminal {
        let term = self.terminals.remove(index);
        if self.current >= self.terminals.len() {
            self.current = self.terminals.len().saturating_sub(1);
        } else if index < self.current {
            self.current -= 1;
        }
        term
    }
}

pub struct TabEntry {
    pub name: String,
    pub is_active: bool,
    pub has_bell: bool,
    pub bell_blink: bool,
    pub waiting: bool,
    pub current: usize,
    pub count: usize,
}

pub struct TerminalManager {
    tabs: Vec<ProjectTab>,
    active: Option<usize>,
    notifiers: HashMap<String, JsonlNotifier>,
    registry_status: HashMap<String, RegistryStatus>,
    last_notify_check: Option<Instant>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            notifiers: HashMap::new(),
            registry_status: HashMap::new(),
            last_notify_check: None,
        }
    }

    fn tab_index_for_root(&self, root: &Path) -> Option<usize> {
        self.tabs.iter().position(|t| t.root == root)
    }

    fn ensure_tab(&mut self, root: &Path, name: &str) -> usize {
        match self.tab_index_for_root(root) {
            Some(i) => i,
            None => {
                self.tabs.push(ProjectTab::new(root, name));
                self.tabs.len() - 1
            }
        }
    }

    fn locate(&self, session_id: &str) -> Option<(usize, usize)> {
        self.tabs
            .iter()
            .enumerate()
            .find_map(|(ti, tab)| tab.position(session_id).map(|si| (ti, si)))
    }

    fn register(&mut self, root: &Path, name: &str, term: EmbeddedTerminal) {
        let id = term.session_id().to_string();
        self.notifiers
            .insert(id.clone(), JsonlNotifier::new(root, &id));
        let ti = self.ensure_tab(root, name);
        self.tabs[ti].push(term);
        self.active = Some(ti);
        self.clear_active_bells();
    }

    pub fn attach(
        &mut self,
        session_id: &str,
        project_name: &str,
        root: &Path,
        takeover_pid: Option<u32>,
        rows: u16,
        cols: u16,
    ) -> Result<()> {
        self.clear_active_bells();
        if self.switch_to(session_id) {
            return Ok(());
        }
        if let Some(pid) = takeover_pid {
            tlog!(info, "taking over session {} from pid {}", session_id, pid);
            kill_process(pid);
        }
        let term = EmbeddedTerminal::spawn_resume(session_id, root, rows, cols)?;
        self.register(root, project_name, term);
        Ok(())
    }

    pub fn attach_new(&mut self, root: &Path, name: &str, rows: u16, cols: u16) -> Result<String> {
        let project_name = super::project_name_for(root);
        let term = EmbeddedTerminal::spawn_new(root, name, rows, cols)?;
        let id = term.session_id().to_string();
        self.register(root, &project_name, term);
        Ok(id)
    }

    pub fn attach_fork(
        &mut self,
        root: &Path,
        source_id: &str,
        name: &str,
        rows: u16,
        cols: u16,
    ) -> Result<String> {
        let project_name = super::project_name_for(root);
        let term = EmbeddedTerminal::spawn_fork(root, source_id, name, rows, cols)?;
        let id = term.session_id().to_string();
        self.register(root, &project_name, term);
        Ok(id)
    }

    pub fn attach_resume_picker(&mut self, root: &Path, rows: u16, cols: u16) -> Result<String> {
        let project_name = super::project_name_for(root);
        let term = EmbeddedTerminal::spawn_resume_picker(root, rows, cols)?;
        let id = term.session_id().to_string();
        self.register(root, &project_name, term);
        Ok(id)
    }

    pub fn switch_to(&mut self, session_id: &str) -> bool {
        let Some((ti, si)) = self.locate(session_id) else {
            return false;
        };
        self.active = Some(ti);
        self.tabs[ti].current = si;
        self.clear_active_bells();
        if let Some(term) = self.tabs[ti].current_terminal() {
            term.clear_bell();
        }
        true
    }

    pub fn detach(&mut self) {
        self.active = None;
    }

    pub fn active_tab(&self) -> Option<&ProjectTab> {
        self.active.and_then(|i| self.tabs.get(i))
    }

    fn active_tab_mut(&mut self) -> Option<&mut ProjectTab> {
        self.active.and_then(|i| self.tabs.get_mut(i))
    }

    pub fn active_terminal(&self) -> Option<&EmbeddedTerminal> {
        self.active_tab().and_then(|t| t.current_terminal())
    }

    pub fn active_terminal_mut(&mut self) -> Option<&mut EmbeddedTerminal> {
        self.active_tab_mut().and_then(|t| t.current_terminal_mut())
    }

    pub fn active_session_id(&self) -> Option<&str> {
        self.active_tab().and_then(|t| t.current_session_id())
    }

    pub fn active_root(&self) -> Option<&Path> {
        self.active_tab().map(|t| t.root())
    }

    pub fn terminal(&self, session_id: &str) -> Option<&EmbeddedTerminal> {
        self.locate(session_id)
            .map(|(ti, si)| &self.tabs[ti].terminals[si])
    }

    pub fn write_to_active(&mut self, bytes: &[u8]) -> Result<()> {
        if let Some(term) = self.active_terminal_mut() {
            term.write_input(bytes)?;
        }
        Ok(())
    }

    pub fn write_to_session(&mut self, session_id: &str, bytes: &[u8]) -> Result<bool> {
        let Some((ti, si)) = self.locate(session_id) else {
            return Ok(false);
        };
        self.tabs[ti].terminals[si].write_input(bytes)?;
        Ok(true)
    }

    pub fn resize_active(&self, rows: u16, cols: u16) -> Result<()> {
        if let Some(term) = self.active_terminal() {
            term.resize(rows, cols)?;
        }
        Ok(())
    }

    fn cycle(&mut self, step: isize) {
        if self.tabs.is_empty() {
            return;
        }
        self.clear_active_bells();
        let len = self.tabs.len() as isize;
        let current = self.active.unwrap_or(0) as isize;
        self.active = Some((current + step).rem_euclid(len) as usize);
        self.clear_active_bells();
    }

    pub fn cycle_next(&mut self) {
        self.cycle(1);
    }

    pub fn cycle_prev(&mut self) {
        self.cycle(-1);
    }

    fn unregister(&mut self, ti: usize, si: usize) -> EmbeddedTerminal {
        let term = self.tabs[ti].remove(si);
        let id = term.session_id().to_string();
        self.notifiers.remove(&id);
        self.registry_status.remove(&id);
        if self.tabs[ti].terminals.is_empty() {
            self.tabs.remove(ti);
            self.active = match self.active {
                Some(a) if a == ti => None,
                Some(a) if a > ti => Some(a - 1),
                other => other,
            };
        }
        term
    }

    pub fn cleanup_inactive_exited(&mut self) {
        let focused = self.active_session_id().map(|s| s.to_string());
        let dead: Vec<String> = self
            .tabs
            .iter()
            .flat_map(|t| t.terminals.iter())
            .filter(|t| t.is_exited() && focused.as_deref() != Some(t.session_id()))
            .map(|t| t.session_id().to_string())
            .collect();
        for id in dead {
            if let Some((ti, si)) = self.locate(&id) {
                self.unregister(ti, si);
            }
        }
    }

    pub fn remove_active(&mut self) {
        if let Some(id) = self.active_session_id().map(|s| s.to_string()) {
            if let Some((ti, si)) = self.locate(&id) {
                self.unregister(ti, si);
            }
        }
    }

    pub fn discard(&mut self, session_id: &str) -> bool {
        let Some((ti, si)) = self.locate(session_id) else {
            return false;
        };
        let term = self.unregister(ti, si);
        term.terminate();
        true
    }

    pub fn active_is_exited(&self) -> bool {
        self.active_terminal().is_some_and(|t| t.is_exited())
    }

    pub fn live_session_ids(&self) -> Vec<(String, String)> {
        self.tabs
            .iter()
            .flat_map(|tab| {
                tab.terminals
                    .iter()
                    .filter(|t| !t.is_exited())
                    .map(move |t| (tab.name.clone(), t.session_id().to_string()))
            })
            .collect()
    }

    pub fn is_attached(&self, session_id: &str) -> bool {
        self.locate(session_id).is_some()
    }

    pub fn has_bell_for(&self, session_id: &str) -> bool {
        self.terminal(session_id).is_some_and(|t| t.has_bell())
    }

    pub fn is_waiting(&self, session_id: &str) -> bool {
        self.registry_status.get(session_id) == Some(&RegistryStatus::Waiting)
    }

    pub fn tab_info(&self) -> Vec<TabEntry> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(ti, tab)| {
                let is_active = self.active == Some(ti);
                let others = tab
                    .terminals
                    .iter()
                    .enumerate()
                    .filter(|(si, _)| !(is_active && *si == tab.current))
                    .map(|(_, t)| t);
                let mut has_bell = false;
                let mut bell_blink = false;
                let mut waiting = false;
                for t in others {
                    has_bell |= t.has_bell();
                    bell_blink |= t.has_bell_blink();
                    waiting |= self.is_waiting(t.session_id());
                }
                TabEntry {
                    name: tab.name.clone(),
                    is_active,
                    has_bell,
                    bell_blink,
                    waiting,
                    current: tab.current + 1,
                    count: tab.terminals.len(),
                }
            })
            .collect()
    }

    fn notify(&self, id: &str, viewing_active: bool) {
        let is_focused = viewing_active && self.active_session_id() == Some(id);
        if !is_focused {
            if let Some(term) = self.terminal(id) {
                term.set_bell();
            }
        }
        ring_terminal_bell();
    }

    pub fn reconcile_ids(&mut self, entries: &[RegistryEntry]) {
        let by_pid: HashMap<u32, &RegistryEntry> = entries.iter().map(|e| (e.pid, e)).collect();
        let mut renames: Vec<(String, String)> = Vec::new();
        for tab in &self.tabs {
            for term in &tab.terminals {
                if entries.iter().any(|e| e.session_id == term.session_id()) {
                    continue;
                }
                if let Some(entry) = term.pid().and_then(|pid| by_pid.get(&pid)) {
                    renames.push((term.session_id().to_string(), entry.session_id.clone()));
                }
            }
        }
        for (old, new) in renames {
            if let Some((ti, si)) = self.locate(&old) {
                let root = self.tabs[ti].root.clone();
                self.tabs[ti].terminals[si].set_session_id(&new);
                self.notifiers.remove(&old);
                self.notifiers
                    .insert(new.clone(), JsonlNotifier::new(&root, &new));
                tlog!(info, "session {} identified as {}", old, new);
            }
        }
    }

    pub fn observe_registry(
        &mut self,
        statuses: &HashMap<String, RegistryStatus>,
        viewing_active: bool,
    ) -> bool {
        let ids: Vec<String> = self.tabs.iter().flat_map(|t| t.session_ids()).collect();
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
        self.active_tab().and_then(|t| t.side_terminal.as_ref())
    }

    pub fn active_side_terminal_mut(&mut self) -> Option<&mut EmbeddedTerminal> {
        self.active_tab_mut().and_then(|t| t.side_terminal.as_mut())
    }

    pub fn open_side_terminal(&mut self, cwd: &Path, rows: u16, cols: u16) -> bool {
        let Some(tab) = self.active_tab_mut() else {
            return false;
        };
        if tab
            .side_terminal
            .as_ref()
            .is_some_and(|existing| !existing.is_exited())
        {
            return true;
        }
        match EmbeddedTerminal::spawn_shell(cwd, rows, cols) {
            Ok(term) => {
                tab.side_terminal = Some(term);
                true
            }
            Err(e) => {
                tlog!(error, "failed to open side shell: {}", e);
                false
            }
        }
    }

    pub fn close_side_terminal(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            if tab.side_terminal.as_ref().is_some_and(|t| t.is_exited()) {
                tab.side_terminal = None;
            }
        }
    }

    pub fn has_active_side_terminal(&self) -> bool {
        self.active_side_terminal().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_sleep(id: &str, dir: &Path) -> EmbeddedTerminal {
        EmbeddedTerminal::spawn_for_test(id, "sleep", &["30"], dir)
    }

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
    fn test_tabs_group_terminals_by_root_and_cycle_wraps() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let mut m = TerminalManager::new();
        m.register(&a, "a", spawn_sleep("a1", &a));
        m.register(&a, "a", spawn_sleep("a2", &a));
        m.register(&b, "b", spawn_sleep("b1", &b));

        assert_eq!(m.tabs.len(), 2);
        assert_eq!(m.active_session_id(), Some("b1"));
        let info = m.tab_info();
        assert_eq!(info[0].count, 2);
        assert_eq!(info[0].current, 2);
        assert!(info[1].is_active);

        m.cycle_next();
        assert_eq!(m.active_session_id(), Some("a2"));
        m.cycle_prev();
        assert_eq!(m.active_session_id(), Some("b1"));

        assert!(m.switch_to("a1"));
        assert_eq!(m.active_session_id(), Some("a1"));
        assert!(!m.switch_to("nope"));
        assert_eq!(
            m.tabs[0].session_ids(),
            vec!["a1".to_string(), "a2".to_string()]
        );

        assert!(m.discard("a1"));
        assert_eq!(m.active_session_id(), Some("a2"));
        assert!(m.discard("a2"));
        assert_eq!(m.tabs.len(), 1);
        assert_eq!(m.active, None);
        assert!(m.discard("b1"));
        assert!(m.tabs.is_empty());
    }

    #[test]
    fn test_tab_markers_and_bells_for_non_current_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        std::fs::create_dir_all(&a).unwrap();
        let mut m = TerminalManager::new();
        m.register(&a, "a", spawn_sleep("a1", &a));
        m.register(&a, "a", spawn_sleep("a2", &a));

        let mut statuses = HashMap::new();
        statuses.insert("a1".to_string(), RegistryStatus::Busy);
        statuses.insert("a2".to_string(), RegistryStatus::Busy);
        assert!(!m.observe_registry(&statuses, true));
        statuses.insert("a1".to_string(), RegistryStatus::Waiting);
        assert!(m.observe_registry(&statuses, true));
        assert!(m.has_bell_for("a1"));
        assert!(!m.has_bell_for("a2"));
        let info = m.tab_info();
        assert!(info[0].waiting);
        assert!(info[0].has_bell);

        statuses.insert("a2".to_string(), RegistryStatus::Idle);
        assert!(m.observe_registry(&statuses, true));
        assert!(!m.has_bell_for("a2"), "focused session gets no visual bell");

        assert!(m.switch_to("a1"));
        assert!(!m.has_bell_for("a1"), "switching clears the bell");
        m.discard("a1");
        m.discard("a2");
    }

    #[test]
    fn test_reconcile_ids_renames_by_pid() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        std::fs::create_dir_all(&a).unwrap();
        let mut m = TerminalManager::new();
        m.register(&a, "a", spawn_sleep("temp-id", &a));
        std::thread::sleep(Duration::from_millis(50));
        let pid = m.terminal("temp-id").unwrap().pid().unwrap();
        let entry = RegistryEntry {
            pid,
            session_id: "real-id".into(),
            name: None,
            status: RegistryStatus::Busy,
            kind: "interactive".into(),
            name_is_custom: false,
            cwd: None,
            messaging_socket_path: None,
            started_at: None,
            waiting_for: None,
        };
        m.reconcile_ids(&[entry]);
        assert!(m.is_attached("real-id"));
        assert!(!m.is_attached("temp-id"));
        assert!(m.notifiers.contains_key("real-id"));
        m.discard("real-id");
    }
}
