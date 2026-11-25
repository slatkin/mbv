//! The rows the plain and feed-group modes project (row 8.1, contracts a and
//! b): the plain mode's rows, letters, and Hero with no Workspace, the
//! feed-group mode's pills and date-ordered rows, no Hero for a folder or a
//! boxset row, and the row art each mode's items paint.

use super::*;

fn movie_item(id: &str, name: &str, year: u32, runtime_ticks: i64) -> EmbyItem {
    let mut item = make_item(name, "Movie");
    item.id = id.to_string();
    item.production_year = year;
    item.runtime_ticks = runtime_ticks;
    item
}

/// A stub `App` whose plain Movies library is active, with the root-list
/// store holding `items` (row 8.1): the state a landed fetch leaves, read by
/// the projection with no Service behind it.
fn plain_movies_app(items: Vec<EmbyItem>) -> App {
    let mut app = make_app_stub();
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = MOVIES_ID.into();
    library.collection_type = "movies".into();
    library.is_folder = true;
    app.push_library_for_test(LibraryTab::new(library));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    stub_gui_state(|gui| {
        gui.emby_roots.insert(
            0,
            crate::gui_core::gui_library::GuiEmbyRootItems::Loaded(items),
        );
    });
    app
}

/// The plain mode's projection (row 8.1, contract a): the rows are the
/// root-list store's rows in the server's `SortName` order, each with its
/// rail letter over the article-stripped sort name (`#` past Z), the first
/// row is the Hero default, the Hero is the shared Emby hero producer's
/// facts with no Workspace, and the mode carries no pill at all.
#[test]
fn the_plain_mode_projects_rows_letters_and_the_hero_without_workspace() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = plain_movies_app(vec![
        {
            let mut item = movie_item(
                "m-1",
                "A Movie",
                1999,
                5400 * mbv_emby_model::TICKS_PER_SECOND,
            );
            item.premiere_date = "1999-06-01T00:00:00Z".into();
            item.people = vec![
                mbv_emby_model::EmbyPerson {
                    name: "Kirsten Johnson".into(),
                    role: String::new(),
                    kind: "Director".into(),
                },
                mbv_emby_model::EmbyPerson {
                    name: "Jane Doe".into(),
                    role: "Lead".into(),
                    kind: "Actor".into(),
                },
            ];
            item.external_urls = vec![mbv_emby_model::EmbyLink {
                name: "IMDb".into(),
                url: "https://www.imdb.com/title/tt0000001".into(),
            }];
            item
        },
        movie_item("m-2", "B Movie", 0, 900 * mbv_emby_model::TICKS_PER_SECOND),
        movie_item("m-3", "9th Street", 0, 0),
    ]);

    let projection = gui_emby_projection(&mut app, 0);

    let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, ["m-1", "m-2", "m-3"], "the rows are the store's list");
    // The rail letter reads the article-stripped sort name, as the TUI's
    // letter buckets do: "A Movie" sorts as "Movie".
    let letters: Vec<char> = projection
        .rows
        .iter()
        .filter_map(|row| row.sort_letter)
        .collect();
    assert_eq!(letters, ['M', 'B', '#'], "the rail letters read the names");
    assert_eq!(
        projection.rows[0].subtitle.as_deref(),
        Some("1999"),
        "a year rides the row's subtitle"
    );
    assert_eq!(
        projection.rows[1].subtitle.as_deref(),
        Some("15:00"),
        "a runtime rides the row's subtitle when no year exists"
    );
    assert!(projection.rows.iter().all(|row| row.date.is_none()));
    assert!(
        projection.collections.is_none() && !projection.feed_group,
        "the plain mode carries no collection selector"
    );
    assert!(!projection.loading && !projection.failed);
    assert_eq!(
        projection.selected.as_deref(),
        Some("m-1"),
        "the first row is the Hero default"
    );
    let hero = projection.hero.expect("the Hero paints");
    assert_eq!(
        hero.kind, "movie",
        "the kicker's kind word is the item type's lowercase word"
    );
    assert_eq!(hero.title, "A Movie");
    // The shared Emby hero producer's facts: the premiere date, the
    // duration row, and the IMDb row the producer appends from the item's
    // external URLs — the same meta rows the TUI's hero paints.
    assert_eq!(
        mbv_ui_model::meta::meta_rows_text(&hero.meta_rows),
        vec!["1 Jun 1999", "01:30:00", "IMDb"]
    );
    assert_eq!(hero.duration_row, Some(1));
    assert_eq!(hero.progress_row, None);
    // The credits and the IMDb target ride the shared producer's output,
    // director first (design D17, Q7): the TUI paints the same table.
    assert_eq!(
        hero.credits,
        vec![
            GuiEmbyHeroCredit {
                name: "Kirsten Johnson".into(),
                role: "Director".into(),
            },
            GuiEmbyHeroCredit {
                name: "Jane Doe".into(),
                role: "Lead".into(),
            },
        ]
    );
    assert_eq!(
        hero.imdb_url.as_deref(),
        Some("https://www.imdb.com/title/tt0000001"),
        "the IMDb meta row's link target rides the hero"
    );
    assert!(hero.overview.is_none());
    // No image tag declares a source, so the Hero carries none and the
    // window paints its own placeholder.
    assert!(hero.art.is_none());
}

