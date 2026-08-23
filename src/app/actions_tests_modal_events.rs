#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::tests::make_item;
use crate::app::{BrowseLevel, LibEvent, LibraryTab, TabSelection};
use mbv_core::api::EmbyItem;
use std::collections::HashMap;

fn folder(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "Folder");
    item.id = id.into();
    item.is_folder = true;
    item
}

// ── album_tracks_cache / LibEvent::AlbumTracksFetched (#145) ────────────
// Proactive track-list fetch/cache for the inline album
// detail pane, mirroring the existing `album_artist_cache` pattern.

#[test]
fn album_tracks_fetched_event_populates_cache_and_clears_loading() {
    use crate::app::tests::make_item;

    let mut app = crate::app::tests::make_app_stub();
    app.album_tracks_loading.insert("album-1".into());

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    app.handle_lib_event(LibEvent::AlbumTracksFetched {
        album_id: "album-1".into(),
        tracks: vec![track],
    });

    assert!(
        !app.album_tracks_loading.contains("album-1"),
        "the loading marker must be cleared once the fetch resolves"
    );
    let cached = app
        .album_tracks_cache
        .get("album-1")
        .expect("fetched tracks must be cached under the album id");
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].id, "track-1");
}
#[test]
fn album_tracks_completion_refreshes_matching_open_modal() {
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Album {
            album_id: "album-1".into(),
        },
        title: "Tracks".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });
    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();

    app.handle_lib_event(LibEvent::AlbumTracksFetched {
        album_id: "album-1".into(),
        tracks: vec![track],
    });

    let modal = app.selection_modal.as_ref().expect("modal stays open");
    assert!(matches!(modal.state, SelectionModalListState::Ready(_)));
    assert_eq!(modal.state.rows()[0].item_id(), Some("track-1"));
}

#[test]
fn album_tracks_completion_does_not_refresh_a_different_open_modal() {
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Album {
            album_id: "album-2".into(),
        },
        title: "Tracks".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });

    app.handle_lib_event(LibEvent::AlbumTracksFetched {
        album_id: "album-1".into(),
        tracks: vec![make_item("Track", "Audio")],
    });

    assert!(matches!(
        app.selection_modal
            .as_ref()
            .expect("modal stays open")
            .state,
        SelectionModalListState::Loading
    ));
}

#[test]
fn series_detail_completion_refreshes_matching_open_modal() {
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Series {
            series_id: "series-1".into(),
        },
        title: "Series".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    let mut episodes = HashMap::new();
    episodes.insert("season-1".into(), vec![episode]);

    app.handle_lib_event(LibEvent::SeriesDetailFetched {
        series_id: "series-1".into(),
        seasons: vec![season],
        episodes,
    });

    let modal = app.selection_modal.as_ref().expect("modal stays open");
    assert!(matches!(modal.state, SelectionModalListState::Ready(_)));
    assert_eq!(modal.state.rows()[1].item_id(), Some("episode-1"));
}

#[test]
fn series_detail_completion_does_not_refresh_a_different_open_modal() {
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Series {
            series_id: "series-2".into(),
        },
        title: "Series".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });

    app.handle_lib_event(LibEvent::SeriesDetailFetched {
        series_id: "series-1".into(),
        seasons: Vec::new(),
        episodes: HashMap::new(),
    });

    assert!(matches!(
        app.selection_modal
            .as_ref()
            .expect("modal stays open")
            .state,
        SelectionModalListState::Loading
    ));
}

#[test]
fn series_detail_completion_keeps_uncached_seasons_loading() {
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalFilter, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Series {
            series_id: "series-1".into(),
        },
        title: "Series".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: Some(SelectionModalFilter {
            labels: vec!["01".into(), "02".into()],
            selected: 1,
        }),
    });
    let mut season_one = make_item("Season 1", "Season");
    season_one.id = "season-1".into();
    season_one.index_number = 1;
    let mut season_two = make_item("Season 2", "Season");
    season_two.id = "season-2".into();
    season_two.index_number = 2;
    let mut episodes = HashMap::new();
    episodes.insert("season-1".into(), vec![make_item("Episode 1", "Episode")]);

    app.handle_lib_event(LibEvent::SeriesDetailFetched {
        series_id: "series-1".into(),
        seasons: vec![season_one, season_two],
        episodes,
    });

    assert!(matches!(
        app.selection_modal
            .as_ref()
            .expect("modal stays open")
            .state,
        SelectionModalListState::Loading
    ));
}

#[test]
fn series_season_completion_refreshes_the_selected_modal_pill_in_place() {
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalFilter, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    let mut season_one = make_item("Season 1", "Season");
    season_one.id = "season-1".into();
    let mut season_two = make_item("Season 2", "Season");
    season_two.id = "season-2".into();
    let mut episodes = HashMap::new();
    episodes.insert("season-1".into(), vec![make_item("Pilot", "Episode")]);
    app.series_detail_cache.insert(
        "series-1".into(),
        crate::app::SeriesDetail {
            seasons: vec![season_one, season_two],
            episodes,
        },
    );
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Series {
            series_id: "series-1".into(),
        },
        title: "Series".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: Some(SelectionModalFilter {
            labels: vec!["01".into(), "02".into()],
            selected: 1,
        }),
    });

    let mut finale = make_item("Finale", "Episode");
    finale.id = "episode-2".into();
    app.handle_lib_event(LibEvent::SeriesSeasonEpisodesFetched {
        series_id: "series-1".into(),
        season_id: "season-2".into(),
        episodes: vec![finale],
    });

    let modal = app.selection_modal.as_ref().expect("modal stays open");
    assert_eq!(modal.filter.as_ref().unwrap().selected, 1);
    assert!(matches!(modal.state, SelectionModalListState::Ready(_)));
    assert_eq!(modal.state.rows()[1].item_id(), Some("episode-2"));
}

