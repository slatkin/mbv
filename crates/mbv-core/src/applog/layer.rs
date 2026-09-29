//! The logfmt `Layer` (design D2): renders events as one logfmt line, with
//! each enclosing span's fields formatted once at span creation and appended
//! to when the span records fields later (`field::Empty` fills).

use std::fmt;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Record};
use tracing::{Event, Id, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use super::line::{self, Line};
use super::time;
use super::{format_stderr_line, sink::FileSink};

/// The pre-rendered logfmt fields of one span, stored in the span's
/// extensions. Empty at creation fields are absent until `Span::record` fills
/// them (`on_record` appends).
struct SpanFields(String);

/// Collects every recorded field of an attributes/record/event in order.
struct FieldVisitor {
    fields: Vec<(String, String)>,
    message: Option<String>,
    /// Set for records bridged from `log` by `tracing-log`; the bridge
    /// dispatches every record through one static "log event" callsite with
    /// target "log", and carries the real target in the `log.target` field.
    log_target: Option<String>,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            fields: Vec::new(),
            message: None,
            log_target: None,
        }
    }

    fn push(&mut self, field: &Field, value: String) {
        match field.name() {
            "message" => self.message = Some(value),
            "log.target" => self.log_target = Some(value),
            "error" => self.fields.push(("error.message".to_owned(), value)),
            // The bridge's synthetic `log.*` context fields never render.
            name if name.starts_with("log.") => {}
            name => self.fields.push((name.to_owned(), value)),
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

#[cfg(any(test, feature = "test"))]
type Tap = Box<dyn Fn(&str) + Send + Sync>;

pub(crate) struct LogfmtLayer {
    stderr: bool,
    file: Mutex<Option<FileSink>>,
    #[cfg(any(test, feature = "test"))]
    tap: Option<Tap>,
}

impl LogfmtLayer {
    pub(crate) fn new(stderr: bool, log_path: Option<PathBuf>) -> Self {
        Self {
            stderr,
            file: Mutex::new(log_path.map(FileSink::at_default_size)),
            #[cfg(any(test, feature = "test"))]
            tap: None,
        }
    }

    #[cfg(any(test, feature = "test"))]
    pub(crate) fn with_sink(tap: impl Fn(&str) + Send + Sync + 'static) -> Self {
        Self {
            stderr: false,
            file: Mutex::new(None),
            tap: Some(Box::new(tap)),
        }
    }

    #[cfg(test)]
    pub(crate) fn capturing(capture: std::sync::Arc<Mutex<Vec<String>>>) -> Self {
        Self::with_sink(move |line| {
            if let Ok(mut lines) = capture.lock() {
                lines.push(line.to_owned());
            }
        })
    }

    fn write(&self, level: tracing::Level, line: &str) {
        if self.stderr {
            eprintln!("{}", format_stderr_line(level, line));
        }
        // A poisoned sink lock drops this line rather than risking recursive
        // logging or panicking from the logging path.
        if let Ok(mut file) = self.file.lock()
            && let Some(sink) = file.as_mut()
        {
            sink.write_line(line);
        }
        #[cfg(any(test, feature = "test"))]
        if let Some(tap) = &self.tap {
            tap(line);
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
            span.extensions_mut()
                .insert(SpanFields(line::format_fields(&visitor.fields)));
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
            let appended = line::format_fields(&visitor.fields);
            if !existing.0.is_empty() && !appended.is_empty() {
                existing.0.push(' ');
            }
            existing.0.push_str(&appended);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);
        let metadata = event.metadata();
        let ts = time::format_ts(time::now_local());
        let source = visitor.log_target.as_deref().unwrap_or(metadata.target());
        // Without `name:`, tracing names the event `event <file>:<line>`
        // (tracing 0.1.44's synthesized default); only an explicit name
        // becomes the `event` field (design D4).
        let is_default_name = metadata
            .name()
            .strip_prefix("event ")
            .and_then(|rest| rest.rsplit_once(':'))
            .is_some_and(|(file, line)| {
                file == metadata.file().unwrap_or_default()
                    && line.parse().ok() == Some(metadata.line().unwrap_or_default())
            });
        let event_name =
            (visitor.log_target.is_none() && !is_default_name).then_some(metadata.name());
        let spans: Vec<String> = ctx
            .event_scope(event)
            .map(|scope| {
                scope
                    .from_root()
                    .filter_map(|span| span.extensions().get::<SpanFields>().map(|f| f.0.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let rendered = line::format_line(&Line {
            ts: &ts,
            level: *metadata.level(),
            source,
            event: event_name,
            fields: &visitor.fields,
            spans: &spans,
            message: visitor.message.as_deref().unwrap_or(""),
        });
        self.write(*metadata.level(), &rendered);
    }
}
