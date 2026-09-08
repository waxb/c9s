mod list;
mod support;

pub use list::{QuickSwitch, Row};

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::session::config::{build_config_items, scan_session_config, ConfigItem};
use crate::session::registry::{RegistryStatus, SessionRegistry};
use crate::session::{
    group_projects, names, DiscoveryWorker, Project, Session, SessionFile, SessionStatus,
};
use crate::store::Store;
use crate::terminal::{EmbeddedTerminal, TerminalManager};
use crate::ui::Panes;
use crate::usage::{UsageData, UsageFetcher};
use anyhow::Result;
use list::ListState;

const CI_CHECK_INTERVAL: Duration = Duration::from_secs(120);
const REGISTRY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(5);
const INITIAL_SCAN_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Detail,
    Help,
    Filter,
    QSwitcher,
    Terminal,
    TerminalQSwitcher,
    Command,
    ConfirmQuit,
    ConfirmKill,
    SessionFilePicker,
    Log,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CiStatus {
    NoBranch,
    None,
    Passing,
    Failing,
    Running,
}

pub struct App {
    projects: Vec<Project>,
    list: ListState,
    qswitch: QuickSwitch,
    view_mode: ViewMode,
    command_input: String,
    discovery: DiscoveryWorker,
    registry: SessionRegistry,
    last_registry_poll: Option<Instant>,
    store: Option<Store>,
    should_quit: bool,
    terminal_manager: TerminalManager,
    detail_items: Vec<ConfigItem>,
    detail_cursor: usize,
    detail_preview: Option<(String, String)>,
    detail_preview_scroll: usize,
    usage_fetcher: UsageFetcher,
    usage: UsageData,
    log_scroll: usize,
    side_panel_open: bool,
    side_panel_focused: bool,
    pub config: Config,
    pub ci_statuses: HashMap<String, CiStatus>,
    ci_tx: mpsc::Sender<(String, CiStatus)>,
    ci_rx: mpsc::Receiver<(String, CiStatus)>,
    ci_last_check: Option<Instant>,
    pub confirm_kill_session_id: Option<String>,
    pub session_files: Vec<SessionFile>,
    pub session_file_cursor: usize,
}

impl App {
    pub fn new() -> Result<Self> {
        let (ci_tx, ci_rx) = mpsc::channel();
        let store = Store::open().ok();
        let hidden = store
            .as_ref()
            .and_then(|s| s.hidden_ids().ok())
            .unwrap_or_default();

        let mut app = Self {
            projects: Vec::new(),
            list: ListState::new(hidden),
            qswitch: QuickSwitch::new(),
            view_mode: ViewMode::List,
            command_input: String::new(),
            discovery: DiscoveryWorker::spawn(DISCOVERY_INTERVAL),
            registry: SessionRegistry::new(),
            last_registry_poll: None,
            store,
            should_quit: false,
            terminal_manager: TerminalManager::new(),
            detail_items: Vec::new(),
            detail_cursor: 0,
            detail_preview: None,
            detail_preview_scroll: 0,
            usage_fetcher: UsageFetcher::new(),
            usage: UsageData::default(),
            log_scroll: 0,
            side_panel_open: false,
            side_panel_focused: false,
            config: Config::load(),
            ci_statuses: HashMap::new(),
            ci_tx,
            ci_rx,
            ci_last_check: None,
            confirm_kill_session_id: None,
            session_files: Vec::new(),
            session_file_cursor: 0,
        };

        if let Some(sessions) = app.discovery.wait_first(INITIAL_SCAN_TIMEOUT) {
            app.apply_sessions(sessions);
        }
        app.refresh_usage();
        Ok(app)
    }

    pub fn request_refresh(&self) {
        self.discovery.request_refresh();
    }

    pub fn poll_discovery(&mut self) -> bool {
        match self.discovery.take_latest() {
            Some(sessions) => {
                self.apply_sessions(sessions);
                true
            }
            None => false,
        }
    }

