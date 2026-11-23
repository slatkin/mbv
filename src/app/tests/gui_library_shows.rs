//! Tests of the GUI Library's Shows projection (add-gui-library-panel
//! design D14; gui-collection-dropdown design D8): the series list's tree —
//! the series nodes with their rail letters, their seasons, and each season's
//! episodes or `None` while unloaded — the series Hero from the TUI's series
//! producer with its season pills and Workspace, and the stale selection's
//! first-row fallback. The projection reads the seeded `App` data alone, so no
//! Service, worker, or mock transport runs.

use super::*;
use crate::app::gui_library::{GuiSectionRequest, GuiShowsNewEvent};
use crate::config::TestStateDirGuard;
use mbv_queue::TvContentMode;
use mbv_ui_model::media_list::{ActiveProgress, MediaSemanticState};
use rstest::rstest;

/// A series row of the series list, with its sort name.
fn series_item(id: &str, name: &str, sort_name: &str) -> EmbyItem {
    let mut item = make_item(name, "Series");
    item.id = id.to_string();
    item.sort_name = sort_name.to_string();
    item
}

/// A browse level over `items`, as the series list's root level.
fn level(items: Vec<EmbyItem>) -> mbv_ui_model::browse::BrowseLevel {
    mbv_ui_model::browse::BrowseLevel {
        parent_id: "lib-tv".to_string(),
        title: "Shows".to_string(),
        items,
        rows: mbv_ui_model::browse::ServerRows::new(0),
        resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".to_string(),
        sort_order: "Ascending".to_string(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

/// A stub `App` whose Shows library is the active destination, with no root
/// level yet — the shape a launch restored onto Shows presents.
fn empty_shows_app() -> App {
    let mut app = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    app.libs.push(crate::app::LibraryTab::new(library));
    app.tab = TabSelection::EmbyLibrary(0);
    app
}

/// A stub `App` whose Shows library is the active destination, with the
/// series list's root level.
fn shows_app(items: Vec<EmbyItem>) -> App {
    let mut app = empty_shows_app();
    app.libs[0].nav_stack.push(level(items));
    app
}

/// The All series mode's fixture (design D14): two root series, the first
/// with a cached detail whose first season has loaded episodes and whose
/// second season has not, and a selected series in the App's Shows selection.
fn series_tree_app() -> App {
    let mut first = series_item("series-a", "Alfred", "Alfred");
    first.image_tags.thumb = "tag".into();
    first.genres = vec!["Drama".into()];
    first.production_year = 2013;
    first.end_year = 2022;
    first.studios = vec!["FX".into()];
    let second = series_item("series-b", "Bridgerton", "Bridgerton");
    let mut app = shows_app(vec![first, second]);
    let mut played = make_item("Pilot", "Episode");
    played.id = "ep-1".into();
    played.index_number = 1;
    played.played = true;
    let mut resumed = make_item("Second", "Episode");
    resumed.id = "ep-2".into();
    resumed.index_number = 2;
    resumed.playback_position_ticks = 300 * mbv_emby_model::TICKS_PER_SECOND;
    resumed.runtime_ticks = 1200 * mbv_emby_model::TICKS_PER_SECOND;
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut later = make_item("Season 2", "Season");
    later.id = "season-2".into();
    let detail = SeriesDetail {
        seasons: vec![season, later],
        episodes: [("season-1".to_string(), vec![played, resumed])]
            .into_iter()
            .collect(),
    };
    app.series_detail_cache
        .insert("series-a".to_string(), detail);
    app
}

/// The All series mode's tree (design D14): the nodes are the root level's
/// series in the server's `SortName` order, each with its rail letter over the
/// sort name and its seasons from the cached detail — the loaded season's
/// episode rows and `None` for a season whose load has not landed — and the
/// first series is the Hero default.
#[test]
fn the_series_mode_projects_the_tree() {
    let _guard = TestStateDirGuard::new();
    let mut app = series_tree_app();

    let projection = crate::app::tests::gui_shows_projection(&mut app, 0);

    let ids: Vec<&str> = projection
        .series
        .iter()
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["series-a", "series-b"],
        "the tree's nodes are the root series"
    );
    let letters: Vec<char> = projection
        .series
        .iter()
        .map(|node| node.sort_letter)
        .collect();
    assert_eq!(letters, ['A', 'B'], "the rail letters read the sort names");
    assert_eq!(
        projection.selected.as_deref(),
        Some("series-a"),
        "the first series node is the Hero default"
    );
    let tree = &projection.series[0];
    assert!(tree.expandable, "a series with a season is expandable");
    let seasons: Vec<&str> = tree
        .seasons
        .iter()
        .map(|season| season.id.as_str())
        .collect();
    assert_eq!(seasons, ["season-1", "season-2"]);
    assert_eq!(
        tree.seasons[0].episodes.as_ref().map(Vec::len),
        Some(2),
        "the loaded season carries its episode rows"
    );
    assert!(
        tree.seasons[1].episodes.is_none(),
        "an unloaded season carries None, so its open asks for the load"
    );
    assert!(
        projection.series[1].seasons.is_empty() && projection.series[1].expandable,
        "a series without a cached detail paints closed and expandable"
    );
}

/// PR 1021 regression: presenting Shows with an empty root starts the series
/// root load from the frame ensure, without the `OpenDestination` click that was
/// the only trigger. A launch restored onto Shows paints an empty list until
/// that click. Two frame passes must leave one loading root, so the trigger
/// does not fire again while the request is in flight.
#[test]
fn presenting_an_empty_shows_root_starts_the_root_load_once_without_an_open_destination_click() {
    let _guard = TestStateDirGuard::new();
    let mut app = empty_shows_app();

    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Shows { lib_idx: 0 });
    crate::app::tests::ensure_gui_destination(&mut app, GuiDestinationKind::Shows { lib_idx: 0 });

    assert_eq!(app.libs[0].nav_stack.len(), 1, "one root level is started");
    let root = &app.libs[0].nav_stack[0];
    assert!(
        root.loading,
        "the root level is loading, so the next frame does not re-request it"
    );
    assert_eq!(
        root.tv_content_mode.as_ref(),
        Some(&TvContentMode::All),
        "the root load is the All series list the Shows destination presents"
    );
}

/// The series Hero comes from the TUI's series producer (design D14): the
/// genre, the show's year range, the season count, and the network, with no
/// duration row while the Workspace has no focus — and the Workspace holds the
/// season pills on the first season with the episodes' played/progress states.
#[test]
fn the_series_hero_carries_the_tui_producers_facts_and_the_workspace() {
    let _guard = TestStateDirGuard::new();
    let mut app = series_tree_app();

    let projection = crate::app::tests::gui_shows_projection(&mut app, 0);

    let hero = projection.hero.expect("the series Hero paints");
    assert_eq!(hero.title, "Alfred");
    assert_eq!(
        mbv_ui_model::meta::meta_rows_text(&hero.meta_rows),
        vec!["DRAMA", "2013 - 2022", "2 Seasons", "FX"],
        "the Hero carries the TUI series producer's rows"
    );
    assert_eq!(
        hero.duration_row, None,
        "the unfocused Workspace leaves the series without a duration row"
    );
    assert_eq!(
        hero.art_shape,
        crate::app::gui_library::GuiArtShape::Portrait,
        "the series art is its Primary poster, not the landscape chain"
    );
    assert!(hero.art.is_some(), "the declared poster fetches its art");
    let workspace = hero
        .workspace
        .expect("the series Hero carries its Workspace");
    assert_eq!(
        workspace.selected_season.as_deref(),
        Some("season-1"),
        "the season pills start on the first season"
    );
    assert_eq!(workspace.seasons.len(), 2);
    assert!(workspace.seasons[0].selected);
    let states: Vec<MediaSemanticState> = workspace
        .episodes
        .iter()
        .map(|row| row.state.clone())
        .collect();
    assert_eq!(
        states,
        vec![
            MediaSemanticState::Played,
            MediaSemanticState::Active {
                progress: Some(ActiveProgress::new(25))
            },
        ],
        "the Workspace rows carry the played/progress states"
    );
}

/// A stale selection falls back to the first row (design D2, D12): a stored
/// id the series list no longer carries selects the list's first series, so
/// the Hero never names a row the frame did not paint.
#[test]
fn a_stale_selection_falls_back_to_the_first_row() {
    let _guard = TestStateDirGuard::new();
    let mut app = shows_app(vec![
        series_item("series-a", "Alfred", "Alfred"),
        series_item("series-b", "Bridgerton", "Bridgerton"),
    ]);
    app.gui.tv_shows.entry(0).or_default().series_id = Some("series-gone".into());

    let projection = crate::app::tests::gui_shows_projection(&mut app, 0);

    assert_eq!(
        projection.selected.as_deref(),
        Some("series-a"),
        "the stale selection fell back to the first series row"
    );
    assert_eq!(
        projection.hero.expect("the Hero paints").title,
        "Alfred",
        "the Hero follows the fallback row"
    );
}

/// The Hero's art box follows the fetched image's own pixels, not only the
/// policy's declared arm (design D5's painted-shape rule, which the TUI's
/// `HeroArtwork::painted_shape` re-arms on). A series declares the portrait
/// arm, and when its Primary image is a landscape still the box must take the
/// image's arm: otherwise the Hero paints a landscape still inside a 3:4 box,
/// which is the GUI/TUI divergence this guards.
#[test]
fn the_series_heroes_art_box_follows_the_fetched_images_own_aspect() {
    let _guard = TestStateDirGuard::new();
    let mut first = series_item("series-a", "Alfred", "Alfred");
    first.image_tags.thumb = "tag".into();
    let mut app = shows_app(vec![first]);
    let still = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        450,
        300,
        image::Rgba([20, 40, 60, 255]),
    ));
    app.images.insert_image(
        mbv_images::series_image_cache_key("series-a", &["Primary"]),
        mbv_images::CachedImage {
            source: Some(crate::app::queue_card_artwork::test_source(&still)),
            img: Some(still),
            cover_box: None,
            applied_logo_key: None,
        },
    );

    let projection = crate::app::tests::gui_shows_projection(&mut app, 0);

    let hero = projection.hero.expect("the series Hero paints");
    assert!(hero.art.is_some(), "the seeded image is the Hero's art");
    assert_eq!(
        hero.art_shape,
        crate::app::gui_library::GuiArtShape::Landscape,
        "the box follows the fetched still's 3:2 pixels, not the declared portrait arm"
    );
}

