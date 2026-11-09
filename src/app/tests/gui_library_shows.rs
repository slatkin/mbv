//! Tests of the GUI Library's Shows projection (add-gui-library-panel
//! design D12, row 7.1, the USER-APPROVED GUI Shows design): the All series
//! mode's series rows with their rail letters and the series Hero's season
//! pills and Workspace, the flat Latest mode's rows and episode Hero, the
//! Upcoming mode's premiere rows with no Hero, and the stale selection's
//! first-row fallback. The projection reads the seeded `App` data alone, so
//! no Service, worker, or mock transport runs.

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

/// The All series mode's rows, letters, default selection, and series Hero
/// (row 7.1, contract a): the rows are the root level's series in the
/// server's `SortName` order, each with its rail letter over the sort name,
/// the first row is the Hero default, and the Hero is the landscape art with
/// the season pills on the first season and the Workspace's episode rows
/// with their played/progress states.
#[test]
fn the_series_mode_projects_rows_letters_and_the_series_hero() {
    let _guard = TestStateDirGuard::new();
    let mut first = series_item("series-a", "Alfred", "Alfred");
    first.image_tags.thumb = "tag".into();
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
    let detail = SeriesDetail {
        seasons: vec![season],
        episodes: [("season-1".to_string(), vec![played, resumed])]
            .into_iter()
            .collect(),
    };
    app.series_detail_cache
        .insert("series-a".to_string(), detail);

    let projection = app.gui_shows_projection(0);

    let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        ["series-a", "series-b"],
        "the rows are the root series"
    );
    let letters: Vec<char> = projection
        .rows
        .iter()
        .filter_map(|row| row.sort_letter)
        .collect();
    assert_eq!(letters, ['A', 'B'], "the rail letters read the sort names");
    assert_eq!(
        projection.selected.as_deref(),
        Some("series-a"),
        "the first series row is the Hero default"
    );
    let hero = projection.hero.expect("the series Hero paints");
    assert_eq!(hero.title, "Alfred");
    assert_eq!(
        hero.art_shape,
        crate::app::gui_library::GuiArtShape::Landscape,
        "the series art is the policy's landscape arm"
    );
    assert!(hero.art.is_some(), "the declared thumb fetches its art");
    let workspace = hero
        .workspace
        .expect("the series Hero carries its Workspace");
    assert_eq!(
        workspace.selected_season.as_deref(),
        Some("season-1"),
        "the season pills start on the first season"
    );
    assert_eq!(workspace.seasons.len(), 1);
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