    fn apply_sessions(&mut self, sessions: Vec<Session>) {
        if let Some(ref store) = self.store {
            for session in &sessions {
                let _ = store.upsert_session(session);
            }
        }

        let previous: HashMap<PathBuf, Project> = std::mem::take(&mut self.projects)
            .into_iter()
            .map(|p| (p.root.clone(), p))
            .collect();
        self.projects = group_projects(sessions)
            .into_iter()
            .map(|p| match previous.get(&p.root) {
                Some(old) => old.with_sessions(p.sessions),
                None => p,
            })
            .collect();
        self.rebuild_rows_keeping_selection();
    }

    fn rebuild_rows_keeping_selection(&mut self) {
        let key = self
            .list
            .selected_row()
            .and_then(|row| self.list.row_key(&self.projects, row));
        self.list.sort_projects(&mut self.projects);
        self.list.rebuild_rows(&self.projects);
        if let Some(key) = key {
            self.list.select_key(&self.projects, &key);
        }
    }

    pub fn refresh_usage(&mut self) {
        self.usage = self.usage_fetcher.get().clone();
    }

    pub fn invalidate_usage(&mut self) {
        self.usage_fetcher.invalidate();
    }

    pub fn usage(&self) -> &UsageData {
        &self.usage
    }

    pub fn project(&self, index: usize) -> Option<&Project> {
        self.projects.get(index)
    }

    pub fn rows(&self) -> &[Row] {
        &self.list.rows
    }

    pub fn row_session(&self, row: Row) -> Option<&Session> {
        let project = self.projects.get(row.project_index())?;
        match row {
            Row::Project(_) => self.current_visible_session(project),
            Row::Session(_, s) => project.sessions.get(s),
        }
    }

    pub fn current_visible_session<'a>(&'a self, project: &'a Project) -> Option<&'a Session> {
        project
            .current_session()
            .filter(|s| self.list.is_visible(s))
            .or_else(|| self.list.visible_sessions(project).into_iter().next())
    }

    pub fn visible_sessions<'a>(&'a self, project: &'a Project) -> Vec<&'a Session> {
        self.list.visible_sessions(project)
    }

    pub fn is_expanded(&self, project: &Project) -> bool {
        self.list.is_expanded(project)
    }

    pub fn hide_session(&mut self, id: &str) {
        self.list.hidden.insert(id.to_string());
        if let Some(ref store) = self.store {
            let _ = store.set_hidden(id, true);
        }
        self.rebuild_rows_keeping_selection();
    }

    pub fn set_project_current(&mut self, root: &Path, session_id: &str) {
        if let Some(project) = self.projects.iter_mut().find(|p| p.root == root) {
            if let Some(idx) = project.index_of(session_id) {
                project.current = idx;
            }
        }
    }

    pub fn find_session(&self, session_id: &str) -> Option<&Session> {
        self.projects.iter().find_map(|p| p.find(session_id))
    }

    pub fn filtered_sessions(&self) -> Vec<&Session> {
        let mut seen = HashSet::new();
        self.list
            .rows
            .iter()
            .filter_map(|row| match row {
                Row::Project(p) => self.projects.get(*p),
                Row::Session(..) => None,
            })
            .flat_map(|p| self.list.visible_sessions(p))
            .filter(|s| seen.insert(s.id.clone()))
            .collect()
    }

    pub fn selected_project(&self) -> Option<&Project> {
        self.list
            .selected_row()
            .and_then(|row| self.projects.get(row.project_index()))
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.list
            .selected_row()
            .and_then(|row| self.row_session(row))
    }

    pub fn selected_index(&self) -> usize {
        self.list.selected
    }

    pub fn nth_project_session(&self, n: usize) -> Option<&Session> {
        self.list
            .nth_project_row(n)
            .and_then(|row| self.row_session(row))
    }

    pub fn expand_selected(&mut self) {
        if let Some(root) = self.selected_project().map(|p| p.root.clone()) {
            self.list.expand(&root);
            self.rebuild_rows_keeping_selection();
        }
    }

