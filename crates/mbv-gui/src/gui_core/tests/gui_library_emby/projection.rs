//! The rows the plain and feed-group modes project (row 8.1, contracts a and
//! b): the plain mode's rows, letters, and Hero with no Workspace, the
//! feed-group mode's pills and date-ordered rows, no Hero for a folder or a
//! boxset row, and the row art each mode's items paint.

use super::*;
use mbv_emby_model::EmbyPerson;
use mbv_ui_model::media_list::{ActiveProgress, MediaSemanticState};
use mbv_ui_model::meta::MetaKind;
use rstest::rstest;

fn movie_item(id: &str, name: &str, year: u32, runtime_ticks: i64) -> EmbyItem {
    let mut item = make_item(name, "Movie");
    item.id = id.to_string();
    item.production_year = year;
    item.runtime_ticks = runtime_ticks;
    item
}

/// One person of a movie's people, over the wire fields the parse carries.
fn person(name: &str, role: &str, kind: &str, id: &str, tag: &str) -> EmbyPerson {
    EmbyPerson {
        name: name.into(),
        role: role.into(),
        kind: kind.into(),
        id: id.into(),
        primary_image_tag: tag.into(),
    }
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

/// A stub `App` whose feed-view library is active with the All group holding
/// `items`, as a landed feed fetch leaves it (row 8.1): the state the
/// projection's feed-group mode reads with no Service behind it.
fn feed_state_app(items: Vec<EmbyItem>) -> App {
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
            vec![folder_item("folder-1", "Channel 1")],
        ));
        library.feed_home_video = Some(FeedHomeVideoState {
            items: [(FeedHomeVideoSelection::All, items)].into_iter().collect(),
            ..FeedHomeVideoState::default()
        });
    });
    app
}

/// The stub `App` one state case projects through: the plain list's root-list
/// store in the plain mode, the feed group's All list in the feed-group mode.
fn state_app(feed: bool, item: EmbyItem) -> App {
    if feed {
        feed_state_app(vec![item])
    } else {
        plain_movies_app(vec![item])
    }
}

/// A played movie, as a server's watched flag leaves it.
fn state_played_item() -> EmbyItem {
    let mut item = movie_item(
        "s-1",
        "Watched",
        2001,
        100 * mbv_emby_model::TICKS_PER_SECOND,
    );
    item.played = true;
    item
}

/// A partly played movie with a resumable position a tenth into its runtime.
fn state_partly_played_item() -> EmbyItem {
    let mut item = movie_item("s-2", "Halfway", 0, 6000 * mbv_emby_model::TICKS_PER_SECOND);
    item.playback_position_ticks = 600 * mbv_emby_model::TICKS_PER_SECOND;
    item
}

/// A movie with no played or resume fact.
fn state_unplayed_item() -> EmbyItem {
    movie_item("s-3", "Fresh", 0, 200 * mbv_emby_model::TICKS_PER_SECOND)
}

/// A played music track, whose stored played fact the derivation ignores.
fn state_played_music_item() -> EmbyItem {
    let mut item = make_item("Song", "Audio");
    item.id = "s-4".into();
    item.media_type = "Audio".into();
    item.played = true;
    item.runtime_ticks = 200 * mbv_emby_model::TICKS_PER_SECOND;
    item
}

/// An Emby library row carries the shared play state (gui-emby-row-played-
/// marks row 1.2; spec "An Emby library row carries the shared play state"):
/// the row's `state` is what the one derivation derives — a played item to
/// `Played`, a resumable position to `Active`, an unplayed item to
/// `Ordinary`, and a played music item to `Ordinary` through the music
/// exception — in both the plain mode's and the feed-group mode's rows. The
/// derivation internals are `mbv-ui-model`'s.
#[rstest]
#[case::a_played_item_is_played(
    state_played_item(),
    MediaSemanticState::Played,
    "a played item's row is Played"
)]
#[case::a_resumable_position_is_active(
    state_partly_played_item(),
    MediaSemanticState::Active {
        progress: Some(ActiveProgress::new(10))
    },
    "a partly played item with a resumable position is Active with its percentage"
)]
#[case::an_unplayed_item_is_ordinary(
    state_unplayed_item(),
    MediaSemanticState::Ordinary,
    "an item with no played or resume fact is Ordinary"
)]
#[case::a_played_music_item_is_ordinary(
    state_played_music_item(),
    MediaSemanticState::Ordinary,
    "a played music item is Ordinary through the music exception"
)]
fn an_emby_row_carries_the_shared_play_state(
    #[values(false, true)] feed: bool,
    #[case] item: EmbyItem,
    #[case] expected: MediaSemanticState,
    #[case] why: &str,
) {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let id = item.id.clone();
    let mut app = state_app(feed, item);

    let projection = gui_emby_projection(&mut app, 0);

    let row = projection
        .rows
        .iter()
        .find(|row| row.id == id)
        .expect("the item's row projects");
    assert_eq!(row.state, expected, "{why}");
}

