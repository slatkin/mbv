use super::*;
use mbv_ids::{EmbySessionId, ItemId, MediaSourceId};
use std::sync::Mutex;

// ── Reporter session guards ────────────────────────────────────────────────

#[rstest::rstest]
fn reporter_session_lifecycle(
    make_no_session_reporter_with_ids: (SessionReporter, mbv_net::mock_http::MockHttp),
    make_no_session_reporter: SessionReporter,
) {
    let (with_ids, _) = make_no_session_reporter_with_ids;
    assert!(with_ids.has_session());
    with_ids.clear_session();
    assert!(!with_ids.has_session());
    assert!(!make_no_session_reporter.has_session());
}

// ── Behavioral: reporter no-session reporting ──────────────────────────────

#[rstest::fixture]
fn make_no_session_reporter() -> SessionReporter {
    let status = Arc::new(Mutex::new(PlayerStatus::default()));
    let client = Arc::new(EmbyClient::new(mbv_config::Config::default()));
    SessionReporter::new(
        client,
        None,
        ItemId::empty(),
        MediaSourceId::new(""),
        EmbySessionId::new(""),
        false,
        status,
    )
}

#[rstest::rstest]
fn reporter_no_session_all_reporting_is_noop(make_no_session_reporter: SessionReporter) {
    let reporter = make_no_session_reporter;
    assert!(!reporter.has_session());
    assert!(!reporter.report_stopped(12345));
    assert!(!reporter.report_stopped_for_shutdown(0, Duration::from_secs(5)));
    reporter.report_stopped_background(12345);
    reporter.report_progress("TimeUpdate");
    reporter.report_progress("Pause");
}

#[rstest::rstest]
fn reporter_with_session_stopped_proceeds_to_client(
    make_no_session_reporter_with_ids: (SessionReporter, mbv_net::mock_http::MockHttp),
) {
    let (reporter, http) = make_no_session_reporter_with_ids;
    assert!(reporter.has_session());
    // Succeeds instantly on the mock: with a session the call must proceed
    // to the client and hit the Stopped endpoint — previously this asserted
    // `!report_stopped`, i.e. it depended on no server answering.
    http.respond(200, "");
    assert!(reporter.report_stopped(0));
    let requests = http.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /Sessions/Playing/Stopped"));
}

#[rstest::rstest]
fn progress_and_ping_send_nothing_before_the_start_report(
    make_no_session_reporter_with_ids: (SessionReporter, mbv_net::mock_http::MockHttp),
) {
    // The ids resolve at submit, but Emby has no session until the first
    // start report goes out: the progress thread (spawned at submit, ticking
    // on its interval) and pings must not reference a session the server
    // never saw started.
    let (reporter, http) = make_no_session_reporter_with_ids;
    assert!(reporter.has_session());

    reporter.report_progress("TimeUpdate");
    reporter.report_ping();
    assert!(
        http.requests().is_empty(),
        "nothing is sent before the start report: {:?}",
        http.requests()
    );

    reporter
        .start_reported
        .store(true, std::sync::atomic::Ordering::Relaxed);
    http.respond(200, "");
    reporter.report_progress("TimeUpdate");
    assert_eq!(http.requests().len(), 1, "the gate opens on the start");
}

#[rstest::fixture]
fn make_no_session_reporter_with_ids() -> (SessionReporter, mbv_net::mock_http::MockHttp) {
    let status = Arc::new(Mutex::new(PlayerStatus::default()));
    let http = mbv_net::mock_http::MockHttp::new();
    let agent = http.agent();
    let cfg = mbv_config::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..mbv_config::Config::default()
    };
    let client = Arc::new(EmbyClient::new(cfg).with_test_agent(agent));
    (
        SessionReporter::new(
            client,
            None,
            ItemId::new("real-item"),
            MediaSourceId::new("msid"),
            EmbySessionId::new("sid"),
            false,
            status,
        ),
        http,
    )
}

// ── Reporter correlation (structured-logging design D5) ───────────────────

/// The shared logfmt capture layer, sending each rendered line to the test
/// thread over a channel so worker-thread events can be awaited without sleeps.
fn capture_layer<S>(lines: mpsc::Sender<String>) -> impl tracing_subscriber::Layer<S>
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    let lines = Mutex::new(lines);
    mbv_core::applog::test_support::capture_layer(move |line| {
        let _ = lines.lock().unwrap().send(line.to_owned());
    })
}

