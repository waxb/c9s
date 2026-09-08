use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::session::config::{build_config_items, scan_session_config, ConfigItem};
use crate::session::registry::{RegistryStatus, SessionRegistry};
use crate::session::{Session, SessionDiscovery, SessionFile, SessionStatus};
use crate::store::Store;
use crate::terminal::{EmbeddedTerminal, TerminalManager};
use crate::usage::{UsageData, UsageFetcher};
use anyhow::Result;

const CI_CHECK_INTERVAL: Duration = Duration::from_secs(120);
const REGISTRY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const QSWITCHER_MAX: usize = 9;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    LastActive,
    Project,
    Cost,
    Status,
    Tokens,
}

impl SortColumn {
    pub fn next(self) -> Self {
        match self {
            Self::LastActive => Self::Project,
            Self::Project => Self::Cost,
            Self::Cost => Self::Status,
            Self::Status => Self::Tokens,
            Self::Tokens => Self::LastActive,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::LastActive => "Last Active",
            Self::Project => "Project",
            Self::Cost => "Cost",
            Self::Status => "Status",
            Self::Tokens => "Tokens",
        }
    }
}

fn status_rank(status: &SessionStatus) -> u8 {
    match status {
        SessionStatus::Thinking => 0,
        SessionStatus::Waiting => 1,
        SessionStatus::Active => 2,
        SessionStatus::Idle => 3,
        SessionStatus::Dead => 4,
    }
}

pub struct App {
    sessions: Vec<Session>,
    filtered: Vec<usize>,
    selected: usize,
    view_mode: ViewMode,
    sort_column: SortColumn,
    filter_query: String,
    command_input: String,
    discovery: SessionDiscovery,
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
    pub ci_statuses: HashMap<String, CiStatus>,
    ci_tx: mpsc::Sender<(String, CiStatus)>,
    ci_rx: mpsc::Receiver<(String, CiStatus)>,
    ci_last_check: Option<Instant>,
    pub confirm_kill_session_id: Option<String>,
    unfollowed: HashSet<String>,
    pub session_files: Vec<SessionFile>,
    pub session_file_cursor: usize,
}

impl App {
    pub fn new() -> Result<Self> {
        let (ci_tx, ci_rx) = mpsc::channel();

        let mut app = Self {
            sessions: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            view_mode: ViewMode::List,
            sort_column: SortColumn::LastActive,
            filter_query: String::new(),
            command_input: String::new(),
            discovery: SessionDiscovery::new(),
            registry: SessionRegistry::new(),
            last_registry_poll: None,
            store: Store::open().ok(),
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
            ci_statuses: HashMap::new(),
            ci_tx,
            ci_rx,
            ci_last_check: None,
            confirm_kill_session_id: None,
            unfollowed: HashSet::new(),
            session_files: Vec::new(),
            session_file_cursor: 0,
        };

        app.refresh()?;
        Ok(app)
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.sessions = self.discovery.discover_all().unwrap_or_default();

        if let Some(ref store) = self.store {
            for session in &self.sessions {
                let _ = store.upsert_session(session);
            }
        }

        self.apply_sort();
        self.apply_filter();
        self.usage = self.usage_fetcher.get().clone();
        Ok(())
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

    pub fn unfollow_session(&mut self, id: &str) {
        self.unfollowed.insert(id.to_string());
        self.apply_filter();
    }

    fn apply_sort(&mut self) {
        match self.sort_column {
            SortColumn::LastActive => self
                .sessions
                .sort_by_key(|s| std::cmp::Reverse(s.last_activity)),
            SortColumn::Project => self
                .sessions
                .sort_by(|a, b| a.project_name.cmp(&b.project_name)),
            SortColumn::Cost => self.sessions.sort_by(|a, b| {
                b.estimated_cost_usd()
                    .partial_cmp(&a.estimated_cost_usd())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortColumn::Status => self.sessions.sort_by_key(|s| status_rank(&s.status)),
            SortColumn::Tokens => self
                .sessions
                .sort_by_key(|s| std::cmp::Reverse(s.total_tokens())),
        }
    }

    fn apply_filter(&mut self) {
        let query = self.filter_query.to_lowercase();
        self.filtered = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| !self.unfollowed.contains(&s.id) && s.matches_filter(&query))
            .map(|(i, _)| i)
            .collect();
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        if self.selected >= self.filtered.len() && !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
        }
    }

    pub fn filtered_sessions(&self) -> Vec<&Session> {
        self.filtered
            .iter()
            .filter_map(|&i| self.sessions.get(i))
            .collect()
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.sessions.get(i))
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn view_mode(&self) -> &ViewMode {
        &self.view_mode
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if matches!(mode, ViewMode::QSwitcher | ViewMode::TerminalQSwitcher) {
            let max = self.filtered.len().min(QSWITCHER_MAX);
            if self.selected >= max && max > 0 {
                self.selected = max - 1;
            }
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
        self.sort_column.label()
    }

    pub fn cycle_sort(&mut self) {
        self.sort_column = self.sort_column.next();
        self.apply_sort();
        self.apply_filter();
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
            _ => self.selected = self.selected.saturating_sub(1),
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
                let limit = self.filtered.len().min(QSWITCHER_MAX);
                if self.selected + 1 < limit {
                    self.selected += 1;
                }
            }
            _ => {
                if self.selected + 1 < self.filtered.len() {
                    self.selected += 1;
                }
            }
        }
    }

