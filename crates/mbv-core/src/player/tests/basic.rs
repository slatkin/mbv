use super::*;
use crate::api::EmbyImageTags;

// ── shift_index_for_move ──────────────────────────────────────────────────

#[test]
fn shift_index_for_move_moves_the_tracked_index_itself() {
    assert_eq!(shift_index_for_move(1, 1, 3), 3);
    assert_eq!(shift_index_for_move(3, 3, 1), 1);
}

#[test]
fn shift_index_for_move_shifts_indices_between_from_and_to() {
    // Moving 1 -> 3 closes the gap it left, shifting everything in (1, 3] down.
    assert_eq!(shift_index_for_move(2, 1, 3), 1);
    assert_eq!(shift_index_for_move(3, 1, 3), 2);
    // Moving 3 -> 1 opens a gap at 1, shifting everything in [1, 3) up.
    assert_eq!(shift_index_for_move(1, 3, 1), 2);
    assert_eq!(shift_index_for_move(2, 3, 1), 3);
}

#[test]
fn shift_index_for_move_leaves_unrelated_indices_alone() {
    assert_eq!(shift_index_for_move(0, 1, 3), 0);
    assert_eq!(shift_index_for_move(4, 1, 3), 4);
}

// ── PlayerCommand serde (IPC protocol integrity) ─────────────────────────

pub(in crate::player) fn make_media_item(id: &str) -> crate::api::EmbyItem {
    crate::api::EmbyItem {
        id: id.into(),
        name: "Test Episode".into(),
        item_type: "Episode".into(),
        is_folder: false,
        child_count: None,
        media_type: "Video".into(),
        collection_type: String::new(),
        runtime_ticks: 3600 * crate::api::TICKS_PER_SECOND,
        played: false,
        playback_position_ticks: 0,
        series_id: "series1".into(),
        series_name: "Show".into(),
        album_id: String::new(),
        album: String::new(),
        index_number: 2,
        parent_index_number: 1,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        artist_items: Vec::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genres: Vec::new(),
        people: Vec::new(),
        external_urls: Vec::new(),
        playlist_item_id: String::new(),
        image_tags: EmbyImageTags::default(),
    }
}

/// Pair queue items with owner-assigned slot ids for the run's queue
/// submission/append helpers.
///
/// Ids come from one monotonic counter per thread rather than a fixed base, so
/// a test that seeds a run and *then* appends cannot hand the run an identity it
/// already holds — the execution sequence asserts slot uniqueness on adoption.
pub(in crate::player) fn owner_paired(items: Vec<QueueItem>) -> Vec<ExecSlot> {
    items
        .into_iter()
        .map(|item| ExecSlot {
            slot_id: owner_slot_id(),
            item,
        })
        .collect()
}

/// A fresh owner-assigned slot identity for fixture queues.
pub(in crate::player) fn owner_slot_id() -> QueueSlotId {
    thread_local! {
        static NEXT_FIXTURE_SLOT_ID: std::cell::Cell<u64> = const { std::cell::Cell::new(1_000) };
    }
    NEXT_FIXTURE_SLOT_ID.with(|next| {
        let raw = next.get();
        next.set(raw + 1);
        QueueSlotId::from_raw(raw)
    })
}

pub(in crate::player) fn make_queue_session_for_pos_tests(
    start_idx: usize,
) -> (PlaybackRun, Arc<Mutex<PlayerStatus>>) {
    let (session, status, _) = make_queue_session_for_pos_tests_with_events(start_idx);
    (session, status)
}

pub(in crate::player) fn make_queue_session_for_pos_tests_with_events(
    start_idx: usize,
) -> (
    PlaybackRun,
    Arc<Mutex<PlayerStatus>>,
    mpsc::Receiver<PlayerEvent>,
) {
    let (session, status, events, _) = make_queue_session_for_pos_tests_with_mock(start_idx);
    (session, status, events)
}

/// Mock-backed session fixture: the Emby client runs on an in-memory
/// transport, so reporting calls succeed instantly instead of failing against
/// no server with a 500ms retry sleep each. Script responses via the returned
/// `MockHttp` before triggering. The mock transport ignores the URL, but it
/// must stay an IP literal so the default resolver never attempts DNS.
pub(in crate::player) fn make_queue_session_for_pos_tests_with_mock(
    start_idx: usize,
) -> (
    PlaybackRun,
    Arc<Mutex<PlayerStatus>>,
    mpsc::Receiver<PlayerEvent>,
    crate::mock_http::MockHttp,
) {
    let http = crate::mock_http::MockHttp::new();
    let agent = http.agent();
    let cfg = crate::config::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..crate::config::Config::default()
    };
    let client = Arc::new(EmbyClient::new(cfg).with_test_agent(agent));
    let (session, status, events) = queue_session_for_pos_tests_with_client(start_idx, client);
    (session, status, events, http)
}

fn queue_session_for_pos_tests_with_client(
    start_idx: usize,
    client: Arc<EmbyClient>,
) -> (
    PlaybackRun,
    Arc<Mutex<PlayerStatus>>,
    mpsc::Receiver<PlayerEvent>,
) {
    let emby_items = [
        make_media_item("ep1"),
        make_media_item("ep2"),
        make_media_item("ep3"),
    ];
    let items: Vec<QueueItem> = emby_items
        .iter()
        .cloned()
        .map(|i| QueueItem::Emby(Box::new(i)))
        .collect();
    let status = Arc::new(Mutex::new(PlayerStatus {
        active: true,
        current_idx: start_idx,
        queue_len: items.len(),
        runtime_ticks: emby_items[start_idx].runtime_ticks,
        title: emby_items[start_idx].display_name(),
        ..PlayerStatus::default()
    }));
    let reporter = SessionReporter::new(
        client,
        None,
        ItemId::new(emby_items[start_idx].id.clone()),
        MediaSourceId::new("msid"),
        EmbySessionId::new("sid"),
        false,
        Arc::clone(&status),
    );
    let (event_tx, event_rx) = mpsc::channel();
    let session = PlaybackRun::new_from_slot_items(
        owner_paired(items),
        RunInit {
            start_idx,
            origin: PlaybackOrigin::Queue,
            reporter,
            config: MpvRunConfig {
                headless: false,
                use_mpv_config: false,
                video_cache_forward_mb: 50,
                video_cache_back_mb: 100,
                no_scripts: true,
                always_skip_intro: false,
                audio_pipe_path: Some("/tmp/mbv-test-pipe".into()),
                audio_pipe_samplerate: 48_000,
                audio_pipe_bitdepth: 16,
                audio_device: None,
            },
            startup_pause_for_pipe: false,
            status: Arc::clone(&status),
            event_tx,
            subtitle_prefs: Arc::new(Mutex::new(SubtitlePrefs::default())),
            shutdown_report_timeout: Arc::new(Mutex::new(None)),
            server_url: "http://example.test".into(),
            token: "token".into(),
            audiobookshelf_context: None,
            prepared_source: None,
        },
    );
    (session, status, event_rx)
}
