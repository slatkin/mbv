//! Tests of the GUI Library's Shows projection (add-gui-library-panel
//! design D14, which supersedes D12's first Shows design): the All series
//! mode's tree — the series nodes with their rail letters, their seasons, and
//! each season's episodes or `None` while unloaded — the series Hero from the
//! TUI's series producer with its season pills and Workspace, the flat Latest
//! mode's rows and episode Hero, the Upcoming mode's premiere rows with no
//! Hero, and the stale selection's first-row fallback. The projection reads
//! the seeded `App` data alone, so no Service, worker, or mock transport runs.

use super::*;
use crate::config::TestStateDirGuard;
use mbv_queue::TvContentMode;

/// A series row of the All series root level, with its sort name.
fn series_item(id: &str, name: &str, sort_name: &str) -> EmbyItem {
    let mut item = make_item(name, "Series");
    item.id = id.to_string();
    item.sort_name = sort_name.to_string();
    item
}

/// An episode row of the Latest root level, with its show and date.
fn episode_item(id: &str, series_name: &str, name: &str, date_added: &str) -> EmbyItem {
    let mut item = make_item(name, "Episode");
    item.id = id.to_string();
    item.series_name = series_name.to_string();
    item.date_added = date_added.to_string();
    item
}

/// A browse level over `items`, in `mode`.
fn level(mode: &TvContentMode, items: Vec<EmbyItem>) -> mbv_ui_model::browse::BrowseLevel {
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
        tv_content_mode: Some(mode.clone()),
        music_grouping: None,
    }
}