/// One episode of a TV library's New list, dated by `date_added` as the TV
/// Latest fetch returns it.
fn new_episode(id: &str, name: &str, date_added: &str) -> EmbyItem {
    let mut item = make_item(name, "Episode");
    item.id = id.to_string();
    item.series_name = "Show".into();
    item.date_added = date_added.into();
    item
}

/// One Upcoming premiere of a TV library.
fn premiere(
    series_id: &str,
    series_name: &str,
    premiere_date: &str,
) -> mbv_emby_model::UpcomingPremiere {
    mbv_emby_model::UpcomingPremiere {
        series_id: mbv_ids::ItemId::new(series_id),
        series_name: series_name.into(),
        season: Some(1),
        episode: Some(2),
        title: "The Episode".into(),
        premiere_date: premiere_date.into(),
    }
}

/// Contract (design D8): a press on one view replaces the active choice, and
/// the collection stores have no Shows entry.
#[test]
fn one_shows_view_is_active_at_a_time() {
    let _guard = TestStateDirGuard::new();
    let mut app = empty_shows_app();

    assert_eq!(gui_shows_view(&app, 0), GuiShowsView::All);
    for view in [GuiShowsView::New, GuiShowsView::Upcoming, GuiShowsView::All] {
        apply_gui_library_command(
            &mut app,
            GuiLibraryCommand::ShowsSetView { lib_idx: 0, view },
        );
        assert_eq!(gui_shows_view(&app, 0), view);
    }
    assert!(app.gui.shows_views.contains_key(&0));
}

