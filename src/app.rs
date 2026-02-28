use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

use crate::session::config::{build_config_items, scan_session_config, ConfigItem};
use crate::session::{Session, SessionConfig, SessionDiscovery, SessionStatus};
use crate::store::Store;
use crate::terminal::TerminalManager;
use crate::tervezo::{
    FileChange, Implementation, ImplementationStatus, SseMessage, SseStream, SshCredentials,
    TervezoConfig, TervezoFetcher, TimelineMessage,
};
use crate::usage::{UsageData, UsageFetcher};
use anyhow::Result;

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
    TervezoDetail,
    Log,
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

#[derive(Debug, Clone)]
pub enum SessionEntry {
    Local(Session),
    Remote(Implementation),
}

impl SessionEntry {
    pub fn id(&self) -> &str {
        match self {
            Self::Local(s) => &s.id,
            Self::Remote(i) => &i.id,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Local(s) => &s.project_name,
            Self::Remote(i) => i.display_name(),
        }
    }

    pub fn status_label(&self) -> &str {
        match self {
            Self::Local(s) => s.status.label(),
            Self::Remote(i) => i.status.label(),
        }
    }

    pub fn last_activity_display(&self) -> String {
        match self {
            Self::Local(s) => s.last_activity_display(),
            Self::Remote(i) => i.last_activity_display(),
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote(_))
    }

    pub fn matches_filter(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let q = query.to_lowercase();
        match self {
            Self::Local(s) => {
                s.project_name.to_lowercase().contains(&q)
                    || s.cwd.to_string_lossy().to_lowercase().contains(&q)
                    || s.git_branch
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
                    || s.model.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    || s.status.label().to_lowercase().contains(&q)
            }
            Self::Remote(i) => {
                i.display_name().to_lowercase().contains(&q)
                    || i.branch
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
                    || i.repo_url
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
                    || i.status.label().to_lowercase().contains(&q)
                    || "tervezo".contains(&q)
            }
        }
    }

    pub fn branch(&self) -> Option<&str> {
        match self {
            Self::Local(s) => s.git_branch.as_deref(),
            Self::Remote(i) => i.branch.as_deref(),
        }
    }

    pub fn estimated_cost(&self) -> Option<f64> {
        match self {
            Self::Local(s) => Some(s.estimated_cost_usd()),
            Self::Remote(i) => i.estimated_cost_usd,
        }
    }

    pub fn total_tokens(&self) -> Option<u64> {
        match self {
            Self::Local(s) => Some(s.total_tokens()),
            Self::Remote(i) => i.total_tokens,
        }
    }

    pub fn message_count(&self) -> Option<u32> {
        match self {
            Self::Local(s) => Some(s.message_count),
            Self::Remote(i) => i.message_count,
        }
    }

    pub fn as_local(&self) -> Option<&Session> {
        match self {
            Self::Local(s) => Some(s),
            Self::Remote(_) => None,
        }
    }

    pub fn as_remote(&self) -> Option<&Implementation> {
        match self {
            Self::Local(_) => None,
            Self::Remote(i) => Some(i),
        }
    }

    fn sort_key_last_activity(&self) -> chrono::DateTime<chrono::Utc> {
        match self {
            Self::Local(s) => s.last_activity,
            Self::Remote(i) => i.updated_at.or(i.created_at).unwrap_or_default(),
        }
    }

    fn sort_key_project(&self) -> &str {
        self.display_name()
    }

    fn sort_key_cost(&self) -> f64 {
        self.estimated_cost().unwrap_or(0.0)
    }

    fn sort_key_status(&self) -> u8 {
        match self {
            Self::Local(s) => match s.status {
                SessionStatus::Thinking => 0,
                SessionStatus::Active => 1,
                SessionStatus::Idle => 2,
                SessionStatus::Dead => 3,
            },
            Self::Remote(i) => match i.status {
                ImplementationStatus::Running => 1,
                ImplementationStatus::Pending | ImplementationStatus::Queued => 2,
                ImplementationStatus::Completed | ImplementationStatus::Merged => 3,
                ImplementationStatus::Failed => 4,
                ImplementationStatus::Stopped | ImplementationStatus::Cancelled => 5,
            },
        }
    }