    pub fn collapse_selected(&mut self) {
        if let Some(project) = self.selected_project() {
            let root = project.root.clone();
            self.list.collapse(&root);
            self.rebuild_rows_keeping_selection();
            self.list.select_project_root(&self.projects, &root);
        }
    }

    pub fn toggle_expand_selected(&mut self) {
        match self.selected_project() {
            Some(project) if self.list.expanded.contains(&project.root) => self.collapse_selected(),
            Some(_) => self.expand_selected(),
            None => {}
        }
    }

    pub fn quick_switch(&self) -> &QuickSwitch {
        &self.qswitch
    }

    pub fn quick_switch_rows(&self) -> Vec<Row> {
        self.qswitch.rows(&self.list, &self.projects)
    }

    pub fn quick_switch_selected_session(&self) -> Option<&Session> {
        self.qswitch
            .selected_row(&self.list, &self.projects)
            .and_then(|row| self.row_session(row))
    }

    pub fn quick_switch_nth_project_session(&self, n: usize) -> Option<&Session> {
        self.qswitch
            .nth_project(&self.list, &self.projects, n)
            .and_then(|row| self.row_session(row))
    }

    pub fn quick_switch_expand(&mut self) {
        self.qswitch.expand(&self.list, &self.projects);
    }

    pub fn quick_switch_collapse(&mut self) {
        self.qswitch.collapse(&self.list, &self.projects);
    }

    fn open_quick_switch(&mut self) {
        let root = self
            .attached_session_id()
            .and_then(|id| self.find_session(id))
            .map(|s| s.cwd.clone())
            .or_else(|| self.selected_project().map(|p| p.root.clone()));
        self.qswitch = QuickSwitch::new();
        if let Some(root) = root {
            let rows = self.qswitch.rows(&self.list, &self.projects);
            if let Some(pos) = rows
                .iter()
                .position(|r| self.projects.get(r.project_index()).map(|p| &p.root) == Some(&root))
            {
                self.qswitch.selected = pos;
            }
        }
    }

    pub fn view_mode(&self) -> &ViewMode {
        &self.view_mode
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if matches!(mode, ViewMode::QSwitcher | ViewMode::TerminalQSwitcher)
            && !matches!(
                self.view_mode,
                ViewMode::QSwitcher | ViewMode::TerminalQSwitcher
            )
        {
            self.open_quick_switch();
        }
        if mode == ViewMode::Detail {
            if let Some(session) = self.selected_session() {
                let cfg = scan_session_config(&session.cwd);
                self.detail_items = build_config_items(&cfg, &session.cwd);
                self.detail_cursor = 0;
                self.detail_preview = None;
                self.detail_preview_scroll = 0;
            }
        } else if mode != ViewMode::SessionFilePicker {
            self.detail_items.clear();
            self.detail_preview = None;
        }
        self.view_mode = mode;
    }

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

    pub fn sort_label(&self) -> &str {
        self.list.sort_column.label()
    }

    pub fn cycle_sort(&mut self) {
        self.list.sort_column = self.list.sort_column.next();
        self.rebuild_rows_keeping_selection();
    }

    pub fn move_up(&mut self) {
        match self.view_mode {
            ViewMode::Detail => {
                if self.detail_preview.is_some() {
                    self.detail_preview_scroll = self.detail_preview_scroll.saturating_sub(1);
                } else {
                    self.detail_cursor = self.detail_cursor.saturating_sub(1);
                }
            }
            ViewMode::Log => self.log_scroll_up(),
            ViewMode::QSwitcher | ViewMode::TerminalQSwitcher => self.qswitch.move_up(),
            _ => self.list.move_up(),
        }
    }

