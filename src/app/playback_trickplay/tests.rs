//! Tests for the scrubber preview's trickplay tile sheets
//! (`add-tauri-gui-client` task 6.7): the position resolves to the smallest
//! offered grid's cached sheet tile, and the cache keeps its bound.

use super::*;

use mbv_emby_model::TICKS_PER_SECOND;
use std::sync::Mutex;

use crate::app::app_stub::make_app_stub;
use crate::app::state::service_runtime::EmbyRuntime;
use crate::app::tests::QueueViewTestExt;
use mbv_emby::EmbyClient;
use mbv_net::mock_http::MockHttp;

/// A URL that never leaves the process: the mock transport ignores it, but it
/// must remain an IP literal so the default resolver never attempts DNS.
const TEST_URL: &str = "http://127.0.0.1:1";

/// The item the tests play: `make_item`'s fixed id and a movie type.
fn playing_app_with_grids(grids: Vec<TrickplayGrid>) -> App {
    let mut app = make_app_stub();
    app.local_view.adopt_queue_items_with_active(
        vec![mbv_queue::QueueItem::Emby(Box::new(
            mbv_emby_model::test_support::make_item("Item 0", "Movie"),
        ))],
        0,
        0,
    );
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.queue_len = 1;
    });
    let key = app
        .playback_queue()
        .item_at(0)
        .expect("the stub queue has a first slot")
        .content_id();
    app.playback_metadata_cache.insert(
        key,
        crate::app::playback_metadata::PlaybackDecorations {
            chapters: Vec::new(),
            intro: None,
            credits: None,
            trickplay: Some(grids),
        },
    );
    app
}

fn grid(width: u32, interval_ms: u64, thumbnail_count: u32) -> TrickplayGrid {
    TrickplayGrid {
        width,
        height: 180,
        tile_width: 10,
        tile_height: 10,
        thumbnail_count,
        interval_ms,
        media_source_id: "ms1".to_string(),
    }
}

fn sheet_key(width: u32, sheet: u32) -> TrickplaySheetKey {
    TrickplaySheetKey {
        item_id: "id".to_string(),
        width,
        sheet,
    }
}

/// Install the mock-backed Emby fill, so a sheet fetch the preview issues is
/// visible in `trickplay_sheets_loading` with no network.
fn install_mock_emby(app: &mut App, http: &MockHttp) {
    let client = EmbyClient::new(crate::config::Config {
        server_url: TEST_URL.into(),
        ..crate::config::Config::default()
    })
    .with_test_agent(http.agent());
    app.emby_runtime = EmbyRuntime::ready(Arc::new(Mutex::new(client)));
}

/// Run one sheet's fetch against the mock and drain its landing, exactly as
/// the preview's worker and the Client's tick do.
fn fetch_and_drain(app: &mut App, key: &TrickplaySheetKey) {
    let client = app
        .emby_snapshot()
        .expect("the mock fill installs a client");
    let fetch = fetch_trickplay_sheet(&client, key, "ms1");
    app.channels
        .trickplay_tx
        .send((key.clone(), fetch))
        .expect("the stub channel accepts the fetch");
    assert!(app.drain_trickplay_sheet_events());
}

/// The preview resolves the hovered position to the smallest offered width's
/// tile and the cached sheet's bytes: a twenty-fifth thumbnail lands at column
/// five and row two of the first sheet.
#[test]
fn preview_resolves_the_position_to_the_smallest_grids_cached_tile() {
    let mut app = playing_app_with_grids(vec![grid(854, 10_000, 250), grid(320, 10_000, 250)]);
    app.trickplay_sheets
        .insert(sheet_key(320, 0), Some(std::sync::Arc::from(&b"jpeg"[..])));
    app.trickplay_sheets
        .insert(sheet_key(854, 0), Some(std::sync::Arc::from(&b"wide"[..])));

    let preview = app
        .trickplay_preview_tile(25 * 10 * TICKS_PER_SECOND)
        .expect("the cached sheet answers the hovered position");

    assert_eq!(
        preview,
        TrickplayPreviewTile {
            key: sheet_key(320, 0),
            width: 320,
            height: 180,
            columns: 10,
            column: 5,
            row: 2,
            jpeg: std::sync::Arc::from(&b"jpeg"[..]),
        }
    );
}