    fn sort_key_tokens(&self) -> u64 {
        self.total_tokens().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TervezoTab {
    Plan,
    Changes,
    TestOutput,
    Analysis,
}

impl TervezoTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Plan => "Plan",
            Self::Changes => "Changes",
            Self::TestOutput => "Tests",
            Self::Analysis => "Analysis",
        }
    }

    pub fn all() -> &'static [TervezoTab] {
        &[Self::Plan, Self::Changes, Self::TestOutput, Self::Analysis]
    }

    pub fn next(self) -> Self {
        match self {
            Self::Plan => Self::Changes,
            Self::Changes => Self::TestOutput,
            Self::TestOutput => Self::Analysis,
            Self::Analysis => Self::Plan,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Plan => Self::Analysis,
            Self::Changes => Self::Plan,
            Self::TestOutput => Self::Changes,
            Self::Analysis => Self::TestOutput,
        }
    }
}

#[allow(dead_code)]
pub enum TervezoDetailMsg {
    Timeline(Vec<TimelineMessage>),
    TimelineAppend(TimelineMessage),
    Plan(String),
    Analysis(String),
    Changes(Vec<FileChange>),
    TestOutput(String),
    SshCreds(SshCredentials),
    Error(TervezoTab, String),
}

pub struct TervezoDetailState {
    pub implementation_id: String,
    pub implementation: Implementation,
    pub active_tab: TervezoTab,
    pub timeline: Vec<TimelineMessage>,
    pub timeline_scroll: usize,
    pub plan_content: Option<String>,
    pub analysis_content: Option<String>,
    pub changes: Option<Vec<FileChange>>,
    pub changes_selected_file: usize,
    pub changes_expanded: HashSet<usize>,
    pub changes_diff_scroll: usize,
    pub test_output: Option<String>,
    pub ssh_creds: Option<SshCredentials>,
    pub loading: HashSet<TervezoTab>,
    pub plan_scroll: usize,
    pub changes_scroll: usize,
    pub test_scroll: usize,
    pub analysis_scroll: usize,
    pub timeline_at_bottom: bool,
}

impl TervezoDetailState {
    pub fn new(implementation: Implementation) -> Self {
        let id = implementation.id.clone();
        Self {
            implementation_id: id,
            implementation,
            active_tab: TervezoTab::Plan,
            timeline: Vec::new(),
            timeline_scroll: 0,
            plan_content: None,
            analysis_content: None,
            changes: None,
            changes_selected_file: 0,
            changes_expanded: HashSet::new(),
            changes_diff_scroll: 0,
            test_output: None,
            ssh_creds: None,
            loading: HashSet::new(),
            plan_scroll: 0,
            changes_scroll: 0,
            test_scroll: 0,
            analysis_scroll: 0,
            timeline_at_bottom: true,
        }
    }

    #[allow(dead_code)]
    pub fn active_tab_scroll(&self) -> usize {
        match self.active_tab {
            TervezoTab::Plan => self.plan_scroll,
            TervezoTab::Changes => self.changes_scroll,
            TervezoTab::TestOutput => self.test_scroll,
            TervezoTab::Analysis => self.analysis_scroll,
        }
    }

    pub fn scroll_active_tab_up(&mut self) {
        match self.active_tab {
            TervezoTab::Plan => {
                self.plan_scroll = self.plan_scroll.saturating_sub(1);
            }
            TervezoTab::Changes => {
                if self.changes_expanded.contains(&self.changes_selected_file) {
                    // Scroll diff content
                    self.changes_diff_scroll = self.changes_diff_scroll.saturating_sub(1);
                } else {
                    // Navigate file list
                    self.changes_selected_file = self.changes_selected_file.saturating_sub(1);
                    self.changes_diff_scroll = 0;
                }
            }
            TervezoTab::TestOutput => {
                self.test_scroll = self.test_scroll.saturating_sub(1);
            }
            TervezoTab::Analysis => {
                self.analysis_scroll = self.analysis_scroll.saturating_sub(1);
            }
        }
    }

    pub fn scroll_active_tab_down(&mut self) {
        match self.active_tab {
            TervezoTab::Plan => self.plan_scroll += 1,
            TervezoTab::Changes => {
                if self.changes_expanded.contains(&self.changes_selected_file) {
                    // Scroll diff content
                    self.changes_diff_scroll += 1;
                } else {
                    // Navigate file list
                    let max = self
                        .changes
                        .as_ref()
                        .map(|c| c.len().saturating_sub(1))
                        .unwrap_or(0);
                    if self.changes_selected_file < max {
                        self.changes_selected_file += 1;
                        self.changes_diff_scroll = 0;
                    }
                }
            }
            TervezoTab::TestOutput => self.test_scroll += 1,
            TervezoTab::Analysis => self.analysis_scroll += 1,
        }
    }

