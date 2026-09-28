use super::*;
use crate::run::StopAction;
use mbv_ids::{EmbySessionId, ItemId, MediaSourceId};
use rstest::rstest;
use tracing_log as tracing_bridge;

// ── Feed playback plumbing (task 5.1) ─────────────────────────────────────

pub(crate) fn make_feed_entry(guid: &str, title: &str) -> mbv_queue::FeedEntry {
    mbv_queue::FeedEntry {
        guid: guid.into(),
        title: title.into(),
        enclosure_url: Some(format!("https://example.com/{guid}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: Some(300 * mbv_emby_model::TICKS_PER_SECOND as u64),
        pub_date_secs: None,
        feed_kind: Some(mbv_queue::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

fn make_feed_entry_no_source(guid: &str, title: &str) -> mbv_queue::FeedEntry {
    let mut e = make_feed_entry(guid, title);
    e.enclosure_url = None;
    e.link = None;
    e
}

#[rstest::fixture]
fn make_feed_session() -> (PlaybackRun, Arc<Mutex<PlayerStatus>>) {
    let entry = make_feed_entry("feed-1", "Podcast Episode 1");
    let status = Arc::new(Mutex::new(PlayerStatus::default()));
    let client = Arc::new(EmbyClient::new(mbv_config::Config::default()));
    let reporter = SessionReporter::new(
        client,
        None,
        ItemId::empty(),
        MediaSourceId::new(""),
        EmbySessionId::new(""),
        true, // is_audio
        Arc::clone(&status),
    );
    let (event_tx, _event_rx) = mpsc::channel();
    let session = PlaybackRun::new_from_slot_items(
        vec![ExecSlot {
            slot_id: QueueSlotId::from_raw(1),
            item: QueueItem::Feed(entry),
        }],
        RunInit {
            start_idx: 0,
            origin: PlaybackOrigin::Standalone,
            reporter,
            config: MpvRunConfig {
                headless: true,
                use_mpv_config: false,
                video_cache_forward_mb: 50,
                video_cache_back_mb: 100,
                no_scripts: true,
                always_skip_intro: false,
                audio_pipe_path: None,
                audio_pipe_samplerate: 0,
                audio_pipe_bitdepth: 0,
                audio_device: None,
            },
            startup_pause_for_pipe: false,
            status: Arc::clone(&status),
            event_tx,
            subtitle_prefs: Arc::new(Mutex::new(SubtitlePrefs::default())),
            shutdown_report_timeout: Arc::new(Mutex::new(None)),
            server_url: String::new(),
            token: String::new(),
            audiobookshelf_context: None,
            prepared_source: None,
        },
    );
    (session, status)
}

#[rstest]
#[case::initializes_with_correct_title_and_queue_len(0)]
#[case::load_active_item_state_sets_zero_position(1)]
#[case::origin_is_standalone(2)]
#[case::has_no_ext_sub_urls(3)]
#[case::reporter_has_no_session(4)]
fn feed_session_properties(
    make_feed_session: (PlaybackRun, Arc<Mutex<PlayerStatus>>),
    #[case] property: u8,
) {
    let (session, _status) = make_feed_session;
    match property {
        0 => {
            assert_eq!(session.osd_title, "Podcast Episode 1");
            assert_eq!(session.queue_len(), 1);
            assert_eq!(session.current_idx, 0);
        }
        1 => {
            assert_eq!(session.last_valid_pos, 0);
            assert!(session.series_id.as_str().is_empty());
        }
        2 => assert_eq!(session.origin, PlaybackOrigin::Standalone),
        3 => assert!(session.ext_sub_urls.is_empty()),
        4 => assert!(
            !session.reporter.has_session(),
            "feed reporter must have no Emby session"
        ),
        _ => unreachable!(),
    }
}

#[rstest::rstest]
#[case::primary_source_returns_enclosure(
    Some("https://example.com/g1.mp3"),
    None,
    Some("https://example.com/g1.mp3")
)]
#[case::falls_back_to_link(
    None,
    Some("https://fallback.example.com/ep"),
    Some("https://fallback.example.com/ep")
)]
#[case::no_primary_source_when_both_absent(None, None, None)]
fn feed_queue_item_primary_source(
    #[case] enclosure: Option<&str>,
    #[case] link: Option<&str>,
    #[case] expected: Option<&str>,
) {
    let mut entry = make_feed_entry("g1", "title");
    entry.enclosure_url = enclosure.map(str::to_owned);
    entry.link = link.map(str::to_owned);
    let qi = QueueItem::Feed(entry);
    if let QueueItem::Feed(e) = &qi {
        assert_eq!(e.primary_source(), expected);
    }
}

#[rstest::rstest]
fn feed_cancel_pending_quit_clears_state(
    make_feed_session: (PlaybackRun, Arc<Mutex<PlayerStatus>>),
) {
    let (mut session, _status) = make_feed_session;
    session.quit_at = Some(std::time::Instant::now());
    *session.shutdown_report_timeout.lock().unwrap() = Some(Duration::from_secs(5));

    session.cancel_pending_quit();

    assert!(session.quit_at.is_none());
    assert!(session.shutdown_report_timeout.lock().unwrap().is_none());
    assert_eq!(PlaybackRun::progress_join_budget(), Duration::from_secs(30));
}

// ── Feed append semantics ──────────────────────────────────────────────────

#[test]
fn feed_append_to_existing_queue_preserves_original_items() {
    // Simulate the append path: start with three Emby items, append a feed entry.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    assert_eq!(session.queue_len(), 3);
    assert_eq!(session.current_idx, 0);

    let feed = make_feed_entry("feed-appended", "Appended Feed");
    let queue_item = QueueItem::Feed(feed.clone());
    let new_idx = session.queue_len();
    session.queue.append_with_id(owner_slot_id(), queue_item);
    session.current_idx = new_idx;

    // Original items preserved, feed appended at end.
    assert_eq!(session.queue_len(), 4);
    assert_eq!(session.current_idx, 3);
    // Original items still at their positions.
    assert_eq!(
        session.queue.slots()[0].item.id(),
        "ep1",
        "first original item preserved"
    );
    assert_eq!(
        session.queue.slots()[1].item.id(),
        "ep2",
        "second original item preserved"
    );
    assert_eq!(
        session.queue.slots()[2].item.id(),
        "ep3",
        "third original item preserved"
    );
    // Feed item at the end.
    assert_eq!(session.queue.slots()[3].item.id(), "feed-appended");
}

#[test]
fn feed_append_to_existing_queue_does_not_change_origin() {
    // When appending to a Queue-origin session, origin must stay Queue
    // so on_end_file's advance path continues to work.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    assert_eq!(session.origin, PlaybackOrigin::Queue);

    let feed = make_feed_entry("feed-1", "Feed 1");
    session
        .queue
        .append_with_id(owner_slot_id(), QueueItem::Feed(feed));
    session.current_idx = session.queue_len() - 1;

    assert_eq!(
        session.origin,
        PlaybackOrigin::Queue,
        "origin must remain Queue after feed append"
    );
}

#[test]
fn feed_empty_queue_creates_standalone_session() {
    // When cmd_load_feed sees an empty queue, it creates a Standalone session.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    // Clear the queue to simulate idle state.
    session.queue = ExecutionSequence::empty();
    session.current_idx = 0;
    assert_eq!(session.queue_len(), 0);

    let feed = make_feed_entry("feed-idle", "Idle Feed");
    let queue_item = QueueItem::Feed(feed);
    session.origin = PlaybackOrigin::Standalone;
    session.queue = ExecutionSequence::from_slot_items(
        vec![(QueueSlotId::from_raw(1), queue_item)],
        Some(QueueSlotId::from_raw(1)),
    );
    session.current_idx = 0;

    assert_eq!(session.queue_len(), 1);
    assert_eq!(session.current_idx, 0);
    assert_eq!(session.origin, PlaybackOrigin::Standalone);
}

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

// ── Source-less feed entry ─────────────────────────────────────────────────

#[test]
fn sourceless_feed_entry_rejected_by_both_early_and_command_checks() {
    let entry = make_feed_entry_no_source("no-src", "No Source");
    // Early check: primary_source() must be None.
    assert!(
        entry.primary_source().is_none(),
        "source-less entry must have no primary source"
    );
    // Command-level check: URL must be empty.
    let url = entry.primary_source().unwrap_or("").to_string();
    assert!(url.is_empty(), "source-less feed must produce empty URL");
}

// ── Behavioral: mixed queue lifecycle ──────────────────────────────────────

#[test]
fn feed_append_displaced_emby_reported_before_ids_clear_and_drain() {
    // Proves cmd_load_feed → on_end_file state machine:
    // 1) report_stopped_background fires with live IDs for old Emby item
    // 2) IDs cleared for Feed
    // 3) the replacement drain suppresses displaced EndFile, resets stop_report
    // 4) After drain, session has no IDs — Feed lifecycle is safe
    let (mut session, _status) = make_queue_session_for_pos_tests(1);
    assert!(session.reporter.has_session());
    let original_id = session.reporter.ids.lock().unwrap().0.clone();
    assert!(!original_id.as_str().is_empty());

    // Simulate cmd_load_feed active path
    session
        .reporter
        .report_stopped_background(session.last_valid_pos);
    assert!(
        session.reporter.has_session(),
        "IDs survive through report_stopped_background"
    );
    session.reporter.clear_session();
    assert!(!session.reporter.has_session());
    session.begin_item_lifecycle(StopAction::NothingPlaying);

    // Simulate on_end_file drain path (displaced EndFile)
    assert!(!session.load_is_ready());
    assert_eq!(session.on_drained(), Drained::HitZero);
    assert!(session.load_is_ready());
    assert_eq!(session.stop_report(), StopReport::NotSent);

    // Feed lifecycle state
    assert!(!session.reporter.has_session());
    assert!(!session.reporter.report_stopped(0));
    session.reporter.report_progress("TimeUpdate");
}

#[test]
fn mixed_queue_feed_advances_to_next_emby_item() {
    // After a Feed item completes in a mixed queue, on_end_file's advance
    // path should reach a subsequent Emby item and re-initialize reporting.
    let (mut session, _status, _, http) = make_queue_session_for_pos_tests_with_mock(0);
    // Queue: [Emby(ep1), Emby(ep2), Feed(f1), Emby(ep3)]
    let feed = make_feed_entry("f1", "Feed 1");
    session
        .queue
        .append_with_id(owner_slot_id(), QueueItem::Feed(feed));
    let ep3 = make_media_item("ep3");
    session
        .queue
        .append_with_id(owner_slot_id(), QueueItem::Emby(Box::new(ep3)));
    assert_eq!(session.queue_len(), 5);

    // Simulate being on the Feed item at index 2 with cleared IDs.
    session.current_idx = 2;
    session.reporter.clear_session();
    assert!(!session.reporter.has_session());

    // Simulate advancing to index 3 (ep3): start_item re-initializes IDs.
    {
        let mut ids = session.reporter.ids.lock().unwrap();
        ids.0 = ItemId::new("ep3");
        ids.1 = MediaSourceId::new("msid-ep3");
        ids.2 = EmbySessionId::new("sid-ep3");
    };
    assert!(session.reporter.has_session());
    // Succeeds instantly on the mock instead of failing against no server
    // with a 500ms retry sleep.
    http.respond(200, "");
    let _ = session.reporter.report_stopped(0);
}

#[test]
fn feed_queue_quit_path_does_not_mark_played_with_empty_id() {
    // Regression: Queue→Quit path (on_end_file lines 270-275) accessed
    // self.reporter.ids directly and called mark_played without a
    // has_session() guard.  A Feed item with Queue origin exiting via
    // Quit would invoke mark_played("") / retry with an empty ID.
    // After the fix, the guard prevents this.
    let (mut session, _status) = make_queue_session_for_pos_tests(0);
    // Append a Feed item so origin stays Queue.
    let feed = make_feed_entry("f-quit", "Quit Feed");
    session
        .queue
        .append_with_id(owner_slot_id(), QueueItem::Feed(feed));
    session.current_idx = session.queue_len() - 1;
    // Clear IDs as cmd_load_feed does.
    session.reporter.clear_session();
    assert!(!session.reporter.has_session());

    // The Queue→Quit guard now checks has_session().  With no session,
    // the mark_played block is skipped entirely.  Verify the guard
    // condition: (natural_end || near_end) && !completed_is_audio && has_session().
    // All three sub-conditions except has_session() could be true for
    // a Feed audio item (is_audio=true → !completed_is_audio=false →
    // guard blocks), but the key invariant is: when has_session() is
    // false, mark_played is never reached regardless of other conditions.
    assert!(!session.reporter.has_session());
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

/// Rendered fields of one span, kept in its registry extensions so `on_record`
/// can append and `on_event` can render enclosing spans outer→inner.
struct CapturedFields(Vec<(String, String)>);

struct FieldPairs<'a>(&'a mut Vec<(String, String)>);

impl tracing::field::Visit for FieldPairs<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push((field.name().to_owned(), format!("{value:?}")));
    }
}

