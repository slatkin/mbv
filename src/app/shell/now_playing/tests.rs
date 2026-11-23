//! The workspace now-playing derivation's outcomes: an Emby item match, a
//! pending slot that moves the match, an Audiobookshelf position resolving to
//! a chapter with chapter-relative progress, a pending book resolving to no
//! chapter, and no active item or a different book resolving to nothing.

use super::*;
use crate::app::tests::{emby_unified_state, make_app_stub, make_items};
use mbv_audiobookshelf::{AudiobookshelfChapter, AudiobookshelfLibrary};
use mbv_ctrl::{TransitionSummary, UnifiedQueueSlot, UnifiedQueueStateData};
use mbv_queue::QueueSource;
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState;

fn emby_playing_app(position_ticks: i64) -> App {
    let mut app = make_app_stub();
    app.seed_local_queue_for_test(make_items(2), 0);
    app.seed_playing_position_for_test(0, position_ticks, Some(10_000));
    app
}

/// The queue's confirmed active slot is item 0 while the pending transition
/// targets item 1's slot (101 in `emby_unified_state`).
fn pending_emby_app() -> App {
    let mut snapshot = emby_unified_state(&make_items(2), 0);
    snapshot.in_flight_transition = Some(TransitionSummary {
        request_id: 1,
        generation: 1,
        target_slot: 101,
    });
    let mut app = make_app_stub();
    app.seed_local_queue_state_for_test(&snapshot);
    app.seed_playing_position_for_test(0, 5_000, Some(10_000));
    app
}

fn chapter(id: usize, start: f64, end: f64) -> AudiobookshelfChapter {
    AudiobookshelfChapter {
        id,
        start,
        end,
        title: format!("Chapter {id}"),
    }
}

fn book_queue_item(id: &str) -> AudiobookshelfBookQueueItem {
    AudiobookshelfBookQueueItem {
        library_item_id: id.into(),
        title: "Book".into(),
        author: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        is_finished: false,
        cover_path: None,
    }
}

/// A book-browse state caching `cached_id`'s chapters (0-100 / 100-200 /
/// 200-300 seconds on the merged timeline), and a playing queue holding
/// `playing_id` as its only item with the player at `position_secs`.
fn book_app(cached_id: &str, playing_id: &str, position_secs: f64) -> App {
    let mut app = make_app_stub();
    let library = AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library);
    state.detail_cache.insert(
        cached_id.into(),
        (
            vec![
                chapter(0, 0.0, 100.0),
                chapter(1, 100.0, 200.0),
                chapter(2, 200.0, 300.0),
            ],
            Vec::new(),
        ),
    );
    app.seed_audiobookshelf_book_browse_for_test(vec![state]);
    app.seed_playback_queue_for_test(
        vec![QueueItem::Audiobookshelf(AudiobookshelfItem::Book(
            book_queue_item(playing_id),
        ))],
        0,
    );
    app.seed_playing_position_for_test(
        0,
        mbv_emby_model::seconds_to_ticks(position_secs),
        Some(mbv_emby_model::seconds_to_ticks(300.0)),
    );
    app
}

/// A confirmed Emby slot and a pending book slot; the playback owner still
/// reports the Emby item, so the book selection is unconfirmed.
fn pending_book_app() -> App {
    let mut app = make_app_stub();
    let library = AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library);
    state.detail_cache.insert(
        "book-a".into(),
        (
            vec![chapter(0, 0.0, 100.0), chapter(1, 100.0, 200.0)],
            Vec::new(),
        ),
    );
    app.seed_audiobookshelf_book_browse_for_test(vec![state]);
    let snapshot = UnifiedQueueStateData {
        queue: None,
        start_slot: None,
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot: Some(100),
        slots: vec![
            UnifiedQueueSlot {
                slot_id: 100,
                item: QueueItem::Emby(Box::new(make_items(1).remove(0))),
            },
            UnifiedQueueSlot {
                slot_id: 101,
                item: QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book_queue_item(
                    "book-a",
                ))),
            },
        ],
        revision: 1,
        source: QueueSource::Unknown,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: Some(TransitionSummary {
            request_id: 1,
            generation: 1,
            target_slot: 101,
        }),
        queued_latest_transition: None,
    };
    app.seed_local_queue_state_for_test(&snapshot);
    app.seed_playing_position_for_test(0, 5_000, Some(10_000));
    app
}

fn inactive_player_app() -> App {
    let mut app = make_app_stub();
    app.seed_local_queue_for_test(make_items(2), 0);
    app.seed_idle_player_for_test();
    app
}

#[test]
fn emby_item_matches_with_clamped_progress() {
    let app = emby_playing_app(2_500);
    assert_eq!(
        derive_workspace_now_playing(&app),
        Some(WorkspaceNowPlaying::Item {
            item_id: ItemId::new("id0"),
            progress: Some(25),
        })
    );
}

#[test]
fn pending_slot_moves_the_match() {
    let app = pending_emby_app();
    assert_eq!(
        derive_workspace_now_playing(&app),
        Some(WorkspaceNowPlaying::Item {
            item_id: ItemId::new("id1"),
            progress: None,
        })
    );
}

#[test]
fn abs_position_resolves_to_chapter_with_chapter_relative_progress() {
    let app = book_app("book-a", "book-a", 150.0);
    assert_eq!(
        derive_workspace_now_playing(&app),
        Some(WorkspaceNowPlaying::Chapter {
            library_item_id: "book-a".into(),
            chapter_id: 1,
            progress: 50,
        })
    );
}

#[test]
fn pending_book_resolves_to_no_chapter() {
    let app = pending_book_app();
    assert_eq!(derive_workspace_now_playing(&app), None);
}

#[test]
fn no_active_item_resolves_to_nothing() {
    let app = inactive_player_app();
    assert_eq!(derive_workspace_now_playing(&app), None);
}

#[test]
fn uncached_book_resolves_to_nothing() {
    let app = book_app("book-a", "book-z", 150.0);
    assert_eq!(derive_workspace_now_playing(&app), None);
}