/// A sheet the server does not hold stays out of the preview: the drain's
/// `None` records the absence, so the position paints no frame and no fetch
/// is issued again for it.
#[test]
fn preview_projects_no_tile_for_a_sheet_the_server_does_not_hold() {
    let mut app = playing_app_with_grids(vec![grid(320, 10_000, 250)]);
    app.trickplay_sheets.insert(sheet_key(320, 0), None);

    assert_eq!(app.trickplay_preview_tile(0), None);
}

/// A sheet the server does not hold answers `404`, the one failure the cache
/// records as absence (task 6.7 fix): the drained absence paints no tile and
/// the same hover position issues no second fetch.
#[test]
fn a_not_found_sheet_is_cached_as_absent_and_not_re_asked() {
    let http = MockHttp::new();
    http.respond(404, "");
    let mut app = playing_app_with_grids(vec![grid(320, 10_000, 250)]);
    install_mock_emby(&mut app, &http);
    let key = sheet_key(320, 0);

    fetch_and_drain(&mut app, &key);

    assert!(
        matches!(app.trickplay_sheets.get(&key), Some(None)),
        "the server's 404 is the cache's definite absence"
    );
    assert_eq!(app.trickplay_preview_tile(0), None);
    assert!(
        !app.trickplay_sheets_loading.contains(&key),
        "a cached absence issues no fetch"
    );
    assert_eq!(http.request_count(), 1, "only the one fetch the test ran");
}

/// A transport failure does not say the server holds no such sheet (task 6.7
/// fix): nothing is cached, the same hover position is not re-asked, and a
/// return to the sheet after another sheet asks again.
#[test]
fn a_transport_failure_is_held_out_of_the_cache_until_the_hover_moves() {
    let http = MockHttp::new();
    http.fail(std::io::ErrorKind::UnexpectedEof);
    http.respond(200, "jpeg");
    http.respond(200, "jpeg");
    let mut app = playing_app_with_grids(vec![grid(320, 10_000, 250)]);
    install_mock_emby(&mut app, &http);
    let sheet0 = sheet_key(320, 0);
    let sheet1 = sheet_key(320, 1);

    fetch_and_drain(&mut app, &sheet0);

    assert!(
        !app.trickplay_sheets.contains_key(&sheet0),
        "a transport failure is not cached as absence"
    );
    assert_eq!(app.trickplay_preview_tile(0), None);
    assert!(
        !app.trickplay_sheets_loading.contains(&sheet0),
        "the same hover position is not asked again"
    );

    // The hover moves on, then returns: only the return asks again.
    assert_eq!(app.trickplay_preview_tile(1000 * TICKS_PER_SECOND), None);
    assert!(app.trickplay_sheets_loading.contains(&sheet1));
    assert_eq!(app.trickplay_preview_tile(0), None);
    assert!(
        app.trickplay_sheets_loading.contains(&sheet0),
        "the return to the failed sheet asks again"
    );
}

/// The cache keeps at most `TRICKPLAY_SHEET_CACHE_BOUND` sheets: the first
/// landed sheet is evicted once a later one lands past the bound.
#[test]
fn cache_evicts_the_oldest_sheet_past_its_bound() {
    let mut app = make_app_stub();
    let sheets =
        u32::try_from(TRICKPLAY_SHEET_CACHE_BOUND).expect("the bound fits a sheet ordinal");
    for sheet in 0..=sheets {
        app.channels
            .trickplay_tx
            .send((sheet_key(320, sheet), TrickplaySheetFetch::Loaded(vec![1])))
            .expect("the stub channel accepts the sheet");
    }

    assert!(app.drain_trickplay_sheet_events());

    assert_eq!(app.trickplay_sheets.len(), TRICKPLAY_SHEET_CACHE_BOUND);
    assert!(!app.trickplay_sheets.contains_key(&sheet_key(320, 0)));
    assert!(app.trickplay_sheets.contains_key(&sheet_key(320, sheets)));
}