#[test]
fn album_modal_opens_loading_while_existing_track_fetch_is_in_flight() {
    use crate::app::types_selection_modal::SelectionModalListState;

    let mut app = crate::app::tests::make_app_stub();
    app.album_tracks_loading.insert("album-1".into());
    let mut album = make_item("Album", "MusicAlbum");
    album.id = "album-1".into();

    app.open_album_selection_modal(&album);

    let modal = app.selection_modal.as_ref().expect("modal open");
    assert!(matches!(modal.state, SelectionModalListState::Loading));
    assert!(app.album_tracks_loading.contains("album-1"));
}

#[test]
fn album_tracks_completion_replaces_loading_with_sorted_rows() {
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Album {
            album_id: "album-1".into(),
        },
        title: "Album".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });
    let mut second = make_item("Second", "Audio");
    second.id = "track-2".into();
    second.index_number = 2;
    let mut first = make_item("First", "Audio");
    first.id = "track-1".into();
    first.index_number = 1;

    app.handle_lib_event(LibEvent::AlbumTracksFetched {
        album_id: "album-1".into(),
        tracks: vec![second, first],
    });

    let modal = app.selection_modal.as_ref().expect("modal stays open");
    assert_eq!(modal.state.rows()[0].item_id(), Some("track-1"));
    assert_eq!(modal.state.rows()[1].item_id(), Some("track-2"));

    app.handle_lib_event(LibEvent::AlbumTracksFetched {
        album_id: "album-1".into(),
        tracks: Vec::new(),
    });
    assert!(matches!(
        app.selection_modal
            .as_ref()
            .expect("modal stays open")
            .state,
        SelectionModalListState::Empty
    ));
}

#[test]
fn series_season_completion_refreshes_matching_open_modal() {
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    app.series_detail_cache.insert(
        "series-1".into(),
        crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: HashMap::new(),
        },
    );
    app.series_season_loading
        .insert(("series-1".into(), "season-1".into()));
    app.series_detail_loading.insert("series-1".into());
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Series {
            series_id: "series-1".into(),
        },
        title: "Series".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();

    app.handle_lib_event(LibEvent::SeriesSeasonEpisodesFetched {
        series_id: "series-1".into(),
        season_id: "season-1".into(),
        episodes: vec![episode],
    });

    let modal = app.selection_modal.as_ref().expect("modal stays open");
    assert!(matches!(modal.state, SelectionModalListState::Ready(_)));
    assert_eq!(modal.state.rows()[1].item_id(), Some("episode-1"));
    assert!(!app
        .series_season_loading
        .contains(&("series-1".into(), "season-1".into())));
    assert!(!app.series_detail_loading.contains("series-1"));
}

#[test]
fn series_season_completion_does_not_refresh_a_different_open_modal() {
    use crate::app::types_selection_modal::{
        SelectionModal, SelectionModalListState, SelectionModalSource,
    };

    let mut app = crate::app::tests::make_app_stub();
    app.selection_modal = Some(SelectionModal {
        source: SelectionModalSource::Series {
            series_id: "series-2".into(),
        },
        title: "Series".into(),
        state: SelectionModalListState::Loading,
        cursor: 0,
        filter: None,
    });
    app.handle_lib_event(LibEvent::SeriesSeasonEpisodesFetched {
        series_id: "series-1".into(),
        season_id: "season-1".into(),
        episodes: Vec::new(),
    });

    assert!(matches!(
        app.selection_modal
            .as_ref()
            .expect("modal stays open")
            .state,
        SelectionModalListState::Loading
    ));
}

#[test]
fn series_season_fetch_suppresses_duplicate_in_flight_requests() {
    let mut app = crate::app::tests::make_app_stub();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    app.series_detail_cache.insert(
        "series-1".into(),
        crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: HashMap::new(),
        },
    );
    app.series_season_loading
        .insert(("series-1".into(), "season-1".into()));

    app.fetch_series_season_episodes("series-1".into(), "season-1".into());

    assert!(app
        .series_season_loading
        .contains(&("series-1".into(), "season-1".into())));
    assert!(matches!(
        app.lib_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
}

#[test]
fn series_season_fetch_producers_require_detail_cache_before_emitting_completion() {
    let mut app = crate::app::tests::make_app_stub();
    let mut library = folder("tv-library", "TV");
    library.collection_type = "tvshows".into();
    let mut series = make_item("Series 1", "Series");
    series.id = "series-1".into();
    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
            parent_id: "root".into(),
            title: "TV".into(),
            items: vec![series],
            total_count: 1,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        feed_home_video: None,
        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    // Both production season-fetch entry points must reject a missing detail
    // cache before they can spawn a worker or emit a completion event.
    app.select_series_season(0, 0);
    app.switch_series_selection_season(0, 1);
    assert!(matches!(
        app.lib_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
}
