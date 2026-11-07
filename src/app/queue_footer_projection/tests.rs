//! The footer's session marker names the shown queue's Player owner and
//! carries whether that owner is live (`add-tauri-gui-client` design D4a: the
//! dot marks the live Owner). The fact is the shown link's own connection
//! state; it needs no second liveness store.
//!
//! The library-context line (`align-gui-with-design-canvas` row U2.2) derives
//! the canvas footer's `Peep Show · Series 1–6` from the queue's own episodes.

use mbv_emby_model::test_support::make_item;
use mbv_queue::QueueItem;

use crate::app::QueueView;
use crate::app::tests::make_app_stub;

/// The owner's liveness follows the shown link's connection: the stub's
/// Owner link is up, and a lost link reports the Owner gone.
#[test]
fn the_footer_owner_is_live_while_its_link_is_up() {
    let app = make_app_stub();
    assert!(
        app.queue_footer_projection().owner_live,
        "a live Owner link marks the owner live"
    );

    app.player.remote().set_disconnected_for_test(true);
    assert!(
        !app.queue_footer_projection().owner_live,
        "a lost Owner link marks the owner not live"
    );
}

/// The canvas footer's library context names the active series and its season
/// range, joined with an en dash (`Peep Show · Series 1–6`, audit/main.md 27).
#[test]
fn an_episodic_queue_names_the_active_series_and_its_season_range() {
    let app = app_with_queue(
        vec![
            episode("Premiere", "Peep Show", 1),
            episode("Wedding", "Peep Show", 6),
            episode("Seasonal Beatings", "Peep Show", 6),
        ],
        Some(1),
    );
    assert_eq!(
        app.queue_footer_projection().library_context.as_deref(),
        Some("Peep Show · Series 1–6")
    );
}

/// A queue whose items carry no seasons paints no library context; the
/// playlist pill stays the footer line's fallback (design DR6).
#[test]
fn a_music_queue_carries_no_library_context() {
    let app = app_with_queue(
        vec![QueueItem::Emby(Box::new(make_item("Track One", "Audio")))],
        Some(1),
    );
    assert_eq!(app.queue_footer_projection().library_context, None);
}

/// One Emby episode of `series` in `season`.
fn episode(name: &str, series: &str, season: i64) -> QueueItem {
    let mut item = make_item(name, "Episode");
    item.series_name = series.to_string();
    item.parent_index_number = season;
    QueueItem::Emby(Box::new(item))
}

/// One app whose queue holds `items` in canonical order, with `active_slot`
/// carrying the owner's playhead.
fn app_with_queue(items: Vec<QueueItem>, active_slot: Option<u64>) -> crate::app::App {
    let mut app = make_app_stub();
    let slots = items
        .into_iter()
        .enumerate()
        .map(|(i, item)| mbv_ctrl::UnifiedQueueSlot {
            slot_id: u64::try_from(i).unwrap_or(u64::MAX) + 1,
            item,
        })
        .collect();
    let state = mbv_ctrl::UnifiedQueueStateData {
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot,
        slots,
        revision: 1,
        source: mbv_queue::QueueSource::Unknown,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    app.local_view = QueueView::from_snapshot(&state);
    app
}
