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
        if g.len() >= self.1 { g.drain(..self.1 / 10); }
        g.push_back(LogEntry { level, source, msg: msg.into() });
    }

    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.0.lock().unwrap().iter().cloned().collect()
    }
}
