use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::session::{Project, Session, SessionStatus};

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

pub fn status_rank(status: &SessionStatus) -> u8 {
    match status {
        SessionStatus::Thinking => 0,
        SessionStatus::Waiting => 1,
        SessionStatus::Active => 2,
        SessionStatus::Idle => 3,
        SessionStatus::Dead => 4,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Project(usize),
    Session(usize, usize),
}

impl Row {
    pub fn project_index(self) -> usize {
        match self {
            Row::Project(p) | Row::Session(p, _) => p,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKey {
    Project(PathBuf),
    Session(String),
}

pub struct ListState {
    pub sort_column: SortColumn,
    pub filter_query: String,
    pub expanded: HashSet<PathBuf>,
    pub hidden: HashSet<String>,
    pub rows: Vec<Row>,
    pub selected: usize,
}

impl ListState {
    pub fn new(hidden: HashSet<String>) -> Self {
        Self {
            sort_column: SortColumn::LastActive,
            filter_query: String::new(),
            expanded: HashSet::new(),
            hidden,
            rows: Vec::new(),
            selected: 0,
        }
    }

    pub fn is_visible(&self, session: &Session) -> bool {
        !self.hidden.contains(&session.id)
    }

    pub fn hidden_count(&self, project: &Project) -> usize {
        project
            .sessions
            .iter()
            .filter(|s| !self.is_visible(s))
            .count()
    }

    pub fn visible_sessions<'a>(&'a self, project: &'a Project) -> Vec<&'a Session> {
        project
            .sessions
            .iter()
            .filter(|s| self.is_visible(s))
            .collect()
    }

    pub fn project_matches(&self, project: &Project) -> bool {
        let query = self.filter_query.to_lowercase();
        self.visible_sessions(project)
            .iter()
            .any(|s| s.matches_filter(&query))
    }

    pub fn is_expanded(&self, project: &Project) -> bool {
        self.expanded.contains(&project.root)
            || (!self.filter_query.is_empty() && self.project_matches(project))
    }

    pub fn sort_projects(&self, projects: &mut [Project]) {
        match self.sort_column {
            SortColumn::LastActive => {
                projects.sort_by_key(|p| std::cmp::Reverse(p.last_activity()))
            }
            SortColumn::Project => projects.sort_by(|a, b| a.name.cmp(&b.name)),
            SortColumn::Cost => projects.sort_by(|a, b| {
                b.total_cost()
                    .partial_cmp(&a.total_cost())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortColumn::Status => projects.sort_by_key(|p| {
                p.sessions
                    .iter()
                    .map(|s| status_rank(&s.status))
                    .min()
                    .unwrap_or(u8::MAX)
            }),
            SortColumn::Tokens => projects.sort_by_key(|p| std::cmp::Reverse(p.total_tokens())),
        }
    }

    pub fn rebuild_rows(&mut self, projects: &[Project]) {
        let query = self.filter_query.to_lowercase();
        let mut rows = Vec::new();
        for (pi, project) in projects.iter().enumerate() {
            let visible = self.visible_sessions(project);
            let only_hidden = visible.is_empty() && self.hidden_count(project) > 0;
            if query.is_empty() {
                if visible.is_empty() && !only_hidden {
                    continue;
                }
            } else if !self.project_matches(project) {
                continue;
            }
            rows.push(Row::Project(pi));
            if self.is_expanded(project) {
                for (si, session) in project.sessions.iter().enumerate() {
                    if self.is_visible(session) && session.matches_filter(&query) {
                        rows.push(Row::Session(pi, si));
                    }
                }
            }
        }
        self.rows = rows;
        self.clamp_selection();
    }

    pub fn clamp_selection(&mut self) {
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
    }

    pub fn selected_row(&self) -> Option<Row> {
        self.rows.get(self.selected).copied()
    }

    pub fn row_key(&self, projects: &[Project], row: Row) -> Option<RowKey> {
        match row {
            Row::Project(p) => projects.get(p).map(|pr| RowKey::Project(pr.root.clone())),
            Row::Session(p, s) => projects
                .get(p)
                .and_then(|pr| pr.sessions.get(s))
                .map(|se| RowKey::Session(se.id.clone())),
        }
    }

    pub fn select_key(&mut self, projects: &[Project], key: &RowKey) {
        if let Some(pos) = self
            .rows
            .iter()
            .position(|row| self.row_key(projects, *row).as_ref() == Some(key))
        {
            self.selected = pos;
        }
    }

    pub fn select_project_root(&mut self, projects: &[Project], root: &Path) {
        self.select_key(projects, &RowKey::Project(root.to_path_buf()));
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.rows.len() {
            self.selected += 1;
        }
    }

    pub fn move_to_top(&mut self) {
        self.selected = 0;
    }

    pub fn move_to_bottom(&mut self) {
        self.selected = self.rows.len().saturating_sub(1);
    }

    pub fn expand(&mut self, root: &Path) {
        self.expanded.insert(root.to_path_buf());
    }

    pub fn collapse(&mut self, root: &Path) {
        self.expanded.remove(root);
    }

    pub fn nth_project_row(&self, n: usize) -> Option<Row> {
        self.rows
            .iter()
            .filter(|r| matches!(r, Row::Project(_)))
            .nth(n)
            .copied()
    }
}

pub struct QuickSwitch {
    pub selected: usize,
    pub expanded: Option<PathBuf>,
}

impl QuickSwitch {
    pub fn new() -> Self {
        Self {
            selected: 0,
            expanded: None,
        }
    }

    pub fn rows(&self, list: &ListState, projects: &[Project]) -> Vec<Row> {
        let mut rows = Vec::new();
        for (pi, project) in projects.iter().enumerate() {
            if list.visible_sessions(project).is_empty() {
                continue;
            }
            rows.push(Row::Project(pi));
            if self.expanded.as_deref() == Some(project.root.as_path()) {
                for (si, session) in project.sessions.iter().enumerate() {
                    if list.is_visible(session) {
                        rows.push(Row::Session(pi, si));
                    }
                }
            }
        }
        rows
    }

    pub fn selected_row(&self, list: &ListState, projects: &[Project]) -> Option<Row> {
        self.rows(list, projects).get(self.selected).copied()
    }

    pub fn selected_key(&self, list: &ListState, projects: &[Project]) -> Option<RowKey> {
        self.selected_row(list, projects)
            .and_then(|row| list.row_key(projects, row))
    }

    pub fn select_key(&mut self, list: &ListState, projects: &[Project], key: &RowKey) {
        let rows = self.rows(list, projects);
        if let Some(pos) = rows
            .iter()
            .position(|row| list.row_key(projects, *row).as_ref() == Some(key))
        {
            self.selected = pos;
        } else {
            self.selected = self.selected.min(rows.len().saturating_sub(1));
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self, list: &ListState, projects: &[Project]) {
        let len = self.rows(list, projects).len();
        if self.selected + 1 < len {
            self.selected += 1;
        }
    }

    pub fn expand(&mut self, list: &ListState, projects: &[Project]) {
        if let Some(row) = self.selected_row(list, projects) {
            let pi = row.project_index();
            self.expanded = projects.get(pi).map(|p| p.root.clone());
            let rows = self.rows(list, projects);
            if let Some(pos) = rows.iter().position(|r| *r == Row::Project(pi)) {
                self.selected = pos;
            }
        }
    }

    pub fn collapse(&mut self, list: &ListState, projects: &[Project]) {
        if let Some(row) = self.selected_row(list, projects) {
            let pi = row.project_index();
            self.expanded = None;
            let rows = self.rows(list, projects);
            if let Some(pos) = rows.iter().position(|r| *r == Row::Project(pi)) {
                self.selected = pos;
            }
        }
    }

    pub fn nth_project(&self, list: &ListState, projects: &[Project], n: usize) -> Option<Row> {
        self.rows(list, projects)
            .into_iter()
            .filter(|r| matches!(r, Row::Project(_)))
            .nth(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::test_support::make_session;
    use crate::session::{group_projects, SessionStatus};

    fn session(id: &str, cwd: &str, title: &str, minutes_ago: i64) -> Session {
        let mut s = make_session(id, "claude-opus-5");
        s.cwd = PathBuf::from(cwd);
        s.project_name = cwd.trim_start_matches('/').to_string();
        s.title = Some(title.to_string());
        s.status = SessionStatus::Dead;
        s.last_activity = chrono::Utc::now() - chrono::Duration::minutes(minutes_ago);
        s
    }

    fn projects() -> Vec<Project> {
        group_projects(vec![
            session("a1", "/a", "alpha one", 1),
            session("a2", "/a", "alpha two", 2),
            session("b1", "/b", "beta", 10),
        ])
    }

    #[test]
    fn test_rows_collapsed_by_default_and_expand_with_key() {
        let projects = projects();
        let mut list = ListState::new(HashSet::new());
        list.rebuild_rows(&projects);
        assert_eq!(list.rows.len(), 2);
        assert!(matches!(list.rows[0], Row::Project(_)));

        let root = projects[0].root.clone();
        list.expand(&root);
        list.rebuild_rows(&projects);
        let expanded_len = 1 + projects[0].sessions.len() + 1;
        assert_eq!(list.rows.len(), expanded_len);
        assert!(matches!(list.rows[1], Row::Session(0, 0)));

        list.collapse(&root);
        list.rebuild_rows(&projects);
        assert_eq!(list.rows.len(), 2);
    }

    #[test]
    fn test_fully_hidden_project_stays_listed_with_hidden_count() {
        let projects = projects();
        let mut list = ListState::new(["b1".to_string()].into_iter().collect());
        list.rebuild_rows(&projects);
        assert_eq!(list.rows.len(), 2);
        let b = projects
            .iter()
            .position(|p| p.root.as_path() == std::path::Path::new("/b"))
            .unwrap();
        assert!(list.rows.contains(&Row::Project(b)));
        assert_eq!(list.hidden_count(&projects[b]), 1);
        assert_eq!(list.hidden_count(&projects[1 - b]), 0);

        list.expand(&projects[b].root);
        list.rebuild_rows(&projects);
        assert_eq!(list.rows.len(), 2);
        assert!(list.visible_sessions(&projects[b]).is_empty());
    }

    #[test]
    fn test_filter_drops_fully_hidden_projects() {
        let projects = projects();
        let mut list = ListState::new(["b1".to_string()].into_iter().collect());
        list.filter_query = "beta".to_string();
        list.rebuild_rows(&projects);
        assert!(list.rows.is_empty());
    }

    #[test]
    fn test_filter_auto_expands_matching_projects_only() {
        let projects = projects();
        let mut list = ListState::new(HashSet::new());
        list.filter_query = "two".to_string();
        list.rebuild_rows(&projects);
        assert_eq!(list.rows.len(), 2);
        assert!(matches!(list.rows[0], Row::Project(_)));
        match list.rows[1] {
            Row::Session(p, s) => assert_eq!(projects[p].sessions[s].id, "a2"),
            _ => panic!("expected session row"),
        }
    }

    #[test]
    fn test_select_key_and_nth_project_row() {
        let projects = projects();
        let mut list = ListState::new(HashSet::new());
        list.expand(&projects[0].root);
        list.rebuild_rows(&projects);
        list.select_key(&projects, &RowKey::Session("a2".into()));
        assert!(matches!(list.selected_row(), Some(Row::Session(0, 1))));
        assert_eq!(list.nth_project_row(1), Some(Row::Project(1)));
        assert_eq!(list.nth_project_row(5), None);
        list.move_to_bottom();
        assert_eq!(list.selected, list.rows.len() - 1);
        list.move_down();
        assert_eq!(list.selected, list.rows.len() - 1);
    }

    #[test]
    fn test_sort_by_name() {
        let mut projects = projects();
        let mut list = ListState::new(HashSet::new());
        list.sort_column = SortColumn::Project;
        projects.reverse();
        list.sort_projects(&mut projects);
        assert_eq!(projects[0].root, PathBuf::from("/a"));
    }

    #[test]
    fn test_quick_switch_expand_collapse() {
        let projects = projects();
        let list = ListState::new(HashSet::new());
        let mut qs = QuickSwitch::new();
        assert_eq!(qs.rows(&list, &projects).len(), 2);
        qs.expand(&list, &projects);
        assert_eq!(qs.rows(&list, &projects).len(), 4);
        qs.move_down(&list, &projects);
        assert!(matches!(
            qs.selected_row(&list, &projects),
            Some(Row::Session(0, 0))
        ));
        assert_eq!(qs.nth_project(&list, &projects, 1), Some(Row::Project(1)));
        qs.collapse(&list, &projects);
        assert_eq!(qs.rows(&list, &projects).len(), 2);
        assert_eq!(qs.selected, 0);
    }

    #[test]
    fn test_quick_switch_keys_survive_project_reorder() {
        let mut projects = projects();
        let list = ListState::new(HashSet::new());
        let mut qs = QuickSwitch::new();
        qs.expand(&list, &projects);
        qs.move_down(&list, &projects);
        qs.move_down(&list, &projects);
        let key = qs.selected_key(&list, &projects).unwrap();
        assert_eq!(key, RowKey::Session("a2".into()));

        projects.reverse();
        qs.select_key(&list, &projects, &key);
        assert!(matches!(
            qs.selected_row(&list, &projects),
            Some(Row::Session(1, 1))
        ));
        assert_eq!(qs.rows(&list, &projects).len(), 4);
    }

    #[test]
    fn test_list_select_key_and_rows_survive_project_reorder() {
        let mut projects = projects();
        let mut list = ListState::new(HashSet::new());
        list.expand(&projects[0].root);
        list.rebuild_rows(&projects);
        list.select_key(&projects, &RowKey::Session("a2".into()));
        let key = list
            .selected_row()
            .and_then(|row| list.row_key(&projects, row))
            .unwrap();

        projects.reverse();
        list.rebuild_rows(&projects);
        list.select_key(&projects, &key);
        let row = list.selected_row().unwrap();
        assert_eq!(
            list.row_key(&projects, row),
            Some(RowKey::Session("a2".into()))
        );
    }
}