    pub fn toggle_changes_expand(&mut self) {
        let idx = self.changes_selected_file;
        if self.changes_expanded.contains(&idx) {
            self.changes_expanded.remove(&idx);
            self.changes_diff_scroll = 0;
        } else {
            self.changes_expanded.insert(idx);
            self.changes_diff_scroll = 0;
        }
    }
}

pub struct App {
    local_sessions: Vec<Session>,
    entries: Vec<SessionEntry>,
    filtered: Vec<usize>,
    selected: usize,
    view_mode: ViewMode,
    sort_column: SortColumn,
    filter_query: String,
    command_input: String,
    discovery: SessionDiscovery,
    store: Option<Store>,
    should_quit: bool,
    terminal_manager: TerminalManager,
    detail_config: Option<SessionConfig>,
    detail_items: Vec<ConfigItem>,
    detail_cursor: usize,
    detail_preview: Option<(String, String)>,
    detail_preview_scroll: usize,
    usage_fetcher: UsageFetcher,
    usage: UsageData,
    tervezo_config: Option<TervezoConfig>,
    tervezo_fetcher: Option<TervezoFetcher>,
    pub tervezo_detail: Option<TervezoDetailState>,
    pub tervezo_detail_tx: Option<mpsc::Sender<TervezoDetailMsg>>,
    tervezo_detail_rx: Option<mpsc::Receiver<TervezoDetailMsg>>,
    ssh_cache: HashMap<String, SshCredentials>,
    sse_stream: Option<SseStream>,
    sse_rx: Option<mpsc::Receiver<SseMessage>>,
    log_scroll: usize,
}

impl App {
    pub fn new() -> Result<Self> {
        let discovery = SessionDiscovery::new();
        let store = Store::open().ok();

        let tervezo_config = TervezoConfig::load();
        let tervezo_fetcher = tervezo_config.as_ref().map(TervezoFetcher::spawn);

        let mut app = Self {
            local_sessions: Vec::new(),
            entries: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            view_mode: ViewMode::List,
            sort_column: SortColumn::LastActive,
            filter_query: String::new(),
            command_input: String::new(),
            discovery,
            store,
            should_quit: false,
            terminal_manager: TerminalManager::new(),
            detail_config: None,
            detail_items: Vec::new(),
            detail_cursor: 0,
            detail_preview: None,
            detail_preview_scroll: 0,
            usage_fetcher: UsageFetcher::new(),
            usage: UsageData::default(),
            tervezo_config,
            tervezo_fetcher,
            tervezo_detail: None,
            tervezo_detail_tx: None,
            tervezo_detail_rx: None,
            ssh_cache: HashMap::new(),
            sse_stream: None,
            sse_rx: None,
            log_scroll: 0,
        };

        app.refresh()?;
        Ok(app)
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.local_sessions = self.discovery.discover_all().unwrap_or_default();

        if let Some(ref store) = self.store {
            for session in &self.local_sessions {
                let _ = store.upsert_session(session);
            }
        }

        self.merge_entries();
        self.apply_sort();
        self.apply_filter();

        if self.selected >= self.filtered.len() && !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
        }

        self.usage = self.usage_fetcher.get().clone();

