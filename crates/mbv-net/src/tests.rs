//! HTTP logging contract (design.md "Tests"): an agent built from
//! `agent_config` logs `http.request.done`/`http.request.failed` inside an
//! `http.request` span carrying `service`, the method and the path.

use std::fmt;
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::{Event, Id, Subscriber};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, layer::Context};

use crate::HttpService;
use crate::mock_http::MockHttp;

/// The fields of one span, stored in the span's extensions.
#[derive(Clone)]
struct SpanFields(Vec<String>);

/// Collects field fragments like `service=emby` in declaration order,
/// skipping the event's `message` pseudo-field.
struct FieldVisitor(Vec<String>);

impl Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() != "message" {
            self.0.push(format!("{}={}", field.name(), value));
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() != "message" {
            self.0.push(format!("{}={:?}", field.name(), value));
        }
    }
}

/// Captures every event as `<event name> | <fields> | <span fields…>`, plus
/// each created span's `<name>=<target>`.
struct Capture {
    lines: Mutex<Vec<String>>,
    span_targets: Mutex<Vec<String>>,
}

struct CaptureLayer(Arc<Capture>);

impl<S> Layer<S> for CaptureLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor(Vec::new());
        attrs.record(&mut visitor);
        if let Some(span) = ctx.span(id) {
            self.0
                .span_targets
                .lock()
                .expect("capture lock")
                .push(format!("{}={}", span.name(), span.metadata().target()));
            span.extensions_mut().insert(SpanFields(visitor.0));
        }
    }

    fn on_record(&self, id: &Id, values: &tracing::span::Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut visitor = FieldVisitor(Vec::new());
        values.record(&mut visitor);
        if let Some(fields) = span.extensions_mut().get_mut::<SpanFields>() {
            fields.0.append(&mut visitor.0);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor(Vec::new());
        event.record(&mut visitor);
        let span_fields: Vec<String> = ctx
            .event_scope(event)
            .map(|scope| {
                scope
                    .from_root()
                    .filter_map(|span| span.extensions().get::<SpanFields>().cloned())
                    .flat_map(|span| span.0)
                    .collect()
            })
            .unwrap_or_default();
        let line = format!(
            "{} | {} | {}",
            event.metadata().name(),
            visitor.0.join(" "),
            span_fields.join(" ")
        );
        self.0.lines.lock().expect("capture lock").push(line);
    }
}

fn captured(capture: &Arc<Capture>, f: impl FnOnce()) -> Vec<String> {
    let subscriber =
        tracing_subscriber::Registry::default().with(CaptureLayer(Arc::clone(capture)));
    tracing::subscriber::with_default(subscriber, f);
    capture.lines.lock().expect("capture lock").clone()
}

#[test]
fn failed_request_logs_service_and_status() {
    let capture = Arc::new(Capture {
        lines: Mutex::new(Vec::new()),
        span_targets: Mutex::new(Vec::new()),
    });
    let mock = MockHttp::new();
    mock.respond(500, "{}");

    let agent = mock.agent_for(HttpService::Emby);
    let lines = captured(&capture, || {
        let error = agent
            .get("http://127.0.0.1:1/Items?api_key=secret")
            .call()
            .expect_err("500 must surface as a status-code error");
        assert!(matches!(error, ureq::Error::StatusCode(500)), "{error}");
    });

    let failed: Vec<&String> = lines
        .iter()
        .filter(|line| line.starts_with("http.request.failed |"))
        .collect();
    let failed = failed
        .first()
        .unwrap_or_else(|| panic!("no failed line in {lines:?}"));
    assert!(failed.contains("service=emby"), "{failed}");
    assert!(failed.contains("http.response.status_code=500"), "{failed}");
    assert!(failed.contains("http.request.method=GET"), "{failed}");
    // Path only, never the query string: the api_key must not appear.
    assert!(failed.contains("url.path=/Items"), "{failed}");
    assert!(!failed.contains("api_key"), "{failed}");
    assert!(failed.contains("duration_ms="), "{failed}");
    // Span and events share the `http` target, so one LogSpec directive
    // (`http=…`) gates the span and its outcome events alike — if the span
    // were gated out while the events pass, the lines would lose every
    // correlation field.
    assert!(
        capture
            .span_targets
            .lock()
            .expect("capture lock")
            .iter()
            .any(|span| span == "http.request=http"),
        "{:?}",
        capture.span_targets.lock().expect("capture lock")
    );
}
