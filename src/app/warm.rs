use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub const PING_TEXT: &str =
    "[c9s keep-warm] Cache keep-warm ping from c9s. No reply or action needed.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarmEntry {
    pub enabled_at: Instant,
    pub last_ping: Option<Instant>,
    pub pings: u32,
}

pub struct KeepWarm {
    entries: HashMap<String, WarmEntry>,
    interval: Duration,
    max_age: Duration,
}

impl KeepWarm {
    pub fn new(interval: Duration, max_age: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            interval,
            max_age,
        }
    }

    pub fn toggle(&mut self, id: &str, now: Instant) -> bool {
        if self.entries.remove(id).is_some() {
            return false;
        }
        self.entries.insert(
            id.to_string(),
            WarmEntry {
                enabled_at: now,
                last_ping: None,
                pings: 0,
            },
        );
        true
    }

    pub fn disable(&mut self, id: &str) {
        self.entries.remove(id);
    }

    pub fn is_warm(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    pub fn warm_ids(&self) -> HashSet<String> {
        self.entries.keys().cloned().collect()
    }

    #[cfg(test)]
    pub fn entry(&self, id: &str) -> Option<&WarmEntry> {
        self.entries.get(id)
    }

    pub fn remaining(&self, id: &str, now: Instant) -> Option<Duration> {
        self.entries.get(id).map(|e| {
            self.max_age
                .saturating_sub(now.duration_since(e.enabled_at))
        })
    }

    pub fn expire(&mut self, now: Instant) -> Vec<String> {
        let max_age = self.max_age;
        let expired: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, e)| now.duration_since(e.enabled_at) >= max_age)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            self.entries.remove(id);
        }
        expired
    }

    pub fn due(&self, id: &str, now: Instant, idle_for: Duration) -> bool {
        let Some(entry) = self.entries.get(id) else {
            return false;
        };
        if idle_for < self.interval {
            return false;
        }
        match entry.last_ping {
            Some(last) => now.duration_since(last) >= self.interval,
            None => true,
        }
    }

    pub fn record_ping(&mut self, id: &str, now: Instant) {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.last_ping = Some(now);
            entry.pings += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warm() -> KeepWarm {
        KeepWarm::new(Duration::from_secs(55 * 60), Duration::from_secs(12 * 3600))
    }

    #[test]
    fn test_toggle_and_query() {
        let mut w = warm();
        let now = Instant::now();
        assert!(w.toggle("a", now));
        assert!(w.is_warm("a"));
        assert_eq!(w.warm_ids(), ["a".to_string()].into_iter().collect());
        assert!(!w.toggle("a", now));
        assert!(!w.is_warm("a"));
    }

    #[test]
    fn test_due_requires_idle_interval_and_spacing() {
        let mut w = warm();
        let t0 = Instant::now();
        w.toggle("a", t0);
        assert!(!w.due("a", t0, Duration::from_secs(10 * 60)));
        assert!(w.due("a", t0, Duration::from_secs(56 * 60)));
        w.record_ping("a", t0);
        assert!(!w.due(
            "a",
            t0 + Duration::from_secs(60),
            Duration::from_secs(60 * 60)
        ));
        assert!(w.due(
            "a",
            t0 + Duration::from_secs(56 * 60),
            Duration::from_secs(56 * 60)
        ));
        assert!(!w.due("missing", t0, Duration::from_secs(3600)));
        assert_eq!(w.entry("a").map(|e| e.pings), Some(1));
    }

    #[test]
    fn test_expire_after_max_age() {
        let mut w = warm();
        let t0 = Instant::now();
        w.toggle("a", t0);
        w.toggle("b", t0 + Duration::from_secs(3600));
        assert!(w.expire(t0 + Duration::from_secs(11 * 3600)).is_empty());
        let expired = w.expire(t0 + Duration::from_secs(12 * 3600));
        assert_eq!(expired, vec!["a".to_string()]);
        assert!(w.is_warm("b"));
        assert!(
            w.remaining("b", t0 + Duration::from_secs(12 * 3600))
                .unwrap()
                <= Duration::from_secs(3600)
        );
    }

    #[test]
    fn test_ping_text_is_a_single_line() {
        assert!(!PING_TEXT.contains('\n'));
        assert!(!PING_TEXT.contains('\r'));
        assert!(PING_TEXT.starts_with("[c9s keep-warm]"));
    }
}
