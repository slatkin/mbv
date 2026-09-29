//! HTTP logging contract (design.md "Tests"): an agent built from
//! `agent_config` logs `http.request.done`/`http.request.failed` inside an
//! `http.request` span carrying `service`, the method and the path.

use std::sync::{Arc, Mutex};

use tracing_subscriber::prelude::*;

use crate::HttpService;
use crate::mock_http::MockHttp;

fn captured(f: impl FnOnce()) -> Vec<String> {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&lines);
    let subscriber = tracing_subscriber::Registry::default().with(
        mbv_core::applog::test_support::capture_layer(move |line| {
            sink.lock().expect("capture lock").push(line.to_owned());
        }),
    );
    tracing::subscriber::with_default(subscriber, f);
    lines.lock().expect("capture lock").clone()
}

#[test]
fn failed_request_logs_service_and_status() {
    let mock = MockHttp::new();
    mock.respond(500, "{}");

    let agent = mock.agent_for(HttpService::Emby);
    let lines = captured(|| {
        let error = agent
            .get("http://127.0.0.1:1/Items?api_key=secret")
            .call()
            .expect_err("500 must surface as a status-code error");
        assert!(matches!(error, ureq::Error::StatusCode(500)), "{error}");
    });

    let failed: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains("event=http.request.failed"))
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
    // (`http=…`) gates both; the event's `source` is that target.
    assert!(failed.contains("source=http"), "{failed}");
}