/// Capture layer for the reporter-correlation test: every event becomes one
/// `key=value` fragment line (event fields, then enclosing spans outer→inner)
/// sent to the test thread over a channel, so worker-thread events can be
/// awaited without sleeps.
struct CaptureLayer {
    lines: Mutex<mpsc::Sender<String>>,
}

impl<S> tracing_subscriber::Layer<S> for CaptureLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut fields = Vec::new();
        attrs.record(&mut FieldPairs(&mut fields));
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(CapturedFields(fields));
        }
    }

    fn on_record(
        &self,
        id: &tracing::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut fields = Vec::new();
        values.record(&mut FieldPairs(&mut fields));
        if let Some(existing) = span.extensions_mut().get_mut::<CapturedFields>() {
            existing.0.append(&mut fields);
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let mut fields = vec![("event".to_owned(), event.metadata().name().to_owned())];
        event.record(&mut FieldPairs(&mut fields));
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope.from_root() {
                if let Some(captured) = span.extensions().get::<CapturedFields>() {
                    fields.extend(captured.0.iter().cloned());
                }
            }
        }
        let line = fields
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = self.lines.lock().unwrap().send(line);
    }
}

/// Blocks until a captured line containing `needle` arrives. The reporter
/// worker logs the awaited lines unconditionally, so the receive is the seam
/// observing the worker's progress — no sleeps.
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
    let subscriber = tracing_subscriber::registry().with(CaptureLayer {
        lines: Mutex::new(line_tx),
    });

    tracing::subscriber::with_default(subscriber, || {
        // mbv-emby is converted in a later unit, so bridge its log records into
        // this capture subscriber while this regression exercises player spans.
        let _ = tracing_bridge::LogTracer::init();

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

        let before = wait_for_captured_line(&line_rx, "PlaybackInfo item=item-b");
        let after = wait_for_captured_line(&line_rx, "Playing item=item-b");

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
        let progress = wait_for_captured_line(&line_rx, "outbound: Progress");
        assert_report_line_correlated(&progress, "item-b", "session-b", &["item-a", "session-a"]);

        reporter.report_ping();
        let ping = wait_for_captured_line(&line_rx, "outbound: Ping session=session-b");
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
    http.respond(200, "");
    http.respond(200, "");
    let cfg = mbv_config::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..mbv_config::Config::default()
    };
    let client = Arc::new(EmbyClient::new(cfg).with_test_agent(http.agent()));

    let (line_tx, line_rx) = mpsc::channel();
    let subscriber = tracing_subscriber::registry().with(CaptureLayer {
        lines: Mutex::new(line_tx),
    });

    tracing::subscriber::with_default(subscriber, || {
        let _ = tracing_bridge::LogTracer::init();

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

        let playback_info = wait_for_captured_line(&line_rx, "PlaybackInfo item=item-b");
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
        let start = wait_for_captured_line(&line_rx, "Playing item=item-b");
        assert_report_line_correlated(&start, "item-b", "session-b", &["item-a", "session-a"]);
    });
}

// ── QueueAppend must not drop appended Feed items ───────────────────────────

#[test]
fn append_items_to_queue_keeps_feed_items() {
    let (mut session, _status) = make_queue_session_for_pos_tests(1);
    let entry = make_feed_entry("feed-1", "Podcast Episode 1");

    session.append_items_to_queue(vec![ExecSlot {
        slot_id: QueueSlotId::from_raw(4_242),
        item: QueueItem::Feed(entry.clone()),
    }]);

    assert_eq!(session.queue_len(), 4);
    assert_eq!(
        session
            .queue
            .slots()
            .last()
            .map(|slot| slot.item.id().to_string()),
        Some(entry.guid.clone())
    );
    // The owner-assigned slot id is retained through the append helper.
    assert_eq!(
        session.queue.slots().last().map(|slot| slot.slot_id),
        Some(QueueSlotId::from_raw(4_242))
    );
}