    pub fn move_to_top(&mut self) {
        if self.view_mode == ViewMode::Log {
            self.log_scroll_to_top();
        } else {
            self.selected = 0;
        }
    }

    pub fn move_to_bottom(&mut self) {
        if self.view_mode == ViewMode::Log {
            self.log_scroll_to_bottom();
        } else if !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
        }
    }

    pub fn is_filtering(&self) -> bool {
        self.view_mode == ViewMode::Filter || !self.filter_query.is_empty()
    }

    pub fn has_active_filter(&self) -> bool {
        !self.filter_query.is_empty()
    }

    pub fn filter_query(&self) -> &str {
        &self.filter_query
    }

    pub fn filter_push(&mut self, c: char) {
        self.filter_query.push(c);
        self.apply_filter();
        self.selected = 0;
    }

    pub fn filter_pop(&mut self) {
        self.filter_query.pop();
        self.apply_filter();
        self.selected = 0;
    }

    pub fn clear_filter(&mut self) {
        self.filter_query.clear();
        self.apply_filter();
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
        self.terminal_manager.live_session_names()
    }

    pub fn poll_registry(&mut self, viewing_active: bool) -> bool {
        if self
            .last_registry_poll
            .is_some_and(|t| t.elapsed() < REGISTRY_POLL_INTERVAL)
        {
            return false;
        }
        self.last_registry_poll = Some(Instant::now());
        let statuses: HashMap<String, RegistryStatus> = self
            .registry
            .live_interactive()
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
        if let Some(completed) = complete_directory(&self.command_input) {
            self.command_input = completed;
        }
    }

    pub fn all_sessions(&self) -> &[Session] {
        &self.sessions
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
            .attached_session_id()
            .and_then(|sid| self.sessions.iter().find(|s| s.id == sid))
            .map(|s| s.cwd.clone())
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
                        let status = check_ci_for_local(&cwd, &branch);
                        let _ = tx.send((id, status));
                    });
                }
            }
        }
    }
}

fn expand_home(input: &str) -> String {
    match (input.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.to_string_lossy().to_string() + rest,
        _ => input.to_string(),
    }
}

fn contract_home(path: &str) -> String {
    match dirs::home_dir() {
        Some(home) => {
            let home_str = home.to_string_lossy().to_string();
            match path.strip_prefix(&home_str) {
                Some(rest) => format!("~{}", rest),
                None => path.to_string(),
            }
        }
        None => path.to_string(),
    }
}

