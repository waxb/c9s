use crate::session::{Session, SessionConfig, SessionDiscovery, SessionStatus};
use crate::session::config::{scan_session_config, build_config_items, ConfigItem};
use crate::store::Store;
use crate::terminal::TerminalManager;
use crate::usage::{UsageData, UsageFetcher};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Detail,
    Help,
    Filter,
    Harpoon,
    Terminal,
    TerminalHarpoon,
    Command,
    ConfirmQuit,
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

pub struct App {
    sessions: Vec<Session>,
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
}

impl App {
    pub fn new() -> Result<Self> {
        let discovery = SessionDiscovery::new();
        let store = Store::open().ok();

        let mut app = Self {
            sessions: Vec::new(),
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

        if self.selected >= self.filtered.len() && !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
        }

        self.usage = self.usage_fetcher.get().clone();

        Ok(())
    }

    pub fn usage(&self) -> &UsageData {
        &self.usage
    }

    fn apply_sort(&mut self) {
        match self.sort_column {
            SortColumn::LastActive => {
                self.sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
            }
            SortColumn::Project => {
                self.sessions.sort_by(|a, b| a.project_name.cmp(&b.project_name));
            }
            SortColumn::Cost => {
                self.sessions.sort_by(|a, b| {
                    b.estimated_cost_usd()
                        .partial_cmp(&a.estimated_cost_usd())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SortColumn::Status => {
                self.sessions.sort_by(|a, b| {
                    let rank = |s: &SessionStatus| -> u8 {
                        match s {
                            SessionStatus::Thinking => 0,
                            SessionStatus::Active => 1,
                            SessionStatus::Idle => 2,
                            SessionStatus::Dead => 3,
                        }
                    };
                    rank(&a.status).cmp(&rank(&b.status))
                });
            }
            SortColumn::Tokens => {
                self.sessions.sort_by(|a, b| b.total_tokens().cmp(&a.total_tokens()));
            }
        }
    }

    fn apply_filter(&mut self) {
        let query = self.filter_query.to_lowercase();
        self.filtered = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                if query.is_empty() {
                    return true;
                }
                s.project_name.to_lowercase().contains(&query)
                    || s.cwd.to_string_lossy().to_lowercase().contains(&query)
                    || s.git_branch
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
                    || s.model
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
                    || s.status.label().to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
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
        if matches!(mode, ViewMode::Harpoon | ViewMode::TerminalHarpoon) {
            let max = self.filtered.len().min(9);
            if self.selected >= max && max > 0 {
                self.selected = max - 1;
            }
        }
        if mode == ViewMode::Detail {
            if let Some(session) = self.selected_session() {
                let cfg = scan_session_config(&session.cwd);
                self.detail_items = build_config_items(&cfg, &session.cwd);
                self.detail_config = Some(cfg);
                self.detail_cursor = 0;
                self.detail_preview = None;
                self.detail_preview_scroll = 0;
            }
        } else {
            self.detail_config = None;
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
                let name = path.file_name()
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
        let limit = if matches!(self.view_mode, ViewMode::Harpoon | ViewMode::TerminalHarpoon) {
            self.filtered.len().min(9)
        } else {
            self.filtered.len()
        };
        if self.selected + 1 < limit {
            self.selected += 1;
        }
    }

    pub fn move_to_top(&mut self) {
        self.selected = 0;
    }

    pub fn move_to_bottom(&mut self) {
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
        self.sessions
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
        &self.sessions
    }

    #[allow(dead_code)]
    pub fn live_sessions(&self) -> Vec<&Session> {
        self.sessions
            .iter()
            .filter(|s| s.pid.is_some())
            .collect()
    }

}