/// Blocks until a captured line containing `needle` arrives. Needles are D4
/// event names, not message text, so message rewording cannot silently hang tests.
/// The reporter worker logs the awaited lines unconditionally, so the receive is
/// the seam observing the worker's progress — no sleeps.
fn wait_for_captured_line(rx: &mpsc::Receiver<String>, needle: &str) -> String {
    let mut seen = 0;
    loop {
        let line = rx
            .recv()
            .expect("capture channel closed before the expected line");
        if line.contains(needle) {
            return line;
        }
        seen += 1;
        assert!(
            seen < 200,
            "line containing {needle} never captured; last: {line}"
        );
    }
}

/// Asserts a report line names the reported session's item and play session,
/// carries no `slot` field, and carries nothing from `forbidden` (the other
/// session's values).
fn assert_report_line_correlated(line: &str, item: &str, session: &str, forbidden: &[&str]) {
    assert!(
        line.contains(&format!("item={item}")),
        "reported item missing: {line}"
    );
    assert!(
        line.contains(&format!("play_session={session}")),
        "reported play_session missing: {line}"
    );
    for value in forbidden {
        assert!(
            !line.contains(value),
            "foreign value {value} leaked: {line}"
        );
    }
    assert!(
        !line.contains("slot="),
        "playback span leaked onto report line: {line}"
    );
}

#[test]
fn deferred_start_report_lines_carry_new_item_not_previous_session() {
    use tracing_subscriber::prelude::*;

    let http = mbv_net::mock_http::MockHttp::new();
    // The worker consumes, in order: the previous session's stopped report,
    // the deferred start's PlaybackInfo fetch, then the start report itself;
    // the direct progress and ping calls follow once the ids hold session B.
    http.respond(200, "");
    http.respond(
        200,
        r#"{"MediaSources":[{"Id":"msid-b"}],"PlaySessionId":"session-b"}"#,
    );
    http.respond(200, "");
    http.respond(200, "");
    http.respond(200, "");
    let cfg = mbv_config::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..mbv_config::Config::default()
    };
    let client = Arc::new(EmbyClient::new(cfg).with_test_agent(http.agent()));

    let (line_tx, line_rx) = mpsc::channel();
    let subscriber = tracing_subscriber::registry().with(capture_layer(line_tx));

    tracing::subscriber::with_default(subscriber, || {
        // Session A's playback span, entered as the run loop would hold it.
        let span_a = PlaybackRun::new_playback_span(
            QueueSlotId::from_raw(7),
            &QueueItem::Emby(Box::new(make_media_item("item-a"))),
        );
        span_a.record("play_session", "session-a");
        let _entered_a = span_a.enter();

        let status = Arc::new(Mutex::new(PlayerStatus::default()));
        let reporter = SessionReporter::new(
            Arc::clone(&client),
            None,
            ItemId::new("item-a"),
            MediaSourceId::new("msid-a"),
            EmbySessionId::new("session-a"),
            false,
            status,
        );

        // The run loop rebuilt the span for the new slot before handing the
        // deferred start to the worker (cmd_load_new order); the shared ids
        // still hold session A. The worker runs on its own thread and reaches
        // this subscriber because its spawn goes through `carry_dispatcher`,
        // which carries the dispatcher but not session A's span.
        let span_b = PlaybackRun::new_playback_span(
            QueueSlotId::from_raw(8),
            &QueueItem::Emby(Box::new(make_media_item("item-b"))),
        );
        reporter.transition_to_deferred(&make_media_item("item-b"), 0, span_b);

        // Non-deferred path: the stopped report for session A must carry its
        // own ids (its span is entered, not just created).
        let stopped = wait_for_captured_line(&line_rx, "sent stopped report");
        assert_report_line_correlated(&stopped, "item-a", "session-a", &["item-b", "session-b"]);

        let before = wait_for_captured_line(&line_rx, "event=emby.playback_info.requested");
        let after = wait_for_captured_line(&line_rx, "event=emby.playback_start.requested");

        assert!(
            before.contains("item=item-b"),
            "new item missing before resolution: {before}"
        );
        assert!(
            !before.contains("play_session="),
            "play_session recorded before get_playback_info resolved: {before}"
        );
        for value in ["item-a", "session-a"] {
            assert!(
                !before.contains(value),
                "previous session value {value} leaked: {before}"
            );
        }
        assert!(
            !before.contains("slot="),
            "playback span leaked onto deferred report line: {before}"
        );
        assert_report_line_correlated(&after, "item-b", "session-b", &["item-a", "session-a"]);

        // After the deferred job the shared ids hold session B, so the direct
        // progress and ping calls must carry B's ids.
        reporter.report_progress("TimeUpdate");
        let progress = wait_for_captured_line(&line_rx, "event=emby.progress.requested");
        assert_report_line_correlated(&progress, "item-b", "session-b", &["item-a", "session-a"]);

        reporter.report_ping();
        let ping = wait_for_captured_line(&line_rx, "event=emby.ping.requested");
        assert_report_line_correlated(&ping, "item-b", "session-b", &["item-a", "session-a"]);
    });
}

