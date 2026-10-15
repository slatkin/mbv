//! The Upcoming list slot's placeholder contract: while the per-session
//! snapshot fetch is in flight the empty list box shows the shared loading
//! placeholder, and after the fetch's retries are exhausted it names the
//! failure with the pill reselect as the visible retry (2026-10-15: the
//! server's metadata refresh blocked the one eager attempt for its whole
//! 30 s budget, leaving a silent empty list).

use super::*;
use mbv_queue::TvContentMode;

fn upcoming_premiere(series: &str) -> mbv_emby_model::UpcomingPremiere {
    mbv_emby_model::UpcomingPremiere {
        series_id: mbv_ids::ItemId::new(series),
        series_name: series.into(),
        season: Some(1),
        episode: Some(1),
        title: "Premiere".into(),
        premiere_date: "2026-10-12T00:00:00.0000000Z".into(),
    }
}

fn upcoming_owner(
    premieres: Vec<mbv_emby_model::UpcomingPremiere>,
    loading: bool,
    failed: bool,
) -> TvContent {
    let mut owner = TvContent::new();
    let mut context = tv_tree_context(Vec::new(), None, None, false);
    context.set_tv_content_mode(Some(TvContentMode::Upcoming));
    context.upcoming = premieres;
    context.upcoming_failed = failed;
    context.list.loading = loading;
    owner.set_content(context);
    owner
}

#[test]
fn the_upcoming_placeholder_names_the_failed_fetch_for_a_reselect_retry() {
    use crate::library_panel::ListSlot;

    let mut owner = upcoming_owner(Vec::new(), false, true);

    match owner.panel_content().list {
        ListSlot::Empty { loading, text } => {
            assert!(!loading, "the failed fetch is no longer in flight");
            assert_eq!(text, " Upcoming fetch failed — select the pill to retry");
        }
        other => panic!("an empty failed Upcoming must use the placeholder slot, got {other:?}"),
    }
}

#[test]
fn the_upcoming_placeholder_shows_loading_while_the_snapshot_fetch_is_in_flight() {
    use crate::library_panel::ListSlot;

    let mut owner = upcoming_owner(Vec::new(), true, false);

    match owner.panel_content().list {
        ListSlot::Empty { loading, text } => {
            assert!(loading);
            assert_eq!(text, " (empty)", "loading paints its own shared wording");
        }
        other => panic!("an empty loading Upcoming must use the placeholder slot, got {other:?}"),
    }
}

/// Regression guard for the 2026-10-15 placeholder work: present premieres
/// keep the canonical media list; only the empty snapshot takes the slot.
#[test]
fn loaded_upcoming_premieres_stay_a_media_slot() {
    use crate::library_panel::ListSlot;

    let mut owner = upcoming_owner(vec![upcoming_premiere("Series 1")], false, false);

    assert!(matches!(owner.panel_content().list, ListSlot::Media(_)));
}

/// A Latest level still fetching shows the shared loading placeholder
/// instead of a blank list box (the tab's non-blocking loading indicator).
#[test]
fn the_latest_placeholder_shows_loading_while_the_level_fetches() {
    use crate::library_panel::ListSlot;

    let mut owner = TvContent::new();
    let mut context = tv_tree_context(Vec::new(), None, None, false);
    context.set_tv_content_mode(Some(TvContentMode::Latest));
    context.list.loading = true;
    owner.set_content(context);

    match owner.panel_content().list {
        ListSlot::Empty { loading, text } => {
            assert!(loading);
            assert_eq!(text, " (empty)");
        }
        other => panic!("an empty loading Latest must use the placeholder slot, got {other:?}"),
    }
}