        Ok(())
    }

    pub fn check_tervezo_dirty(&mut self) -> bool {
        if let Some(ref fetcher) = self.tervezo_fetcher {
            if fetcher.take_dirty() {
                self.merge_entries();
                self.apply_sort();
                self.apply_filter();
                if self.selected >= self.filtered.len() && !self.filtered.is_empty() {
                    self.selected = self.filtered.len() - 1;
                }
                return true;
            }
        }
        false
    }

    pub fn drain_tervezo_detail_messages(&mut self) -> bool {
        let rx = match self.tervezo_detail_rx.as_ref() {
            Some(rx) => rx,
            None => return false,
        };

        let mut changed = false;
        while let Ok(msg) = rx.try_recv() {
            if let Some(ref mut state) = self.tervezo_detail {
                match msg {
                    TervezoDetailMsg::Timeline(msgs) => {
                        state.timeline = msgs;
                        changed = true;
                    }
                    TervezoDetailMsg::TimelineAppend(msg) => {
                        state.timeline.push(msg);
                        if state.timeline.len() > 1000 {
                            state.timeline.drain(..state.timeline.len() - 1000);
                        }
                        changed = true;
                    }
                    TervezoDetailMsg::Plan(content) => {
                        state.plan_content = Some(content);
                        state.loading.remove(&TervezoTab::Plan);
                        changed = true;
                    }
                    TervezoDetailMsg::Analysis(content) => {
                        state.analysis_content = Some(content);
                        state.loading.remove(&TervezoTab::Analysis);
                        changed = true;
                    }
                    TervezoDetailMsg::Changes(changes) => {
                        state.changes = Some(changes);
                        state.loading.remove(&TervezoTab::Changes);
                        changed = true;
                    }
                    TervezoDetailMsg::TestOutput(output) => {
                        state.test_output = Some(output);
                        state.loading.remove(&TervezoTab::TestOutput);
                        changed = true;
                    }
                    TervezoDetailMsg::SshCreds(creds) => {
                        let id = state.implementation_id.clone();
                        state.ssh_creds = Some(creds.clone());
                        self.ssh_cache.insert(id, creds);
                        changed = true;
                    }
                    TervezoDetailMsg::Error(tab, _err) => {
                        state.loading.remove(&tab);
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    pub fn drain_sse_messages(&mut self) -> bool {
        let rx = match self.sse_rx.as_ref() {
            Some(rx) => rx,
            None => return false,
        };

        let mut changed = false;
        while let Ok(msg) = rx.try_recv() {
            if let Some(ref mut state) = self.tervezo_detail {
                match msg {
                    SseMessage::Event(timeline_msg) => {
                        state.timeline.push(*timeline_msg);
                        if state.timeline.len() > 1000 {
                            state.timeline.drain(..state.timeline.len() - 1000);
                        }
                        changed = true;
                    }
                    SseMessage::Error(_) => {
                        // SSE errors are handled by reconnection logic in the stream itself
                    }
                }
            }
        }
        changed
    }

    pub fn start_sse_stream(&mut self, implementation_id: &str) {
        self.stop_sse_stream();

        let config = match self.tervezo_config.as_ref() {
            Some(c) => c.clone(),
            None => return,
        };

        let (tx, rx) = mpsc::channel();
        let stream = SseStream::connect(&config, implementation_id, None, tx);
        self.sse_stream = Some(stream);
        self.sse_rx = Some(rx);
    }

    pub fn stop_sse_stream(&mut self) {
        self.sse_stream = None;
        self.sse_rx = None;
    }

    fn merge_entries(&mut self) {
        let mut entries: Vec<SessionEntry> = self
            .local_sessions
            .iter()
            .cloned()
            .map(SessionEntry::Local)
            .collect();

        if let Some(ref fetcher) = self.tervezo_fetcher {
            let remote = fetcher.implementations();
            for imp in remote {
                entries.push(SessionEntry::Remote(imp));
            }
        }

        self.entries = entries;
    }

    pub fn usage(&self) -> &UsageData {
        &self.usage
    }

    fn apply_sort(&mut self) {
        match self.sort_column {
            SortColumn::LastActive => {
                self.entries
                    .sort_by_key(|e| std::cmp::Reverse(e.sort_key_last_activity()));
            }
            SortColumn::Project => {
                self.entries
                    .sort_by(|a, b| a.sort_key_project().cmp(b.sort_key_project()));
            }
            SortColumn::Cost => {
                self.entries.sort_by(|a, b| {
                    b.sort_key_cost()
                        .partial_cmp(&a.sort_key_cost())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SortColumn::Status => {
                self.entries.sort_by_key(|e| e.sort_key_status());
            }
            SortColumn::Tokens => {
                self.entries
                    .sort_by_key(|e| std::cmp::Reverse(e.sort_key_tokens()));
            }
        }
    }

    fn apply_filter(&mut self) {
        let query = self.filter_query.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.matches_filter(&query))
            .map(|(i, _)| i)
            .collect();
    }

    pub fn filtered_sessions(&self) -> Vec<&SessionEntry> {
        self.filtered
            .iter()
            .filter_map(|&i| self.entries.get(i))
            .collect()
    }

    pub fn selected_session(&self) -> Option<&SessionEntry> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.entries.get(i))
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn set_selected(&mut self, idx: usize) {
        self.selected = idx;
    }

    pub fn view_mode(&self) -> &ViewMode {
        &self.view_mode
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if matches!(mode, ViewMode::QSwitcher | ViewMode::TerminalQSwitcher) {
            let max = self.filtered.len().min(9);
            if self.selected >= max && max > 0 {
                self.selected = max - 1;
            }
        }
        if mode == ViewMode::Detail {
            if let Some(entry) = self.selected_session() {
                if let Some(session) = entry.as_local() {
                    let cfg = scan_session_config(&session.cwd);
                    self.detail_items = build_config_items(&cfg, &session.cwd);
                    self.detail_config = Some(cfg);
                    self.detail_cursor = 0;
                    self.detail_preview = None;
                    self.detail_preview_scroll = 0;
                }
            }
        } else if mode != ViewMode::TervezoDetail {
            self.detail_config = None;
            self.detail_items.clear();
            self.detail_preview = None;
        }
        if mode == ViewMode::TervezoDetail {
            if let Some(entry) = self.selected_session() {
                if let Some(imp) = entry.as_remote() {
                    let state = TervezoDetailState::new(imp.clone());
                    self.tervezo_detail = Some(state);
                    let (tx, rx) = mpsc::channel();
                    self.tervezo_detail_tx = Some(tx);
                    self.tervezo_detail_rx = Some(rx);
                }
            }
        } else {
            self.tervezo_detail = None;
            self.tervezo_detail_tx = None;
            self.tervezo_detail_rx = None;
            self.stop_sse_stream();
        }
        self.view_mode = mode;
    }

    pub fn tervezo_config(&self) -> Option<&TervezoConfig> {
        self.tervezo_config.as_ref()
    }

    pub fn has_tervezo(&self) -> bool {
        self.tervezo_config.is_some()
    }

    pub fn remote_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_remote()).count()
    }

    #[allow(dead_code)]
    pub fn ssh_cache(&self) -> &HashMap<String, SshCredentials> {
        &self.ssh_cache
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
        if self.view_mode == ViewMode::Detail {
            if self.detail_preview.is_some() {
                self.detail_preview_scroll = self.detail_preview_scroll.saturating_sub(1);
            } else if self.detail_cursor > 0 {
                self.detail_cursor -= 1;
            }
            return;
        }
        if self.view_mode == ViewMode::TervezoDetail {
            if let Some(ref mut state) = self.tervezo_detail {
                state.scroll_active_tab_up();
            }
            return;
        }
        if self.view_mode == ViewMode::Log {
            self.log_scroll_up();
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.view_mode == ViewMode::Detail {
            if self.detail_preview.is_some() {
                self.detail_preview_scroll += 1;
            } else if self.detail_cursor + 1 < self.detail_items.len() {
                self.detail_cursor += 1;
            }
            return;
        }
        if self.view_mode == ViewMode::TervezoDetail {
            if let Some(ref mut state) = self.tervezo_detail {
                state.scroll_active_tab_down();
            }
            return;
        }
        if self.view_mode == ViewMode::Log {
            self.log_scroll_down();
            return;
        }
        let limit = if matches!(
            self.view_mode,
            ViewMode::QSwitcher | ViewMode::TerminalQSwitcher
        ) {
            self.filtered.len().min(9)
        } else {
            self.filtered.len()
        };
        if self.selected + 1 < limit {
            self.selected += 1;
        }
    }

    pub fn move_to_top(&mut self) {
        if self.view_mode == ViewMode::Log {
            self.log_scroll_to_top();
            return;
        }
        self.selected = 0;
    }

    pub fn move_to_bottom(&mut self) {
        if self.view_mode == ViewMode::Log {
            self.log_scroll_to_bottom();
            return;
        }
        if !self.filtered.is_empty() {
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

    pub fn is_attached(&self, session_id: &str) -> bool {
        self.terminal_manager.is_attached(session_id)
    }

    pub fn has_bell(&self, session_id: &str) -> bool {
        self.terminal_manager.has_bell_for(session_id)
    }

    pub fn active_attached_sessions(&self) -> Vec<String> {
        self.local_sessions
            .iter()
            .filter(|s| {
                self.terminal_manager.is_attached(&s.id)
                    && matches!(s.status, SessionStatus::Active | SessionStatus::Thinking)
            })
            .map(|s| s.project_name.clone())
            .collect()
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

    pub fn all_sessions(&self) -> &[Session] {
        &self.local_sessions
    }

    #[allow(dead_code)]
    pub fn live_sessions(&self) -> Vec<&Session> {
        self.local_sessions
            .iter()
            .filter(|s| s.pid.is_some())
            .collect()
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
}