/// Contract (design D9): New paints the merged latest episodes, newest first,
/// with the title split and Hero the old Home section carried.
#[test]
fn the_new_view_paints_the_merged_latest_episodes() {
    let _guard = TestStateDirGuard::new();
    let mut app = empty_shows_app();
    app.gui.shows_new.insert(
        "lib-a".into(),
        GuiSectionRequest::Landed(vec![
            new_episode("ep-old", "Old", "2026-11-01T00:00:00.0000000Z"),
            new_episode("ep-new", "New", "2026-11-10T00:00:00.0000000Z"),
        ]),
    );
    app.gui.shows_new.insert(
        "lib-b".into(),
        GuiSectionRequest::Landed(vec![
            new_episode("ep-mid", "Mid", "2026-11-05T00:00:00.0000000Z"),
            new_episode("ep-new", "New", "2026-11-10T00:00:00.0000000Z"),
        ]),
    );
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowsSetView {
            lib_idx: 0,
            view: GuiShowsView::New,
        },
    );

    let projection = crate::app::tests::gui_shows_projection(&mut app, 0);

    let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, ["ep-new", "ep-mid", "ep-old"]);
    assert!(projection.rows.iter().all(|row| !row.display_only));
    assert_eq!(projection.rows[0].title, "Show");
    assert_eq!(projection.rows[0].subtitle.as_deref(), Some("New"));
    assert_eq!(projection.selected.as_deref(), Some("ep-new"));
    assert_eq!(projection.hero.expect("the Hero paints").title, "New");
}

