//! Shared capture layer and log-line capture for tests in other crates (`test` feature).

use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;

/// The real logfmt layer with `sink` receiving every rendered line, so tests
/// assert against the same format production writes.
pub fn capture_layer<S>(sink: impl Fn(&str) + Send + Sync + 'static) -> impl Layer<S>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    super::layer::LogfmtLayer::with_sink(sink)
}

/// Runs `run` under the shared logfmt capture layer and returns the rendered
/// lines (structured-logging design D5).
///
/// # Panics
///
/// Panics if the capture-line mutex is poisoned, i.e. `run` or the capture
/// layer panicked while holding it.
pub fn capture_log_lines(run: impl FnOnce()) -> Vec<String> {
    use tracing_subscriber::prelude::*;

    let lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&lines);
    let subscriber = tracing_subscriber::registry().with(capture_layer(move |line| {
        sink.lock().unwrap().push(line.to_owned());
    }));
    tracing::subscriber::with_default(subscriber, run);
    lines.lock().unwrap().clone()
}