/// The feed-group mode's projection (row 8.1, contract b; 2.6, design D2, D3,
/// D4): the pills are All and each root folder, with the checked set's pill
/// checked, the checked folders' items are the rows in the stored date order
/// with the year gutter and no rail letter, and the mode's flag is set.
#[test]
fn the_feed_group_mode_projects_pills_and_the_date_ordered_rows() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = FEED_ID.into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    app.push_library_for_test(LibraryTab::new(library));
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    app.config_for_test(|config| config.feed_view_libraries.push("youtube".into()));
    app.edit_library_for_test(0, |library| {
        library.nav_stack.push(level(
            FEED_ID,
            "YouTube",
            vec![
                folder_item("folder-1", "Channel 1"),
                folder_item("folder-2", "Empty"),
            ],
        ));
    });
    let mut first = make_item("Old Clip", "Video");
    first.id = "vid-1".into();
    first.production_year = 2020;
    first.parent_id = "folder-1".into();
    first.date_added = "2020-01-01T10:00:00.0000000Z".into();
    let mut second = make_item("New Clip", "Video");
    second.id = "vid-2".into();
    second.parent_id = "folder-1".into();
    second.date_added = "2026-01-01T10:00:00.0000000Z".into();
    app.edit_library_for_test(0, |library| {
        library.feed_home_video = Some(FeedHomeVideoState {
            folder_counts: [("folder-1".to_string(), 2_usize)].into_iter().collect(),
            items: [(FeedHomeVideoSelection::All, vec![first, second])]
                .into_iter()
                .collect(),
            ..FeedHomeVideoState::default()
        });
    });

    let projection = gui_emby_projection(&mut app, 0);

    assert!(projection.feed_group, "the feed-group mode is named");
    let collections = projection
        .collections
        .as_ref()
        .expect("the feed-group mode carries its collections");
    let names: Vec<&str> = collections
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Channel 1", "Empty"],
        "the dropdown offers each root folder, with no New row"
    );
    let selected: Vec<&str> = collections
        .entries
        .iter()
        .filter(|entry| entry.checked)
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(selected, Vec::<&str>::new(), "All checks no folder's row");
    assert!(collections.all, "the selection is All");
    let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        ["vid-2", "vid-1"],
        "the rows are the checked folders' date order, newest first"
    );
    assert!(
        projection.rows.iter().all(|row| row.sort_letter.is_none()),
        "the date-ordered list shows no rail"
    );
    assert_eq!(
        projection.rows[1].date.as_deref(),
        Some("2020"),
        "the year rides the feed row's gutter"
    );
    assert_eq!(
        projection.rows[0].subtitle.as_deref(),
        Some("Channel 1"),
        "an All row names the programme (root folder) the video sits in"
    );
    assert_eq!(projection.collection_names, vec!["All"]);
    assert_eq!(
        projection.selected.as_deref(),
        Some("vid-2"),
        "the first row is the Hero default"
    );
    assert!(projection.hero.is_some(), "the Hero paints");
}

/// The TUI's Hero gate (row 8.1, design D15): a row that is a folder paints no
/// Hero, because the Hero describes an item and a folder is a container, and a
/// Movies library admits only a `Movie`, so a `BoxSet` row paints none either.
/// `hero_item` is the TUI's own rule.
#[test]
fn a_folder_or_a_boxset_row_paints_no_hero() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut boxset = make_item("Boxset", "BoxSet");
    boxset.id = "box-1".into();
    let mut app = plain_movies_app(vec![boxset]);
    stub_gui_state(|gui| {
        gui.emby_libs.insert(
            0,
            crate::gui_core::gui_library::GuiEmbyLibrarySelection {
                picked_row: Some("box-1".into()),
                ..Default::default()
            },
        );
    });

    let projection = gui_emby_projection(&mut app, 0);

    assert_eq!(
        projection.selected.as_deref(),
        Some("box-1"),
        "the row is still the selection"
    );
    assert!(
        projection.hero.is_none(),
        "a BoxSet row names a container, so the Hero paints nothing"
    );
}

/// A row image for the cache, as a thumbnail rendition would decode: a
/// landscape still at the Emby row's size.
fn row_image() -> image::DynamicImage {
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        128,
        72,
        image::Rgba([20, 40, 60, 255]),
    ))
}

/// A plain list row paints the art its item's Hero artwork policy declares
/// (gui-delineated-rows design D4, D5): a row whose item declares an image
/// shows the cached row image with its item's media kind, and a row whose item
/// declares none is the no-source state.
#[test]
fn the_plain_mode_rows_paint_their_row_art() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut tagged = movie_item("m-1", "A Movie", 1999, 0);
    tagged.image_tags.thumb = "t1".into();
    let key = mbv_images::row::emby_row_request(&tagged, false).cache_key;
    let untagged = movie_item("m-2", "B Movie", 0, 0);
    let mut app = plain_movies_app(vec![tagged, untagged]);
    app.seed_card_image_for_test(key.clone(), row_image());

    let projection = gui_emby_projection(&mut app, 0);

    assert_eq!(
        projection.rows[0].art,
        GuiRowArt::Image {
            key: key.clone(),
            square: false
        },
        "a tagged row shows its cached row image"
    );
    assert_eq!(projection.rows[0].media, GuiRowMediaKind::Video);
    assert_eq!(
        projection.rows[1].art,
        GuiRowArt::NoSource,
        "an item that declares no image has no source"
    );
}