/// Contract (design D9): Upcoming paints the merged premieres, soonest first,
/// and every row is display-only.
#[test]
fn the_upcoming_view_paints_display_only_premieres() {
    let _guard = TestStateDirGuard::new();
    let mut app = empty_shows_app();
    let later = premiere("series-a", "Show A", "2026-12-01T00:00:00.0000000Z");
    let sooner = premiere("series-b", "Show B", "2026-11-20T00:00:00.0000000Z");
    app.tv_upcoming_snapshots
        .insert("lib-a".into(), vec![later.clone()]);
    app.tv_upcoming_snapshots
        .insert("lib-b".into(), vec![sooner.clone()]);
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowsSetView {
            lib_idx: 0,
            view: GuiShowsView::Upcoming,
        },
    );

    let projection = crate::app::tests::gui_shows_projection(&mut app, 0);

    let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, [sooner.stable_key(), later.stable_key()]);
    assert!(projection.rows.iter().all(|row| row.display_only));
    assert_eq!(projection.rows[0].title, "Show B");
    assert_eq!(
        projection.rows[0].subtitle.as_deref(),
        Some("S01:E02 — The Episode")
    );
}

/// Contract (design D9): a play or enqueue command on an Upcoming row acts on
/// nothing, so the Owner's queue stays untouched.
#[test]
fn an_upcoming_row_resolves_no_playable_item() {
    let _guard = TestStateDirGuard::new();
    let mut app = empty_shows_app();
    app.tv_upcoming_snapshots.insert(
        "lib-a".into(),
        vec![premiere(
            "series-a",
            "Show A",
            "2026-11-20T00:00:00.0000000Z",
        )],
    );
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowsSetView {
            lib_idx: 0,
            view: GuiShowsView::Upcoming,
        },
    );
    let id = crate::app::tests::gui_shows_projection(&mut app, 0).rows[0]
        .id
        .clone();

    assert_eq!(
        crate::app::gui_library::shows_projection::shows_item(&app, 0, &id),
        None
    );
}

/// PR 1021 regression: a New fetch that lands with no rows — whether the
/// Service returned an empty list or the worker failed — paints the empty
/// state, not `loading`, and needs no re-request to get there.
#[rstest]
#[case::empty(Ok(Vec::new()))]
#[case::failed(Err(mbv_emby::EmbyError::resolve("latest episodes failed")))]
fn an_empty_or_failed_new_fetch_paints_the_empty_state(
    #[case] result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
) {
    let _guard = TestStateDirGuard::new();
    let mut app = empty_shows_app();
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowsSetView {
            lib_idx: 0,
            view: GuiShowsView::New,
        },
    );
    handle_gui_shows_new(
        &mut app,
        GuiShowsNewEvent {
            library_id: "lib-tv".into(),
            result,
        },
    );

    let projection = crate::app::tests::gui_shows_projection(&mut app, 0);

    assert!(!projection.loading, "the landed fetch is not loading");
    assert!(projection.rows.is_empty(), "the list paints no rows");
}

/// An Upcoming row shows its series' image (gui-delineated-rows design D5,
/// D6): the premiere has no Hero, so its row takes the row chain addressed to
/// the premiere's series, under the series' own declared tag — the image a
/// series row shows.
#[test]
fn an_upcoming_row_shows_its_series_image() {
    let _guard = TestStateDirGuard::new();
    let mut series = series_item("series-a", "Show A", "Show A");
    series.image_tags.thumb = "t1".into();
    let key = mbv_images::row::emby_row_request(&series, false).cache_key;
    let mut app = shows_app(vec![series]);
    app.tv_upcoming_snapshots.insert(
        "lib-a".into(),
        vec![premiere(
            "series-a",
            "Show A",
            "2026-12-01T00:00:00.0000000Z",
        )],
    );
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowsSetView {
            lib_idx: 0,
            view: GuiShowsView::Upcoming,
        },
    );
    let still = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        128,
        72,
        image::Rgba([20, 40, 60, 255]),
    ));
    app.seed_card_image_for_test(key.clone(), still);

    let projection = crate::app::tests::gui_shows_projection(&mut app, 0);

    assert_eq!(
        projection.rows[0].art,
        GuiRowArt::Image {
            key: key.clone(),
            square: false
        },
        "the premiere row shows the series image under the series' tag"
    );
    assert_eq!(projection.rows[0].media, GuiRowMediaKind::Video);
}