/// The plain mode's projection (row 8.1, contract a): the rows are the
/// root-list store's rows in the server's `SortName` order, each with its
/// rail letter over the article-stripped sort name (`#` past Z), the first
/// row is the Hero default, the Hero is the D1 builder's facts with no
/// Workspace, and the mode carries no pill at all.
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
                    id: String::new(),
                    primary_image_tag: String::new(),
                },
                mbv_emby_model::EmbyPerson {
                    name: "Jane Doe".into(),
                    role: "Lead".into(),
                    kind: "Actor".into(),
                    id: "person-9".into(),
                    primary_image_tag: "tag-9".into(),
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
    assert_eq!(hero.title, "A Movie");
    // The D1 movie builder's facts (spec scenario "A movie's Hero"): the
    // year, runtime, and IMDb-link pills over the Director and Released
    // rows — and no runtime row.
    assert_eq!(
        pill_facts(&hero.facts),
        vec![
            ("1999".into(), Some(MetaKind::Date), None),
            ("01:30:00".into(), Some(MetaKind::Time), None),
            (
                "IMDb".into(),
                Some(MetaKind::Plain),
                Some("https://www.imdb.com/title/tt0000001".into())
            ),
        ],
        "the pills are the year, the runtime, and the IMDb link"
    );
    assert_eq!(
        row_facts(&hero.facts),
        vec![
            (GuiHeroFactLabel::Director, "Kirsten Johnson".into()),
            (GuiHeroFactLabel::Released, "1 Jun 1999".into()),
        ],
        "the rows are the director and the release date"
    );
    // The cast (design D3): the people that are not directors, with Jane
    // Doe's photo fetched — the director paints in the Director row only.
    assert_eq!(
        cast_facts(&hero.cast),
        vec![("Jane Doe", "Lead", "JD", true)]
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

/// One pill of `facts` as `(text, type kind, link target)`, in paint order.
fn pill_facts(facts: &GuiHeroFacts) -> Vec<(String, Option<MetaKind>, Option<String>)> {
    facts
        .pills
        .iter()
        .map(|pill| {
            (
                pill.segments.iter().map(|s| s.text.as_str()).collect(),
                pill.segments.first().map(|s| s.kind),
                pill.link.clone(),
            )
        })
        .collect()
}

/// One row of `facts` as `(label, text)`, in paint order.
/// The cast as `(name, role, initials, has photo)`, in the order the Hero carries it.
fn cast_facts(cast: &[GuiHeroCastMember]) -> Vec<(&str, &str, &str, bool)> {
    cast.iter()
        .map(|member| {
            (
                member.name.as_str(),
                member.role.as_str(),
                member.initials.as_str(),
                member.photo.is_some(),
            )
        })
        .collect()
}

fn row_facts(facts: &GuiHeroFacts) -> Vec<(GuiHeroFactLabel, String)> {
    facts
        .rows
        .iter()
        .map(|row| {
            (
                row.label,
                row.value.iter().map(|s| s.text.as_str()).collect(),
            )
        })
        .collect()
}

/// A movie whose cast is `people`, selected so the Hero projects.
fn cast_app(people: Vec<EmbyPerson>) -> App {
    let mut item = movie_item("m-1", "A Movie", 2015, 0);
    item.people = people;
    plain_movies_app(vec![item])
}

/// One cast outcome the frame carries: the member's name, role, and
/// initials, and whether a photo rides.
type ExpectedMember = (&'static str, &'static str, &'static str, bool);

/// The cast of a movie's Hero (gui-hero-design-polish design D3; spec
/// scenario "A cast member with no photo"): the director paints in the
/// Director row and not in the cast; a person with an id and a tag has a
/// photo; a person with no tag has the initials and no photo.
#[rstest]
#[case::a_director_is_not_in_the_cast(
    vec![person("Kirsten Johnson", "", "Director", "", "")],
    Vec::<ExpectedMember>::new(),
    "the director is the Director row, not a cast member",
)]
#[case::a_person_with_an_id_and_a_tag_has_a_photo(
    vec![person("Jane Doe", "Lead", "Actor", "person-9", "tag-9")],
    vec![("Jane Doe", "Lead", "JD", true)],
    "the id and tag start the Primary photo fetch, so the photo rides",
)]
#[case::a_person_with_no_tag_has_initials_and_no_photo(
    vec![person("Jane Doe", "Lead", "Actor", "person-9", "")],
    vec![("Jane Doe", "Lead", "JD", false)],
    "no tag means no photo, and the initials fall in",
)]
fn the_cast_carries_the_non_directors_and_their_photo_rules(
    #[case] people: Vec<EmbyPerson>,
    #[case] expected: Vec<ExpectedMember>,
    #[case] why: &str,
) {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = cast_app(people);

    let projection = gui_emby_projection(&mut app, 0);

    let hero = projection.hero.expect("the Hero paints");
    assert_eq!(cast_facts(&hero.cast), expected, "{why}");
}
