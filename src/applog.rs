use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level { Debug, Info, Warn, Error }

impl Level {
    pub fn label(self) -> &'static str {
        match self { Level::Error => "E", Level::Warn => "W", Level::Info => "I", Level::Debug => "D" }
    }
    pub fn logfmt(self) -> &'static str {
        match self { Level::Error => "error", Level::Warn => "warn", Level::Info => "info", Level::Debug => "debug" }
    }
}

impl From<log::Level> for Level {
    fn from(l: log::Level) -> Self {
        match l {
            log::Level::Error => Level::Error,
            log::Level::Warn  => Level::Warn,
            log::Level::Info  => Level::Info,
            log::Level::Debug | log::Level::Trace => Level::Debug,
        }
    }
}

#[derive(Clone)]
pub struct LogEntry {
    pub level: Level,
    pub ts: String,
    pub source: String,
    pub msg: String,
}

fn now_ts() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let s = secs % 86400;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

#[derive(Clone)]
pub struct AppLog {
    buf: Arc<Mutex<VecDeque<LogEntry>>>,
    capacity: usize,
    stderr: bool,
    file: Arc<Mutex<Option<std::fs::File>>>,
}

impl AppLog {
    fn new(capacity: usize, stderr: bool, log_path: Option<PathBuf>) -> Self {
        let file = log_path.and_then(|path| {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // rotate if > 1 MB
            if path.metadata().map(|m| m.len()).unwrap_or(0) > 1_000_000 {
                let mut old = path.clone();
                old.set_extension("log.old");
                let _ = std::fs::rename(&path, &old);
            }
            std::fs::OpenOptions::new().create(true).append(true).open(&path).ok()
        });
        AppLog {
            buf: Arc::new(Mutex::new(VecDeque::new())),
            capacity,
            stderr,
            file: Arc::new(Mutex::new(file)),
        }
    }

    fn push_entry(&self, mut entry: LogEntry) {
        entry.ts = now_ts();
        let line = format!("ts={} level={} source={} msg=\"{}\"",
            entry.ts, entry.level.logfmt(), entry.source,
            entry.msg.replace('\\', "\\\\").replace('"', "\\\""));
        if self.stderr {
            eprintln!("{line}");
        }
        if let Ok(mut guard) = self.file.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = writeln!(f, "{line}");
            }
        }
        if self.capacity == 0 { return; }
        let mut g = self.buf.lock().unwrap();
        if g.len() >= self.capacity { g.drain(..(self.capacity / 10).max(1)); }
        g.push_back(entry);
    }

    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.buf.lock().unwrap().iter().cloned().collect()
    }
}

static GLOBAL: OnceLock<AppLog> = OnceLock::new();
static LOGGER: GlobalLogger = GlobalLogger;

struct GlobalLogger;

impl log::Log for GlobalLogger {
    fn enabled(&self, _: &log::Metadata) -> bool { true }

    fn log(&self, record: &log::Record) {
        // mbv targets are bare words ("api", "ws", "img", etc.) with no "::".
        // Third-party crates use module paths ("rustls::client", etc.) — suppress
        // them everywhere below Warn level.
        if record.target().contains("::") && record.level() > log::Level::Warn {
            return;
        }
        if let Some(log) = GLOBAL.get() {
            log.push_entry(LogEntry {
                level: record.level().into(),
                ts: String::new(),
                source: record.target().to_string(),
                msg: record.args().to_string(),
            });
        }
    }

    fn flush(&self) {}
}

pub fn init(capacity: usize, stderr: bool, log_path: Option<PathBuf>) {
    if GLOBAL.get().is_some() { return; }
    let applog = AppLog::new(capacity, stderr, log_path);
    GLOBAL.get_or_init(|| applog);
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Debug);
}

/// Returns the global ring buffer for the TUI Log tab.
pub fn global() -> Option<&'static AppLog> {
    GLOBAL.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(capacity: usize) -> AppLog {
        AppLog::new(capacity, false, None)
    }

    fn entry(level: Level, source: &str, msg: &str) -> LogEntry {
        LogEntry { level, ts: String::new(), source: source.into(), msg: msg.into() }
    }

    #[test]
    fn level_labels() {
        assert_eq!(Level::Error.label(), "E");
        assert_eq!(Level::Warn.label(),  "W");
        assert_eq!(Level::Info.label(),  "I");
        assert_eq!(Level::Debug.label(), "D");
    }

    #[test]
    fn capacity_zero_drops_all_pushes() {
        let log = make(0);
        log.push_entry(entry(Level::Info, "src", "msg"));
        assert!(log.snapshot().is_empty());
    }

    #[test]
    fn push_adds_entry_visible_in_snapshot() {
        let log = make(10);
        log.push_entry(entry(Level::Warn, "ws", "hello"));
        let snap = log.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].level, Level::Warn);
        assert_eq!(snap[0].source, "ws");
        assert_eq!(snap[0].msg, "hello");
    }

    #[test]
    fn capacity_respected_drains_ten_percent() {
        let log = make(10);
        for i in 0..10 {
            log.push_entry(entry(Level::Info, "s", &i.to_string()));
        }
        assert_eq!(log.snapshot().len(), 10);
        log.push_entry(entry(Level::Info, "s", "10"));
        assert_eq!(log.snapshot().len(), 10);
        assert_eq!(log.snapshot()[0].msg, "1");
    }

    #[test]
    fn clone_shares_underlying_storage() {
        let log = make(10);
        let clone = log.clone();
        log.push_entry(entry(Level::Debug, "s", "shared"));
        assert_eq!(clone.snapshot().len(), 1);
        assert_eq!(clone.snapshot()[0].msg, "shared");
    }

    // ── log::Level conversion ─────────────────────────────────────────────────

    #[test]
    fn level_from_log_error() {
        assert_eq!(Level::from(log::Level::Error), Level::Error);
    }

    #[test]
    fn level_from_log_warn() {
        assert_eq!(Level::from(log::Level::Warn), Level::Warn);
    }

    #[test]
    fn level_from_log_info() {
        assert_eq!(Level::from(log::Level::Info), Level::Info);
    }

    #[test]
    fn level_from_log_debug() {
        assert_eq!(Level::from(log::Level::Debug), Level::Debug);
    }

    #[test]
    fn level_from_log_trace_maps_to_debug() {
        assert_eq!(Level::from(log::Level::Trace), Level::Debug);
    }

    // ── global ring buffer via log macros ─────────────────────────────────────
    // init() uses a OnceLock so it can only succeed once per process.
    // This test exercises the path where the global is already initialized
    // (by a prior call in the same process) and verifies global() returns Some.

    #[test]
    fn global_returns_some_after_init() {
        // init() is idempotent via OnceLock; if already called, this is a no-op.
        crate::applog::init(100, false, None);
        assert!(crate::applog::global().is_some());
    }

    #[test]
    fn log_macro_routes_to_ring_buffer() {
        crate::applog::init(100, false, None);
        let before = crate::applog::global().unwrap().snapshot().len();
        log::info!(target: "test", "ring buffer routing test");
        let after = crate::applog::global().unwrap().snapshot().len();
        assert!(after > before, "log macro should have added an entry to the ring buffer");
        let last = crate::applog::global().unwrap().snapshot().into_iter().last().unwrap();
        assert_eq!(last.source, "test");
        assert_eq!(last.msg, "ring buffer routing test");
        assert_eq!(last.level, Level::Info);
    }
}