#[test]
fn resolved_start_report_line_carries_resolved_item_and_session() {
    use tracing_subscriber::prelude::*;

    let http = mbv_net::mock_http::MockHttp::new();
    // transition_to fetches PlaybackInfo synchronously on the calling
    // thread; the worker then consumes the stopped and start reports.
    http.respond(
        200,
        r#"{"MediaSources":[{"Id":"msid-b"}],"PlaySessionId":"session-b"}"#,
    );
    // The stopped report runs concurrently with the synchronous playback-info
    // fetch; either may consume the first scripted response.
    http.respond(
        200,
        r#"{"MediaSources":[{"Id":"msid-b"}],"PlaySessionId":"session-b"}"#,
    );
    http.respond(200, "");
    let cfg = mbv_config::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..mbv_config::Config::default()
    };
    let client = Arc::new(EmbyClient::new(cfg).with_test_agent(http.agent()));

    let (line_tx, line_rx) = mpsc::channel();
    let subscriber = tracing_subscriber::registry().with(capture_layer(line_tx));

    tracing::subscriber::with_default(subscriber, || {
        let span_a = PlaybackRun::new_playback_span(
            QueueSlotId::from_raw(7),
            &QueueItem::Emby(Box::new(make_media_item("item-a"))),
        );
        span_a.record("play_session", "session-a");
        let _entered_a = span_a.enter();

        let status = Arc::new(Mutex::new(PlayerStatus::default()));
        let reporter = SessionReporter::new(
            Arc::clone(&client),
            None,
            ItemId::new("item-a"),
            MediaSourceId::new("msid-a"),
            EmbySessionId::new("session-a"),
            false,
            status,
        );

        // Synchronously resolved transition: the run loop rebuilt and entered
        // the new slot's span before the call, and the resolved session id is
        // already known at job entry.
        let span_b = PlaybackRun::new_playback_span(
            QueueSlotId::from_raw(8),
            &QueueItem::Emby(Box::new(make_media_item("item-b"))),
        );
        let _entered_b = span_b.enter();
        reporter.transition_to(&make_media_item("item-b"), 0, &span_b);

        let playback_info = wait_for_captured_line(&line_rx, "event=emby.playback_info.requested");
        assert!(
            playback_info.contains("slot=8"),
            "new slot missing: {playback_info}"
        );
        assert!(
            !playback_info.contains("slot=7"),
            "old slot leaked: {playback_info}"
        );
        assert!(
            !playback_info.contains("item=item-a"),
            "old item leaked: {playback_info}"
        );
        let start = wait_for_captured_line(&line_rx, "event=emby.playback_start.requested");
        assert_report_line_correlated(&start, "item-b", "session-b", &["item-a", "session-a"]);
    });
}

// ── Submit-time start report waits for actual playback ─────────────────────

/// Blocks until the captured line containing `needle`, collecting every line
/// seen before it (the needle included), so a test can assert absence among
/// the lines a FIFO worker drained before its sentinel.
fn collect_until_captured_line(rx: &mpsc::Receiver<String>, needle: &str) -> Vec<String> {
    let mut seen = Vec::new();
    loop {
        let line = rx
            .recv()
            .expect("capture channel closed before the expected line");
        let found = line.contains(needle);
        seen.push(line);
        if found {
            return seen;
        }
    }
}

