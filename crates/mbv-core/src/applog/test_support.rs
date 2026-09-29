//! Shared capture layer for tests in other crates (`test` feature).

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