    pub fn move_down(&mut self) {
        match self.view_mode {
            ViewMode::Detail => {
                if self.detail_preview.is_some() {
                    self.detail_preview_scroll += 1;
                } else if self.detail_cursor + 1 < self.detail_items.len() {
                    self.detail_cursor += 1;
                }
            }
            ViewMode::Log => self.log_scroll_down(),
            ViewMode::QSwitcher | ViewMode::TerminalQSwitcher => {
                self.qswitch.move_down(&self.list, &self.projects)
            }
            _ => self.list.move_down(),
        }
    }

    pub fn move_to_top(&mut self) {
        if self.view_mode == ViewMode::Log {
            self.log_scroll_to_top();
        } else {
            self.list.move_to_top();
        }
    }

    pub fn move_to_bottom(&mut self) {
        if self.view_mode == ViewMode::Log {
            self.log_scroll_to_bottom();
        } else {
            self.list.move_to_bottom();
        }
    }

    pub fn is_filtering(&self) -> bool {
        self.view_mode == ViewMode::Filter || !self.list.filter_query.is_empty()
    }

    pub fn has_active_filter(&self) -> bool {
        !self.list.filter_query.is_empty()
    }

    pub fn filter_query(&self) -> &str {
        &self.list.filter_query
    }

    pub fn filter_push(&mut self, c: char) {
        self.list.filter_query.push(c);
        self.list.rebuild_rows(&self.projects);
        self.list.selected = 0;
    }

    pub fn filter_pop(&mut self) {
        self.list.filter_query.pop();
        self.list.rebuild_rows(&self.projects);
        self.list.selected = 0;
    }

    pub fn clear_filter(&mut self) {
        self.list.filter_query.clear();
        self.rebuild_rows_keeping_selection();
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn terminal_manager(&self) -> &TerminalManager {
        &self.terminal_manager
    }

    pub fn terminal_manager_mut(&mut self) -> &mut TerminalManager {
        &mut self.terminal_manager
    }

    pub fn attached_session_id(&self) -> Option<&str> {
        self.terminal_manager.active_session_id()
    }

    pub fn has_bell(&self, session_id: &str) -> bool {
        self.terminal_manager.has_bell_for(session_id)
    }

    pub fn active_attached_sessions(&self) -> Vec<String> {
        self.terminal_manager
            .live_session_ids()
            .into_iter()
            .map(|(project, id)| match self.find_session(&id) {
                Some(s) => format!("{}: {}", project, crate::ui::session_title(s)),
                None => format!("{}: {}", project, &id[..8.min(id.len())]),
            })
            .collect()
    }

    pub fn panes(&self) -> Panes {
        Panes {
            session: false,
            shell: self.is_side_panel_open(),
        }
    }

    pub fn generate_session_name(&self) -> String {
        let taken: HashSet<String> = self
            .registry
            .live_interactive()
            .into_iter()
            .filter_map(|e| e.name)
            .collect();
        names::generate(&taken)
    }

    pub fn poll_registry(&mut self, viewing_active: bool) -> bool {
        if self
            .last_registry_poll
            .is_some_and(|t| t.elapsed() < REGISTRY_POLL_INTERVAL)
        {
            return false;
        }
        self.last_registry_poll = Some(Instant::now());
        let entries = self.registry.live_interactive();
        self.terminal_manager.reconcile_ids(&entries);
        let statuses: HashMap<String, RegistryStatus> = entries
            .into_iter()
            .map(|e| (e.session_id, e.status))
            .collect();
        self.terminal_manager
            .observe_registry(&statuses, viewing_active)
    }

    pub fn command_input(&self) -> &str {
        &self.command_input
    }

    pub fn command_push(&mut self, c: char) {
        self.command_input.push(c);
    }

    pub fn command_pop(&mut self) {
        self.command_input.pop();
    }

    pub fn command_take(&mut self) -> String {
        std::mem::take(&mut self.command_input)
    }

    pub fn command_tab_complete(&mut self) {
        if let Some(completed) = support::complete_directory(&self.command_input) {
            self.command_input = completed;
        }
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

pub fn expand_home(input: &str) -> String {
    support::expand_home(input)
}
