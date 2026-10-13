//! The workspace now-playing derivation (hero-workspace-now-playing row 1.1):
//! one `#[case]` table over the derivation's outcomes — an Emby item match, a
//! pending slot that moves the match, an Audiobookshelf position resolving to
//! a chapter with chapter-relative progress, a pending book resolving to no
//! chapter, and no active item or a different book resolving to nothing.

use super::*;
use crate::app::QueueView;
use crate::app::tests::{QueueViewTestExt, emby_unified_state, make_app_stub, make_items};
use mbv_audiobookshelf::{AudiobookshelfChapter, AudiobookshelfLibrary};
use mbv_ctrl::{TransitionSummary, UnifiedQueueSlot, UnifiedQueueStateData};
use mbv_queue::QueueSource;
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState;
use rstest::rstest;

fn emby_playing_app(current_idx: usize, position_ticks: i64) -> App {
    let mut app = make_app_stub();
    app.local_view.adopt_items(make_items(2), 0);
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = current_idx;
        status.position_ticks = position_ticks;
        status.runtime_ticks = 10_000;
    });
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
    app.local_view = QueueView::from_snapshot(&snapshot);
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.position_ticks = 5_000;
        status.runtime_ticks = 10_000;
    });
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
    app.audiobookshelf_book_browse.push(state);
    app.playback_queue_mut().adopt_queue_items(
        vec![QueueItem::Audiobookshelf(AudiobookshelfItem::Book(
            book_queue_item(playing_id),
        ))],
        0,
    );
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.position_ticks = mbv_emby_model::seconds_to_ticks(position_secs);
        status.runtime_ticks = mbv_emby_model::seconds_to_ticks(300.0);
    });
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
    app.audiobookshelf_book_browse.push(state);
    let snapshot = UnifiedQueueStateData {
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
    app.local_view = QueueView::from_snapshot(&snapshot);
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.position_ticks = 5_000;
        status.runtime_ticks = 10_000;
    });
    app
}

fn inactive_player_app() -> App {
    let mut app = make_app_stub();
    app.local_view.adopt_items(make_items(2), 0);
    app.player.update_status(|status| {
        status.active = false;
    });
    app
}

fn scenario_emby_item_matches_with_clamped_progress() {
    let app = emby_playing_app(0, 2_500);
    assert_eq!(
        derive_workspace_now_playing(&app),
        Some(WorkspaceNowPlaying::Item {
            content_id: QueueItemContentId::Emby("id0".into()),
            progress: Some(25),
        })
    );
}

fn scenario_pending_slot_moves_the_match() {
    let app = pending_emby_app();
    assert_eq!(
        derive_workspace_now_playing(&app),
        Some(WorkspaceNowPlaying::Item {
            content_id: QueueItemContentId::Emby("id1".into()),
            progress: None,
        })
    );
}

fn scenario_abs_position_resolves_to_chapter_with_chapter_relative_progress() {
    let app = book_app("book-a", "book-a", 150.0);
    assert_eq!(
        derive_workspace_now_playing(&app),
        Some(WorkspaceNowPlaying::Chapter {
            content_id: QueueItemContentId::AudiobookshelfBook {
                library_item_id: "book-a".into(),
            },
            chapter_id: 1,
            progress: 50,
        })
    );
}

fn scenario_pending_book_resolves_to_no_chapter() {
    let app = pending_book_app();
    assert_eq!(derive_workspace_now_playing(&app), None);
}

fn scenario_no_active_item_or_different_book_resolves_to_nothing() {
    let inactive = inactive_player_app();
    assert_eq!(derive_workspace_now_playing(&inactive), None);
    let uncached = book_app("book-a", "book-z", 150.0);
    assert_eq!(derive_workspace_now_playing(&uncached), None);
}

#[rstest]
#[case::emby_item_matches_with_clamped_progress(|| scenario_emby_item_matches_with_clamped_progress())]
#[case::pending_slot_moves_the_match(|| scenario_pending_slot_moves_the_match())]
#[case::abs_position_resolves_to_chapter_with_chapter_relative_progress(|| {
    scenario_abs_position_resolves_to_chapter_with_chapter_relative_progress();
})]
#[case::pending_book_resolves_to_no_chapter(|| scenario_pending_book_resolves_to_no_chapter())]
#[case::no_active_item_or_different_book_resolves_to_nothing(|| {
    scenario_no_active_item_or_different_book_resolves_to_nothing();
})]
fn workspace_now_playing_derivation(#[case] scenario: fn()) {
    scenario();
}