#[test]
fn run_that_never_plays_reports_no_emby_start() {
    // Regression (#938-era session churn): the start report fired when the
    // submission was accepted, so a run that died without ever playing still
    // sent start+stop to Emby within milliseconds, and repeated attempts
    // churned the server-side session. The start report now waits for actual
    // playback, so a never-played run sends only its stop.
    let (mut session, _status, _events, http) = make_queue_session_for_pos_tests_with_mock(0);
    session.origin = PlaybackOrigin::Standalone;
    http.respond(200, "");
    let mut progress = noop_progress();

    assert!(!session.on_end_file_standalone(EndFileReason::Stop, &mut progress));

    let requests = http.requests();
    assert!(
        requests
            .iter()
            .any(|r| r.starts_with("POST /Sessions/Playing/Stopped")),
        "the stop report is expected: {requests:?}"
    );
    assert!(
        !requests
            .iter()
            .any(|r| r.starts_with("POST /Sessions/Playing ")),
        "a never-played run must not report a session start: {requests:?}"
    );
}

#[test]
fn first_playback_restart_reports_the_emby_start_once() {
    use tracing_subscriber::prelude::*;

    let (line_tx, line_rx) = mpsc::channel();
    let subscriber = tracing_subscriber::registry().with(capture_layer(line_tx));

    tracing::subscriber::with_default(subscriber, || {
        // The session (and its report worker) must be built inside the
        // captured dispatcher: the worker inherits the dispatcher at spawn.
        let (mut session, _status, _events, http) = make_queue_session_for_pos_tests_with_mock(0);
        // Requests race for the script FIFO: the worker's start POST (async)
        // and the run thread's sync progress POSTs can land in either order,
        // so script more responses than any interleaving consumes. What the
        // test asserts is the start line count, not the mock's order.
        for _ in 0..6 {
            http.respond(200, "");
        }
        let (ours, _peer) = std::os::unix::net::UnixStream::pair().expect("socket pair");
        let mpv =
            Mpv::for_test(crate::mpv_session::ipc::IpcSession::from_stream(ours).expect("session"));

        session.on_playback_restart(&mpv);
        session.on_playback_restart(&mpv);
        // Sentinel: FIFO order makes every earlier line observable before it.
        session.reporter.report_stopped_background(0);
        let drained = collect_until_captured_line(&line_rx, "sent stopped report");
        let starts = drained
            .iter()
            .filter(|line| line.contains("event=emby.playback_start.requested"))
            .count();
        assert_eq!(starts, 1, "exactly one start report, drained: {drained:?}");
    });
}

#[test]
fn a_start_reported_by_the_advance_path_is_not_repeated_by_the_first_restart() {
    // An advance whose start report already went out (a submitted item that
    // failed before playing, or a mixed queue's first Emby item) must not be
    // duplicated when the run's first `PlaybackRestart` arrives for that
    // item -- the reporter's start flag, not a run-local one-shot, decides.
    use tracing_subscriber::prelude::*;

    let (line_tx, line_rx) = mpsc::channel();
    let subscriber = tracing_subscriber::registry().with(capture_layer(line_tx));

    tracing::subscriber::with_default(subscriber, || {
        let (mut session, _status, _events, http) = make_queue_session_for_pos_tests_with_mock(0);
        for _ in 0..6 {
            http.respond(200, "");
        }
        let (ours, _peer) = std::os::unix::net::UnixStream::pair().expect("socket pair");
        let mpv =
            Mpv::for_test(crate::mpv_session::ipc::IpcSession::from_stream(ours).expect("session"));

        // The advance path's order: `start_item` reports the start before the
        // item's `PlaybackRestart` can arrive.
        let item = session
            .active_item()
            .and_then(QueueItem::as_emby)
            .expect("the queue holds Emby items")
            .clone();
        let (_urls, ok) = session.reporter.start_item(&item, &session.playback_span);
        assert!(ok, "the mock answered the advance's start report");

        session.on_playback_restart(&mpv);
        // Sentinel: FIFO order makes every earlier line observable before it.
        session.reporter.report_stopped_background(0);
        let drained = collect_until_captured_line(&line_rx, "sent stopped report");
        let starts = drained
            .iter()
            .filter(|line| line.contains("event=emby.playback_start.requested"))
            .count();
        assert_eq!(starts, 1, "exactly one start report, drained: {drained:?}");
    });
}
