use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex, OnceLock};

const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    #[allow(dead_code)]
    Warn,
    Error,
}

impl LogLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
}

struct LogBuffer {
    entries: Vec<LogEntry>,
    dirty: bool,
}

fn global() -> &'static Arc<Mutex<LogBuffer>> {
    static INSTANCE: OnceLock<Arc<Mutex<LogBuffer>>> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Arc::new(Mutex::new(LogBuffer {
            entries: Vec::new(),
            dirty: false,
        }))
    })
}

pub fn push(level: LogLevel, msg: String) {
    let mut buf = global().lock().unwrap();
    buf.entries.push(LogEntry {
        timestamp: Utc::now(),
        level,
        message: msg,
    });
    if buf.entries.len() > MAX_ENTRIES {
        let excess = buf.entries.len() - MAX_ENTRIES;
        buf.entries.drain(..excess);
    }
    buf.dirty = true;
}

pub fn take_dirty() -> bool {
    let mut buf = global().lock().unwrap();
    let was = buf.dirty;
    buf.dirty = false;
    was
}

pub fn entries() -> Vec<LogEntry> {
    global().lock().unwrap().entries.clone()
}

pub fn clear() {
    let mut buf = global().lock().unwrap();
    buf.entries.clear();
    buf.dirty = true;
}

pub fn entry_count() -> usize {
    global().lock().unwrap().entries.len()
}

#[macro_export]
macro_rules! tlog {
    (info, $($arg:tt)*) => {
        $crate::log::push($crate::log::LogLevel::Info, format!($($arg)*))
    };
    (warn, $($arg:tt)*) => {
        $crate::log::push($crate::log::LogLevel::Warn, format!($($arg)*))
    };
    (error, $($arg:tt)*) => {
        $crate::log::push($crate::log::LogLevel::Error, format!($($arg)*))
    };
}