/// A stub `App` whose Shows library is the active destination, with the
/// root level `mode`'s list and the selection storing that mode.
fn shows_app(mode: TvContentMode, items: Vec<EmbyItem>) -> App {
    let mut app = make_app_stub();
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    let mut tab = crate::app::LibraryTab::new(library);
    tab.nav_stack.push(level(&mode, items));
    app.libs.push(tab);
    app.tab = TabSelection::EmbyLibrary(0);
    app.gui_tv_shows.insert(
        0,
        crate::app::gui_library::GuiTvShowsSelection {
            mode: Some(mode),
            ..Default::default()
        },
    );
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
    let mut app = shows_app(TvContentMode::All, vec![first, second]);
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

    let projection = app.gui_shows_projection(0);

    assert!(
        projection.rows.is_empty(),
        "the All series mode's list is the tree, not the flat rows"
    );
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

/// The series Hero comes from the TUI's series producer (design D14): the
/// genre, the show's year range, the season count, and the network, with no
/// duration row while the Workspace has no focus — and the Workspace holds the
/// season pills on the first season with the episodes' played/progress states.
#[test]
fn the_series_hero_carries_the_tui_producers_facts_and_the_workspace() {
    let _guard = TestStateDirGuard::new();
    let mut app = series_tree_app();

    let projection = app.gui_shows_projection(0);

    let hero = projection.hero.expect("the series Hero paints");
    assert_eq!(hero.title, "Alfred");
    assert_eq!(
        hero.meta_rows,
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
    let states: Vec<crate::app::gui_library::GuiHomeRowState> = workspace
        .episodes
        .iter()
        .map(|row| row.state.clone())
        .collect();
    assert_eq!(
        states,
        vec![
            crate::app::gui_library::GuiHomeRowState::Played,
            crate::app::gui_library::GuiHomeRowState::Active { progress: Some(25) },
        ],
        "the Workspace rows carry the played/progress states"
    );
}

/// The flat Latest mode's rows and episode Hero (row 7.1, contract b): the
/// rows are the root level's newest episodes in the server's `DateCreated`
/// descending order, each with the split show/episode labels and the
/// provider-date gutter and no rail letter, and the Hero is the selected
/// episode's facts with no Workspace.
#[test]
fn the_latest_mode_projects_flat_rows_and_the_episode_hero() {
    let _guard = TestStateDirGuard::new();
    let mut episode = episode_item("ep-new", "Alfred", "The New Case", "2026-11-08T10:00:00Z");
    episode.image_tags.series_thumb = "tag".into();
    let mut app = shows_app(TvContentMode::Latest, vec![episode.clone()]);

    let projection = app.gui_shows_projection(0);

    assert_eq!(projection.rows.len(), 1);
    let row = &projection.rows[0];
    assert_eq!(row.id, "ep-new");
    assert_eq!(row.title, "Alfred", "the show's name leads the row");
    assert_eq!(row.subtitle.as_deref(), Some("The New Case"));
    assert!(row.date.is_some(), "the provider date rides the gutter");
    assert_eq!(row.sort_letter, None, "the date-sorted list shows no rail");
    assert_eq!(
        projection.selected.as_deref(),
        Some("ep-new"),
        "the first episode row is the Hero default"
    );
    let hero = projection.hero.expect("the episode Hero paints");
    assert_eq!(hero.title, "The New Case");
    assert_eq!(
        hero.parent.as_deref(),
        Some("Alfred"),
        "the series is the largest unit, so it rides as the Hero's parent"
    );
    assert_eq!(
        hero.art_shape,
        crate::app::gui_library::GuiArtShape::Landscape,
        "a thumb-declared episode takes the landscape arm"
    );
    assert!(hero.workspace.is_none(), "the Latest Hero has no Workspace");
}

/// The Upcoming mode's rows and missing Hero (row 7.1, contract c): the rows
/// are the snapshot's premieres with the series' name, the season/episode
/// label, and the premiere date, and the Upcoming mode projects no Hero at
/// all.
#[test]
fn the_upcoming_mode_projects_premiere_rows_and_no_hero() {
    let _guard = TestStateDirGuard::new();
    let mut app = shows_app(TvContentMode::Upcoming, Vec::new());
    let premieres = vec![
        mbv_emby_model::UpcomingPremiere {
            series_id: mbv_ids::ItemId::new("series-a".to_string()),
            series_name: "Alfred".into(),
            season: Some(2),
            episode: Some(7),
            title: "The Reveal".into(),
            premiere_date: "2026-12-01T00:00:00.000Z".into(),
        },
        mbv_emby_model::UpcomingPremiere {
            series_id: mbv_ids::ItemId::new("series-b".to_string()),
            series_name: "Bridgerton".into(),
            season: None,
            episode: None,
            title: "Special".into(),
            premiere_date: String::new(),
        },
    ];
    app.tv_upcoming_snapshots
        .insert("lib-tv".to_string(), premieres.clone());

    let projection = app.gui_shows_projection(0);

    assert_eq!(projection.rows.len(), 2);
    assert_eq!(
        projection.rows[0].id,
        premieres[0].stable_key(),
        "a premiere row's identity is its stable key"
    );
    assert_eq!(projection.rows[0].title, "Alfred");
    assert_eq!(
        projection.rows[0].subtitle.as_deref(),
        Some("S02:E07 — The Reveal")
    );
    assert!(projection.rows[0].date.is_some());
    assert_eq!(
        projection.rows[1].subtitle.as_deref(),
        Some("no number — Special")
    );
    assert!(
        projection.hero.is_none(),
        "the Upcoming mode paints no Hero"
    );
}

/// A stale selection falls back to the first row (design D2, D12): a stored
/// id the active mode's list no longer carries selects the list's first row,
/// so the Hero never names a row the frame did not paint.
#[test]
fn a_stale_selection_falls_back_to_the_first_row() {
    let _guard = TestStateDirGuard::new();
    let mut app = shows_app(
        TvContentMode::All,
        vec![
            series_item("series-a", "Alfred", "Alfred"),
            series_item("series-b", "Bridgerton", "Bridgerton"),
        ],
    );
    app.gui_tv_shows.get_mut(&0).unwrap().series_id = Some("series-gone".into());

    let projection = app.gui_shows_projection(0);

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
/// `HeroArtwork::painted_shape` re-arms on). A thumb-declared episode declares
/// the landscape arm, and when its Primary image is a portrait poster the box
/// must take the image's arm: otherwise the Hero paints a poster inside a 16:9
/// band, which is the GUI/TUI divergence this guards.
#[test]
fn the_episode_heroes_art_box_follows_the_fetched_images_own_aspect() {
    let _guard = TestStateDirGuard::new();
    let mut episode = episode_item("ep-new", "Alfred", "The New Case", "2026-11-08T10:00:00Z");
    episode.image_tags.thumb = "tag".into();
    let mut app = shows_app(TvContentMode::Latest, vec![episode]);
    // The Primary image, seeded with a portrait poster: the policy's arm is
    // Landscape, the fetched image's own aspect is Portrait.
    let poster = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        300,
        450,
        image::Rgba([20, 40, 60, 255]),
    ));
    app.images.insert_image(
        "ep-new:Primary".to_string(),
        mbv_images::CachedImage {
            source: Some(crate::app::queue_card_artwork::test_source(&poster)),
            img: Some(poster),
            protocols: std::collections::HashMap::new(),
            cover_box: None,
            applied_logo_key: None,
        },
    );

    let projection = app.gui_shows_projection(0);

    let hero = projection.hero.expect("the episode Hero paints");
    assert!(hero.art.is_some(), "the seeded image is the Hero's art");
    assert_eq!(
        hero.art_shape,
        crate::app::gui_library::GuiArtShape::Portrait,
        "the box follows the fetched poster's 3:4 pixels, not the declared landscape arm"
    );
}
