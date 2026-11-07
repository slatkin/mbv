//! Tests for the scrubber preview's trickplay tile sheets
//! (`add-tauri-gui-client` task 6.7): the position resolves to the smallest
//! offered grid's cached sheet tile, and the cache keeps its bound.

use super::*;

use mbv_emby_model::TICKS_PER_SECOND;

use crate::app::app_stub::make_app_stub;
use crate::app::tests::QueueViewTestExt;

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
            .send((sheet_key(320, sheet), Some(vec![1])))
            .expect("the stub channel accepts the sheet");
    }

    assert!(app.drain_trickplay_sheet_events());

    assert_eq!(app.trickplay_sheets.len(), TRICKPLAY_SHEET_CACHE_BOUND);
    assert!(!app.trickplay_sheets.contains_key(&sheet_key(320, 0)));
    assert!(app.trickplay_sheets.contains_key(&sheet_key(320, sheets)));
}
