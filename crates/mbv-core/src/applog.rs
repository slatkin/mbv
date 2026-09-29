//! Application logging entry point: one `tracing` dispatcher with a logfmt
//! layer, a `LogSpec` filter (design D3), and the `log` → `tracing` bridge for
//! crates not yet converted (design D1).

pub mod sink;
pub mod spec;

mod layer;
mod line;
mod time;

use std::path::PathBuf;
use std::sync::OnceLock;
use tracing_subscriber::prelude::*;

pub use spec::LogSpec;

static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Installs the global dispatcher, the log bridge, the file sink (running the
/// startup rotation check) and, when `stderr` is set, the `<prio>` stderr sink.
/// A second call is a no-op, as is an already-installed bridge.
///
/// # Panics
///
/// Panics if a global subscriber was already installed by something other than
/// this function.
pub fn init(stderr: bool, log_path: Option<PathBuf>, spec: &LogSpec) {
    if INITIALIZED.set(()).is_err() {
        return;
    }
    let subscriber = layer::LogfmtLayer::new(stderr, log_path)
        .with_filter(spec.clone())
        .with_subscriber(tracing_subscriber::Registry::default());
    tracing::subscriber::set_global_default(subscriber)
        .expect("applog::init should install the first global subscriber");
    let _ = tracing_log::LogTracer::init();
    log::set_max_level(spec_max_level(spec));
}

/// Wraps a thread body so the spawned thread logs under the spawning
/// operation's span and dispatcher (design D5). Use for threads whose whole
/// life belongs to the spawning operation.
pub fn carry_context<F>(f: F) -> impl FnOnce() + Send + 'static
where
    F: FnOnce() + Send + 'static,
{
    let span = tracing::Span::current();
    let dispatcher = tracing::dispatcher::get_default(Clone::clone);
    move || {
        tracing::dispatcher::with_default(&dispatcher, || {
            let _entered = span.enter();
            f();
        });
    }
}

/// Wraps a thread body so the spawned thread logs through the spawning
/// thread's dispatcher but starts with no span (design D5). Use for threads
/// that outlive the operation that spawned them.
pub fn carry_dispatcher<F>(f: F) -> impl FnOnce() + Send + 'static
where
    F: FnOnce() + Send + 'static,
{
    let dispatcher = tracing::dispatcher::get_default(Clone::clone);
    move || tracing::dispatcher::with_default(&dispatcher, f)
}

pub(crate) fn format_stderr_line(level: tracing::Level, line: &str) -> String {
    let priority = match level {
        tracing::Level::ERROR => 3,
        tracing::Level::WARN => 4,
        tracing::Level::INFO => 6,
        tracing::Level::DEBUG | tracing::Level::TRACE => 7,
    };
    format!("<{priority}>{line}")
}

fn spec_max_level(spec: &LogSpec) -> log::LevelFilter {
    let most_verbose = spec
        .directives
        .iter()
        .map(|(_, level)| *level)
        .chain(std::iter::once(spec.default))
        .max()
        .unwrap_or(tracing::level_filters::LevelFilter::OFF);
    most_verbose
        .to_string()
        .parse()
        .unwrap_or(log::LevelFilter::Off)
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Log as _;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::Registry;

    /// Test subscriber around the real layer with an in-memory sink.
    fn capturing_subscriber() -> (impl tracing::Subscriber, Arc<Mutex<Vec<String>>>) {
        let capture = Arc::new(Mutex::new(Vec::new()));
        let subscriber =
            Registry::default().with(layer::LogfmtLayer::capturing(Arc::clone(&capture)));
        (subscriber, capture)
    }

    fn captured_lines(capture: &Mutex<Vec<String>>) -> Vec<String> {
        capture.lock().expect("capture lock").clone()
    }

    /// Test subscriber with the real layer under a `LogSpec` filter.
    fn filtered_capturing_subscriber(
        spec: &LogSpec,
    ) -> (impl tracing::Subscriber, Arc<Mutex<Vec<String>>>) {
        let capture = Arc::new(Mutex::new(Vec::new()));
        let subscriber = layer::LogfmtLayer::capturing(Arc::clone(&capture))
            .with_filter(spec.clone())
            .with_subscriber(Registry::default());
        (subscriber, capture)
    }

    // ── unnamed and bridged events ────────────────────────────────────────────────

    #[test]
    fn unnamed_event_has_no_event_field() {
        let (subscriber, capture) = capturing_subscriber();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "test", "plain message");
        });

        let lines = captured_lines(&capture);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("source=test"));
        assert!(lines[0].contains("msg=\"plain message\""));
        assert!(!lines[0].contains("event="));
    }

    // `tracing-log` dispatches every `log` record through one static "log
    // event" callsite with target "log"; the line must carry the record's
    // real `log.target` as `source` and none of the synthetic `log.*` fields.
    #[test]
    fn bridged_log_record_uses_log_target_as_source() {
        let (subscriber, capture) = capturing_subscriber();
        tracing::subscriber::with_default(subscriber, || {
            let record = log::Record::builder()
                .level(log::Level::Warn)
                .target("ureq")
                .module_path(Some("ureq::agent"))
                .args(format_args!("connection failed"))
                .build();
            tracing_log::LogTracer::default().log(&record);
        });

        let lines = captured_lines(&capture);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("level=warn source=ureq"));
        assert!(lines[0].contains("msg=\"connection failed\""));
        assert!(!lines[0].contains("event="));
        assert!(!lines[0].contains("log."));
    }

    // ── filter: spans are not third-party capped ─────────────────────────────

    #[test]
    fn module_path_span_is_created_while_module_path_event_stays_capped() {
        let spec = LogSpec::parse("info").expect("valid spec");
        let (subscriber, capture) = filtered_capturing_subscriber(&spec);
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(target: "mbv_daemon::control", "correlation", slot = 12);
            let _entered = span.enter();
            tracing::info!(target: "mbv_daemon::control", "capped noise");
            tracing::info!(name: "app.done", target: "app", "done");
        });

        let lines = captured_lines(&capture);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("source=app event=app.done"));
        assert!(lines[0].contains("slot=12"));
    }

    // ── init and the log bridge ───────────────────────────────────────────────

    // nextest runs each test in its own process, so the global state `init`
    // sets is not shared with other tests.
    #[test]
    fn init_at_info_disables_debug_records() {
        let spec = LogSpec::parse("info").expect("valid spec");
        init(false, None, &spec);

        assert!(!log::log_enabled!(target: "applog_test", log::Level::Debug));
        assert!(log::log_enabled!(target: "applog_test", log::Level::Warn));
    }

    // `log::set_max_level` is global, so it must be the spec's most verbose
    // level; the per-target narrowing is the filter's job.
    #[test]
    fn init_sets_bridge_max_level_from_most_verbose_directive() {
        let spec = LogSpec::parse("info,player=debug").expect("valid spec");
        init(false, None, &spec);

        assert_eq!(log::max_level(), log::LevelFilter::Debug);
    }

    // ── stderr priority prefix ────────────────────────────────────────────────

    #[rstest::rstest]
    #[case(tracing::Level::ERROR, "<3>line")]
    #[case(tracing::Level::WARN, "<4>line")]
    #[case(tracing::Level::INFO, "<6>line")]
    #[case(tracing::Level::DEBUG, "<7>line")]
    #[case(tracing::Level::TRACE, "<7>line")]
    fn stderr_line_has_systemd_priority_prefix(
        #[case] level: tracing::Level,
        #[case] expected: &str,
    ) {
        assert_eq!(format_stderr_line(level, "line"), expected);
    }

    // ── span inheritance ──────────────────────────────────────────────────────

    #[test]
    fn event_inside_span_carries_span_fields() {
        let (subscriber, capture) = capturing_subscriber();
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("playback", slot = 12, item = "abc");
            let _entered = span.enter();
            tracing::info!(name: "player.load.failed", target: "player", "load failed");
        });

        let lines = captured_lines(&capture);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("source=player event=player.load.failed"));
        assert!(lines[0].contains("slot=12 item=abc"));
        assert!(lines[0].contains("msg=\"load failed\""));
    }

    // A `field::Empty` filled after span creation (`Span::record`) must reach
    // the rendered line via the layer's `on_record` append.
    #[test]
    fn field_recorded_after_span_creation_reaches_events() {
        let (subscriber, capture) = capturing_subscriber();
        tracing::subscriber::with_default(subscriber, || {
            let span =
                tracing::info_span!("playback", slot = 12, play_session = tracing::field::Empty);
            let _entered = span.enter();

            tracing::info!(name: "player.started", target: "player", "started");

            span.record("play_session", "session-1");
            tracing::info!(name: "player.started", target: "player", "started");
        });
        let lines = captured_lines(&capture);
        assert_eq!(lines.len(), 2);
        assert!(!lines[0].contains("play_session"));
        assert!(lines[0].contains("slot=12"));
        assert!(lines[1].contains("slot=12 play_session=session-1"));
    }

    #[test]
    fn nested_spans_render_outermost_to_innermost() {
        let (subscriber, capture) = capturing_subscriber();
        tracing::subscriber::with_default(subscriber, || {
            let outer = tracing::info_span!("op", outer_field = "o");
            let _outer = outer.enter();
            let inner = tracing::info_span!("step", inner_field = "i");
            let _inner = inner.enter();
            tracing::info!(name: "test.done", target: "test", "done");
        });

        let lines = captured_lines(&capture);
        assert_eq!(lines.len(), 1);
        let position = |needle: &str| lines[0].find(needle).expect("field present");
        assert!(position("outer_field=o") < position("inner_field=i"));
    }

    // ── thread handoff (design D5) ────────────────────────────────────────────

    #[test]
    fn carry_context_carries_span_and_dispatcher_to_spawned_thread() {
        let (subscriber, capture) = capturing_subscriber();
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("playback", slot = 12);
            let _entered = span.enter();
            let handle = std::thread::spawn(carry_context(|| {
                tracing::info!(name: "test.done", target: "test", "done");
            }));
            handle.join().expect("thread should not panic");
        });

        let lines = captured_lines(&capture);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("slot=12"));
        assert!(lines[0].contains("msg=done"));
    }

    #[test]
    fn carry_dispatcher_carries_dispatcher_but_no_span_to_spawned_thread() {
        let (subscriber, capture) = capturing_subscriber();
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("playback", slot = 12);
            let _entered = span.enter();
            let handle = std::thread::spawn(carry_dispatcher(|| {
                tracing::info!(name: "test.done", target: "test", "done");
            }));
            handle.join().expect("thread should not panic");
        });

        let lines = captured_lines(&capture);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("msg=done"));
        assert!(!lines[0].contains("slot="));
    }
}
