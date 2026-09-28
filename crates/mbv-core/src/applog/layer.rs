//! The logfmt `Layer` (design D2): renders events as one logfmt line, with
//! each enclosing span's fields formatted once at span creation and appended
//! to when the span records fields later (`field::Empty` fills).

use std::fmt;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::Arc;
use std::sync::Mutex;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Record};
use tracing::{Event, Id, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use super::line::{self, Line};
use super::time;
use super::{format_stderr_line, sink::FileSink};

/// The fields of one span, stored in the span's extensions. Empty at creation
/// fields are absent until `Span::record` fills them (`on_record` appends).
struct SpanFields(Vec<(String, String)>);

/// Collects every recorded field of an attributes/record/event in order.
struct FieldVisitor {
    fields: Vec<(String, String)>,
    message: Option<String>,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            fields: Vec::new(),
            message: None,
        }
    }

    fn push(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = Some(value);
        } else {
            self.fields.push((field.name().to_owned(), value));
        }
    }
}

impl Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field, value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.push(field, format!("{value:?}"));
    }
}

pub(crate) struct LogfmtLayer {
    stderr: bool,
    file: Mutex<Option<FileSink>>,
    #[cfg(test)]
    capture: Option<Arc<Mutex<Vec<String>>>>,
}

impl LogfmtLayer {
    pub(crate) fn new(stderr: bool, log_path: Option<PathBuf>) -> Self {
        Self {
            stderr,
            file: Mutex::new(log_path.map(FileSink::at_default_size)),
            #[cfg(test)]
            capture: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn capturing(capture: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            stderr: false,
            file: Mutex::new(None),
            capture: Some(capture),
        }
    }

    fn write(&self, level: tracing::Level, line: &str) {
        if self.stderr {
            eprintln!("{}", format_stderr_line(level, line));
        }
        if let Ok(mut file) = self.file.lock()
            && let Some(sink) = file.as_mut()
        {
            sink.write_line(line);
        }
        #[cfg(test)]
        if let Some(capture) = &self.capture
            && let Ok(mut lines) = capture.lock()
        {
            lines.push(line.to_owned());
        }
    }
}

impl<S> Layer<S> for LogfmtLayer
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::new();
        attrs.record(&mut visitor);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(SpanFields(visitor.fields));
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut extensions = span.extensions_mut();
        if let Some(existing) = extensions.get_mut::<SpanFields>() {
            let mut visitor = FieldVisitor::new();
            values.record(&mut visitor);
            existing.0.append(&mut visitor.fields);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);
        let metadata = event.metadata();
        let level = level_name(*metadata.level());
        let ts = time::format_ts(time::now_local());
        // Without `name:`, tracing names the event after its message; only an
        // explicit name becomes the `event` field (design D4).
        let name = metadata.name();
        let event_name = (visitor.message.as_deref() != Some(name)).then_some(name);
        let spans: Vec<String> = ctx
            .event_scope(event)
            .map(|scope| {
                scope
                    .from_root()
                    .filter_map(|span| {
                        span.extensions()
                            .get::<SpanFields>()
                            .map(|fields| line::format_fields(&fields.0))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let rendered = line::format_line(&Line {
            ts: &ts,
            level,
            source: metadata.target(),
            event: event_name,
            fields: &visitor.fields,
            spans: &spans,
            message: visitor.message.as_deref().unwrap_or(""),
        });
        self.write(*metadata.level(), &rendered);
    }
}

fn level_name(level: tracing::Level) -> &'static str {
    match level {
        tracing::Level::ERROR => "error",
        tracing::Level::WARN => "warn",
        tracing::Level::INFO => "info",
        tracing::Level::DEBUG => "debug",
        tracing::Level::TRACE => "trace",
    }
}
