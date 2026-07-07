use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Error => "E",
            Level::Warn => "W",
            Level::Info => "I",
            Level::Debug => "D",
        }
    }
    pub fn logfmt(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
        }
    }
}

impl From<log::Level> for Level {
    fn from(l: log::Level) -> Self {
        match l {
            log::Level::Error => Level::Error,
            log::Level::Warn => Level::Warn,
            log::Level::Info => Level::Info,
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
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        libc::localtime_r(&secs, &mut tm);
    }
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
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
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .ok()
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
        let line = format!(
            "ts={} level={} source={} msg=\"{}\"",
            entry.ts,
            entry.level.logfmt(),
            entry.source,
            entry.msg.replace('\\', "\\\\").replace('"', "\\\"")
        );
        if self.stderr {
            eprintln!("{line}");
        }
        if let Ok(mut guard) = self.file.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = writeln!(f, "{line}");
            }
        }
        if self.capacity == 0 {
            return;
        }
        let mut g = self.buf.lock().unwrap();
        if g.len() >= self.capacity {
            g.drain(..(self.capacity / 10).max(1));
        }
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
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

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
    if GLOBAL.get().is_some() {
        return;
    }
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
        LogEntry {
            level,
            ts: String::new(),
            source: source.into(),
            msg: msg.into(),
        }
    }

    #[test]
    fn capacity_zero_drops_all_pushes() {
        let log = make(0);
        log.push_entry(entry(Level::Info, "src", "msg"));
        assert!(log.snapshot().is_empty());
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
        // `applog::global()` is a single process-wide ring buffer that every
        // test's `log::*!` calls write into concurrently (the `log` crate's
        // global logger has no per-thread/per-test scoping), so this test
        // can't assume its own entry is the *last* one in the snapshot --
        // another test can log something after this call but before the
        // snapshot is taken. Search for our own entry by its unique
        // target+message instead of relying on ordering.
        crate::applog::init(100, false, None);
        let before = crate::applog::global().unwrap().snapshot().len();
        log::info!(target: "test", "ring buffer routing test");
        let after = crate::applog::global().unwrap().snapshot().len();
        assert!(
            after > before,
            "log macro should have added an entry to the ring buffer"
        );
        let entry = crate::applog::global()
            .unwrap()
            .snapshot()
            .into_iter()
            .find(|e| e.source == "test" && e.msg == "ring buffer routing test")
            .expect("our own log entry should be present in the ring buffer snapshot");
        assert_eq!(entry.level, Level::Info);
    }
}
