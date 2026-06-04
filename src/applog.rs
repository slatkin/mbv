use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level { Debug, Info, Warn, Error }

impl Level {
    pub fn label(self) -> &'static str {
        match self { Level::Error => "E", Level::Warn => "W", Level::Info => "I", Level::Debug => "D" }
    }
}

#[derive(Clone)]
pub struct LogEntry {
    pub level: Level,
    pub source: &'static str,
    pub msg: String,
}

#[derive(Clone)]
pub struct AppLog(Arc<Mutex<VecDeque<LogEntry>>>, usize);

impl AppLog {
    pub fn new(capacity: usize) -> Self {
        AppLog(Arc::new(Mutex::new(VecDeque::new())), capacity)
    }

    pub fn push(&self, level: Level, source: &'static str, msg: impl Into<String>) {
        if self.1 == 0 { return; }
        let mut g = self.0.lock().unwrap();
        if g.len() >= self.1 { g.drain(..(self.1 / 10).max(1)); }
        g.push_back(LogEntry { level, source, msg: msg.into() });
    }

    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.0.lock().unwrap().iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_labels() {
        assert_eq!(Level::Error.label(), "E");
        assert_eq!(Level::Warn.label(),  "W");
        assert_eq!(Level::Info.label(),  "I");
        assert_eq!(Level::Debug.label(), "D");
    }

    #[test]
    fn capacity_zero_drops_all_pushes() {
        let log = AppLog::new(0);
        log.push(Level::Info, "src", "msg");
        assert!(log.snapshot().is_empty());
    }

    #[test]
    fn push_adds_entry_visible_in_snapshot() {
        let log = AppLog::new(10);
        log.push(Level::Warn, "ws", "hello");
        let snap = log.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].level, Level::Warn);
        assert_eq!(snap[0].source, "ws");
        assert_eq!(snap[0].msg, "hello");
    }

    #[test]
    fn capacity_one_never_exceeds_one_entry() {
        let log = AppLog::new(1);
        log.push(Level::Info, "s", "a");
        log.push(Level::Info, "s", "b");
        log.push(Level::Info, "s", "c");
        let snap = log.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].msg, "c");
    }

    #[test]
    fn capacity_respected_drains_ten_percent() {
        let log = AppLog::new(10);
        for i in 0..10 {
            log.push(Level::Info, "s", i.to_string());
        }
        assert_eq!(log.snapshot().len(), 10);
        // 11th push: drain 1 (10% of 10 = 1), then add → stays at 10
        log.push(Level::Info, "s", "10");
        assert_eq!(log.snapshot().len(), 10);
        // oldest entry ("0") was evicted
        assert_eq!(log.snapshot()[0].msg, "1");
    }

    #[test]
    fn clone_shares_underlying_storage() {
        let log = AppLog::new(10);
        let clone = log.clone();
        log.push(Level::Debug, "s", "shared");
        assert_eq!(clone.snapshot().len(), 1);
        assert_eq!(clone.snapshot()[0].msg, "shared");
    }
}
