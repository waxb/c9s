use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{Session, SessionStatus};

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub sessions: Vec<Session>,
    pub current: usize,
}

impl Project {
    pub fn current_session(&self) -> Option<&Session> {
        self.sessions.get(self.current)
    }

    pub fn find(&self, session_id: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == session_id)
    }

    pub fn index_of(&self, session_id: &str) -> Option<usize> {
        self.sessions.iter().position(|s| s.id == session_id)
    }

    pub fn total_cost(&self) -> f64 {
        self.sessions.iter().map(|s| s.estimated_cost_usd()).sum()
    }

    pub fn total_tokens(&self) -> u64 {
        self.sessions.iter().map(|s| s.total_tokens()).sum()
    }

    pub fn total_input_tokens(&self) -> u64 {
        self.sessions
            .iter()
            .map(|s| s.input_tokens + s.cache_read_tokens)
            .sum()
    }

    pub fn total_output_tokens(&self) -> u64 {
        self.sessions.iter().map(|s| s.output_tokens).sum()
    }

    pub fn total_messages(&self) -> u64 {
        self.sessions.iter().map(|s| s.message_count as u64).sum()
    }

    pub fn last_activity(&self) -> DateTime<Utc> {
        self.sessions
            .iter()
            .map(|s| s.last_activity)
            .max()
            .unwrap_or_default()
    }

    pub fn with_sessions(&self, sessions: Vec<Session>) -> Project {
        let current_id = self.current_session().map(|s| s.id.clone());
        let current = current_id
            .and_then(|id| sessions.iter().position(|s| s.id == id))
            .unwrap_or(0);
        Project {
            root: self.root.clone(),
            name: self.name.clone(),
            sessions,
            current,
        }
    }
}

fn is_live(session: &Session) -> bool {
    session.status != SessionStatus::Dead
}

pub fn order_sessions(sessions: &mut [Session]) {
    sessions.sort_by(|a, b| {
        is_live(b)
            .cmp(&is_live(a))
            .then_with(|| b.last_activity.cmp(&a.last_activity))
    });
}

pub fn group_projects(sessions: Vec<Session>) -> Vec<Project> {
    let mut by_root: BTreeMap<PathBuf, Vec<Session>> = BTreeMap::new();
    for session in sessions {
        by_root
            .entry(session.cwd.clone())
            .or_default()
            .push(session);
    }

    let mut projects: Vec<Project> = by_root
        .into_iter()
        .map(|(root, mut sessions)| {
            order_sessions(&mut sessions);
            let name = sessions
                .first()
                .map(|s| s.project_name.clone())
                .unwrap_or_else(|| project_name_for(&root));
            Project {
                root,
                name,
                sessions,
                current: 0,
            }
        })
        .collect();

    projects.sort_by_key(|p| std::cmp::Reverse(p.last_activity()));
    projects
}

pub fn project_name_for(root: &Path) -> String {
    let text = root.to_string_lossy();
    let parts: Vec<&str> = text.trim_end_matches('/').rsplit('/').collect();
    match parts.len() {
        0 => text.to_string(),
        1 => parts[0].to_string(),
        _ => format!("{}/{}", parts[1], parts[0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::test_support::make_session;

    fn session(id: &str, cwd: &str, live: bool, minutes_ago: i64) -> Session {
        let mut s = make_session(id, "claude-opus-5");
        s.cwd = PathBuf::from(cwd);
        s.project_name = project_name_for(Path::new(cwd));
        s.status = if live {
            SessionStatus::Idle
        } else {
            SessionStatus::Dead
        };
        s.last_activity = Utc::now() - chrono::Duration::minutes(minutes_ago);
        s
    }

    #[test]
    fn test_groups_by_root_and_orders_live_first_then_newest() {
        let projects = group_projects(vec![
            session("a-old-live", "/p/a", true, 60),
            session("a-new-dead", "/p/a", false, 1),
            session("a-mid-dead", "/p/a", false, 30),
            session("b-only", "/p/b", false, 5),
        ]);
        assert_eq!(projects.len(), 2);
        let a = projects
            .iter()
            .find(|p| p.root == Path::new("/p/a"))
            .unwrap();
        let ids: Vec<&str> = a.sessions.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["a-old-live", "a-new-dead", "a-mid-dead"]);
        assert_eq!(a.current, 0);
        assert_eq!(a.name, "p/a");
    }

    #[test]
    fn test_projects_sorted_by_most_recent_activity() {
        let projects = group_projects(vec![
            session("x", "/p/x", false, 100),
            session("y", "/p/y", false, 2),
            session("x2", "/p/x", false, 1),
        ]);
        let roots: Vec<&Path> = projects.iter().map(|p| p.root.as_path()).collect();
        assert_eq!(roots, vec![Path::new("/p/x"), Path::new("/p/y")]);
    }

    #[test]
    fn test_with_sessions_keeps_current_by_id() {
        let mut p = group_projects(vec![
            session("one", "/p", false, 3),
            session("two", "/p", false, 1),
        ])
        .remove(0);
        p.current = p.index_of("one").unwrap();
        let reordered = p.with_sessions(vec![session("one", "/p", true, 3)]);
        assert_eq!(reordered.current_session().unwrap().id, "one");
        let gone = p.with_sessions(vec![session("two", "/p", false, 1)]);
        assert_eq!(gone.current, 0);
    }

    #[test]
    fn test_totals() {
        let mut a = session("a", "/p", false, 1);
        a.input_tokens = 10;
        a.output_tokens = 5;
        a.message_count = 3;
        let mut b = session("b", "/p", false, 2);
        b.input_tokens = 20;
        b.cache_read_tokens = 7;
        b.message_count = 4;
        let p = group_projects(vec![a, b]).remove(0);
        assert_eq!(p.total_input_tokens(), 37);
        assert_eq!(p.total_output_tokens(), 5);
        assert_eq!(p.total_messages(), 7);
        assert_eq!(p.total_tokens(), 42);
    }

    #[test]
    fn test_project_name_for() {
        assert_eq!(
            project_name_for(Path::new("/Users/me/Documents/c9s")),
            "Documents/c9s"
        );
        assert_eq!(project_name_for(Path::new("/c9s")), "/c9s");
    }
}
