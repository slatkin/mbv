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
    #[must_use]
    pub fn logfmt(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Level> {
        match value {
            "error" => Some(Level::Error),
            "warn" => Some(Level::Warn),
            "info" => Some(Level::Info),
            "debug" => Some(Level::Debug),
            _ => None,
        }
    }

    fn max_level_filter(self) -> log::LevelFilter {
        match self {
            Level::Error => log::LevelFilter::Error,
            Level::Warn => log::LevelFilter::Warn,
            Level::Info => log::LevelFilter::Info,
            Level::Debug => log::LevelFilter::Debug,
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

#[derive(Clone, Debug)]
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
    // SAFETY: `libc::tm` is a C POD struct; zero is a valid initial value for
    // passing it as the output buffer to `localtime_r`.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: `secs` and `tm` are valid pointers for the duration of the call,
    // and `localtime_r` writes the broken-down time into `tm`.
    unsafe { libc::localtime_r(&raw const secs, &raw mut tm) };
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

#[derive(Clone, Debug)]
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
            if path.metadata().map_or(0, |m| m.len()) > 1_000_000 {
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
            eprintln!("{}", format_stderr_line(entry.level, &line));
        }
        if let Ok(mut guard) = self.file.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = writeln!(f, "{line}");
            }
        }
    }
}

fn format_stderr_line(level: Level, line: &str) -> String {
    let priority = match level {
        Level::Error => 3,
        Level::Warn => 4,
        Level::Info => 6,
        Level::Debug => 7,
    };
    format!("<{priority}>{line}")
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

pub fn init(stderr: bool, log_path: Option<PathBuf>, level: Level) {
    if GLOBAL.get().is_some() {
        return;
    }
    let applog = AppLog::new(stderr, log_path);
    GLOBAL.get_or_init(|| applog);
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(level.max_level_filter());
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // ── log::Level conversion ─────────────────────────────────────────────────

    #[test]
    fn level_from_log_trace_maps_to_debug() {
        assert_eq!(Level::from(log::Level::Trace), Level::Debug);
    }

    #[test]
    fn init_at_info_disables_debug_records() {
        init(false, None, Level::Info);

        assert!(!log::log_enabled!(target: "applog_test", log::Level::Debug));
    }

    #[rstest]
    #[case(Level::Error, "<3>line")]
    #[case(Level::Warn, "<4>line")]
    #[case(Level::Info, "<6>line")]
    #[case(Level::Debug, "<7>line")]
    fn stderr_line_has_systemd_priority_prefix(#[case] level: Level, #[case] expected: &str) {
        assert_eq!(format_stderr_line(level, "line"), expected);
    }
}