fn complete_directory(input: &str) -> Option<String> {
    let expanded = expand_home(input);
    let path = Path::new(&expanded);
    let (dir, prefix) = if path.is_dir() && expanded.ends_with('/') {
        (path.to_path_buf(), String::new())
    } else {
        let dir = path.parent().unwrap_or(Path::new("."));
        let prefix = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        (dir.to_path_buf(), prefix)
    };

    let entries = std::fs::read_dir(&dir).ok()?;
    let mut matches: Vec<String> = entries
        .flatten()
        .filter(|e| {
            e.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                && e.file_name().to_string_lossy().starts_with(&prefix)
        })
        .map(|e| {
            let full = format!("{}/", dir.join(e.file_name()).to_string_lossy());
            if input.starts_with('~') {
                contract_home(&full)
            } else {
                full
            }
        })
        .collect();
    matches.sort();

    match matches.len() {
        0 => None,
        1 => matches.pop(),
        _ => {
            let common = longest_common_prefix(&matches);
            (common.len() > input.len()).then_some(common)
        }
    }
}

fn longest_common_prefix(strings: &[String]) -> String {
    let Some(first) = strings.first() else {
        return String::new();
    };
    let mut len = first.len();
    for s in &strings[1..] {
        len = len.min(s.len());
        for (i, (a, b)) in first.chars().zip(s.chars()).enumerate() {
            if a != b {
                len = len.min(i);
                break;
            }
        }
    }
    first[..len].to_string()
}

fn check_ci_for_local(cwd: &Path, branch: &str) -> CiStatus {
    let output = std::process::Command::new("gh")
        .args([
            "run",
            "list",
            "--branch",
            branch,
            "--limit",
            "1",
            "--json",
            "conclusion,status",
        ])
        .current_dir(cwd)
        .output();
    match output {
        Ok(o) if o.status.success() => parse_gh_runs_json(&String::from_utf8_lossy(&o.stdout)),
        _ => CiStatus::None,
    }
}

fn parse_gh_runs_json(text: &str) -> CiStatus {
    let runs: Vec<serde_json::Value> = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return CiStatus::None,
    };
    let Some(run) = runs.first() else {
        return CiStatus::None;
    };
    let status = run.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let conclusion = run.get("conclusion").and_then(|v| v.as_str()).unwrap_or("");

    if status == "in_progress" || status == "queued" {
        CiStatus::Running
    } else if conclusion == "success" {
        CiStatus::Passing
    } else if conclusion == "failure" {
        CiStatus::Failing
    } else {
        CiStatus::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_longest_common_prefix() {
        let items = vec!["/a/foo/".to_string(), "/a/foobar/".to_string()];
        assert_eq!(longest_common_prefix(&items), "/a/foo");
        assert_eq!(longest_common_prefix(&[]), "");
        assert_eq!(longest_common_prefix(&["/x/".to_string()]), "/x/");
    }

    #[test]
    fn test_parse_gh_runs_json() {
        assert_eq!(
            parse_gh_runs_json(r#"[{"status":"completed","conclusion":"success"}]"#),
            CiStatus::Passing
        );
        assert_eq!(
            parse_gh_runs_json(r#"[{"status":"completed","conclusion":"failure"}]"#),
            CiStatus::Failing
        );
        assert_eq!(
            parse_gh_runs_json(r#"[{"status":"in_progress","conclusion":""}]"#),
            CiStatus::Running
        );
        assert_eq!(parse_gh_runs_json("[]"), CiStatus::None);
        assert_eq!(parse_gh_runs_json("not json"), CiStatus::None);
    }

    #[test]
    fn test_complete_directory_single_match() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("alpha")).unwrap();
        std::fs::create_dir(dir.path().join("beta")).unwrap();
        let input = format!("{}/al", dir.path().display());
        let completed = complete_directory(&input).unwrap();
        assert!(completed.ends_with("/alpha/"));
    }

    #[test]
    fn test_complete_directory_common_prefix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("proj-one")).unwrap();
        std::fs::create_dir(dir.path().join("proj-two")).unwrap();
        let input = format!("{}/p", dir.path().display());
        let completed = complete_directory(&input).unwrap();
        assert!(completed.ends_with("/proj-"));
        assert_eq!(complete_directory(&completed), None);
    }

    #[test]
    fn test_sort_column_cycle() {
        let mut col = SortColumn::LastActive;
        for _ in 0..5 {
            col = col.next();
        }
        assert_eq!(col, SortColumn::LastActive);
    }
}
