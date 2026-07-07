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
    stderr: bool,
    file: Arc<Mutex<Option<std::fs::File>>>,
}

impl AppLog {
    fn new(stderr: bool, log_path: Option<PathBuf>) -> Self {
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

pub fn init(stderr: bool, log_path: Option<PathBuf>) {
    if GLOBAL.get().is_some() {
        return;
    }
    let applog = AppLog::new(stderr, log_path);
    GLOBAL.get_or_init(|| applog);
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Debug);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── log::Level conversion ─────────────────────────────────────────────────

    #[test]
    fn level_from_log_trace_maps_to_debug() {
        assert_eq!(Level::from(log::Level::Trace), Level::Debug);
    }

    #[test]
    fn push_entry_writes_to_file() {
        let dir = std::env::temp_dir().join(format!("mbv-applog-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.log");
        let _ = std::fs::remove_file(&path);
        let log = AppLog::new(false, Some(path.clone()));
        log.push_entry(LogEntry {
            level: Level::Info,
            ts: String::new(),
            source: "s".into(),
            msg: "hello".into(),
        });
        let contents = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(contents.contains("hello"));
        let _ = std::fs::remove_file(&path);
    }
}
