//! Bounded browse caches (issue #917): `series_detail_cache` and
//! `album_tracks_cache` used to grow without bound within a session, because
//! only a service replacement `clear()` ever removed an entry. Each fresh
//! insert past the cache's entry cap must evict the first-inserted entry;
//! a re-fetched id gets fresh eviction age.

use crate::app::state::app_struct::{MAX_ALBUM_TRACKS_CACHE, MAX_SERIES_DETAIL_CACHE};
use crate::app::tests::render_fixtures::make_movie_app;
use mbv_emby_model::test_support::make_item;
use mbv_ui_model::browse::SeriesDetail;

fn series_detail(season_id: &str) -> SeriesDetail {
    let mut season = make_item(season_id, "Season");
    season.id = season_id.into();
    SeriesDetail {
        seasons: vec![season],
        episodes: std::collections::HashMap::new(),
    }
}

fn album_track(track_id: &str) -> Vec<mbv_emby_model::EmbyItem> {
    let mut track = make_item(track_id, "Audio");
    track.id = track_id.into();
    vec![track]
}

#[test]
fn fresh_series_details_past_the_cap_evict_the_oldest_entry() {
    let mut app = make_movie_app();
    let last = MAX_SERIES_DETAIL_CACHE;
    for index in 0..=last {
        app.cache_series_detail(&format!("show-{index}"), series_detail("season"));
    }

    assert_eq!(app.series_detail_cache.len(), MAX_SERIES_DETAIL_CACHE);
    assert!(
        !app.series_detail_cache.contains_key("show-0"),
        "the first-inserted series detail is evicted once the cap is reached"
    );
    assert!(
        app.series_detail_cache
            .contains_key(&format!("show-{last}"))
    );
}

#[test]
fn fresh_album_track_lists_past_the_cap_evict_the_oldest_entry() {
    let mut app = make_movie_app();
    let last = MAX_ALBUM_TRACKS_CACHE;
    for index in 0..=last {
        app.cache_album_tracks(format!("album-{index}"), album_track("track-1"));
    }

    assert_eq!(app.album_tracks_cache.len(), MAX_ALBUM_TRACKS_CACHE);
    assert!(
        !app.album_tracks_cache.contains_key("album-0"),
        "the first-inserted album track list is evicted once the cap is reached"
    );
    assert!(
        app.album_tracks_cache
            .contains_key(&format!("album-{last}"))
    );
}

#[test]
fn refetching_a_cached_album_moves_it_to_the_back_of_the_eviction_order() {
    let mut app = make_movie_app();
    for index in 0..MAX_ALBUM_TRACKS_CACHE {
        app.cache_album_tracks(format!("album-{index}"), album_track("track-1"));
    }
    // Record the re-fetched album's identity so the later assertion cannot
    // silently check a value from the first insert.
    app.cache_album_tracks("album-1".into(), album_track("track-refetched"));

    // One more fresh insert exceeds the cap and evicts the oldest id; the
    // re-fetched album must survive it.
    app.cache_album_tracks(
        format!("album-{MAX_ALBUM_TRACKS_CACHE}"),
        album_track("track-1"),
    );

    assert!(
        !app.album_tracks_cache.contains_key("album-0"),
        "the unrevisited oldest album is evicted"
    );
    assert_eq!(
        app.album_tracks_cache["album-1"][0].id, "track-refetched",
        "the re-fetched album keeps its fresh list and fresh eviction age"
    );
}

#[test]
fn a_late_series_detail_completion_keeps_the_cached_detail_and_its_eviction_slot() {
    // Precondition of the no-replace contract also pinned by
    // `late_series_detail_completion_does_not_replace_cached_detail`:
    // `cache_series_detail` must not reorder or replace an existing entry.
    let mut app = make_movie_app();
    app.cache_series_detail("show-1", series_detail("cached-season"));
    app.cache_series_detail("show-1", series_detail("late-season"));

    assert_eq!(
        app.series_detail_cache["show-1"].seasons[0].id, "cached-season",
        "a late completion must not replace the cached projection"
    );
    assert_eq!(
        app.series_detail_cache_order,
        std::collections::VecDeque::from(["show-1".to_string()]),
        "the cache stays bounded without a duplicate order slot"
    );
}
