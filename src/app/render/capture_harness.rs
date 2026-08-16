// THROWAWAY — capture harness for openspec change `centralize-ui-design-language`
// (tasks 0.2/0.3). Deleted, along with its registration in `render/mod.rs`, by
// task 10.1 at close-out. Do not extend or rely on this beyond that change.
//
// Renders each of the eight hero-plus-list screens at one width below
// `TWO_COLUMN_THRESHOLD` and one above it, and writes the text buffer to
// `target/ui-captures/<name>-<width>.txt` for before/after byte diffing.
// Run with:
//   rtk cargo nextest run -p mbv capture_harness::capture_all_screens -- --ignored
#![allow(dead_code)]

use super::test_helpers::*;
use super::*;
use crate::app::layout::LayoutMain;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
use crate::app::types_feed_tab::FeedTabState;
use crate::app::types_playback::HomeLatestSource;
use crate::app::{BrowseLevel, LibraryTab, PanelFocus, TabSelection, TWO_COLUMN_THRESHOLD};
use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfChapter, AudiobookshelfLibrary};
use mbv_core::playback_queue::QueueItem;

const NARROW_WIDTH: u16 = TWO_COLUMN_THRESHOLD - 4;
// The breakpoint is tested against the right-panel *content* area, not the
// raw terminal width: the default queue column (`LEFT_WIDTH_DEFAULT` = 40)
// plus its gap and tab padding eat ~44 columns before that check runs. A
// terminal width of `TWO_COLUMN_THRESHOLD + 28` was found (while starting
// phase 6) to leave the content area at ~66 columns -- always below the
// 82-column threshold, so every prior "wide" capture in this change was
// silently exercising the narrow arrangement twice. This offset pushes the
// content area comfortably past the threshold.
const WIDE_WIDTH: u16 = TWO_COLUMN_THRESHOLD + 64;
const HEIGHT: u16 = 40;

fn capture(app: &mut App, width: u16, height: u16) -> String {
    let term = render_view_to_terminal(app, width, height).0;
    buffer_to_string(&term)
}

fn write_capture(name: &str, width: u16, text: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-captures");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{name}-{width}.txt")), text).unwrap();
}

fn make_tv_shows_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "lib-shows".into();
    library.is_folder = true;
    library.collection_type = "tvshows".into();

    let mut focused = make_item("Focused Show", "Series");
    focused.id = "series-focused".into();
    focused.overview = "This overview should appear in the compact show banner while the list remains visible underneath.".into();
    focused.production_year = 2014;
    focused.genre = "Drama".into();

    let mut second = make_item("Second Show", "Series");
    second.id = "series-second".into();

    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-shows".into(),
            title: "Shows".into(),
            items: vec![focused, second],
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: Some("Series".into()),
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

    app
}

fn book(id: &str, title: &str, author_surname: &str) -> AudiobookshelfBook {
    AudiobookshelfBook {
        library_item_id: id.into(),
        title: title.into(),
        author_display: Some(author_surname.into()),
        author_sort_key: author_surname.into(),
        cover_path: None,
        chapters: Vec::new(),
        audio_files: Vec::new(),
    }
}

fn make_audiobookshelf_books_app() -> App {
    let mut app = make_app_stub();
    let library = AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library.clone());
    state.append_page_books(
        0,
        3,
        vec![
            book("book-a", "Alpha Tales", "Adams"),
            book("book-m", "Middle Ground", "Mason"),
            book("book-z", "Zenith Story", "Zephyr"),
        ],
    );
    state.detail_cache.insert(
        "book-a".into(),
        (
            vec![AudiobookshelfChapter {
                id: 0,
                start: 0.0,
                end: 60.0,
                title: "Chapter One".into(),
            }],
            Vec::new(),
        ),
    );
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_book_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}

fn make_audiobookshelf_podcasts_app() -> App {
    crate::app::tests_podcast::audiobookshelf_app()
}

fn make_feeds_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::Feeds;
    app.panel_focus = PanelFocus::Library;

    let mut feed_tab = FeedTabState::default();
    feed_tab.subscriptions = vec![mbv_core::config::FeedSubscription {
        name: "Example Feed".into(),
        url: "https://example.test/feed.xml".into(),
        kind: mbv_core::config::FeedKind::Video,
    }];
    feed_tab.entries = vec![vec![
        mbv_core::playback_queue::FeedEntry {
            guid: "guid-1".into(),
            title: "First Feed Item".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played: false,
        },
        mbv_core::playback_queue::FeedEntry {
            guid: "guid-2".into(),
            title: "Second Feed Item".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played: false,
        },
    ]];
    feed_tab.rebuild_all_entries();
    app.feed_tab = feed_tab;
    app
}

fn make_home_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::Home;

    let mut continuing = make_item("Continuing Movie", "Movie");
    continuing.id = "movie-continuing".into();
    continuing.overview = "An overview for the Home continue-watching hero.".into();
    continuing.production_year = 2020;
    app.home.continue_items = vec![continuing];

    let mut latest_item = make_item("Latest Movie", "Movie");
    latest_item.id = "movie-latest".into();
    app.home.latest = vec![(
        "Movies".into(),
        HomeLatestSource::Emby("lib-movies".into()),
        vec![QueueItem::Emby(Box::new(latest_item))],
        0,
    )];

    app
}

#[test]
#[ignore = "THROWAWAY: run manually for before/after capture diffing"]
fn capture_all_screens() {
    let screens: Vec<(&str, fn() -> App)> = vec![
        ("home", make_home_app as fn() -> App),
        ("movies", make_movie_app as fn() -> App),
        ("tv_shows", make_tv_shows_app as fn() -> App),
        ("music", make_music_group_app as fn() -> App),
        ("audiobooks", make_audiobookshelf_books_app as fn() -> App),
        ("podcasts", make_audiobookshelf_podcasts_app as fn() -> App),
        ("feeds", make_feeds_app as fn() -> App),
        ("home_videos", make_home_video_app as fn() -> App),
    ];

    for (name, make) in &screens {
        for width in [NARROW_WIDTH, WIDE_WIDTH] {
            let mut app = make();
            let text = capture(&mut app, width, HEIGHT);
            write_capture(name, width, &text);
        }
    }

    // Diagnostic: compare audiobooks against music at the same width so the
    // gap recorded for task 6.4 is concrete, not asserted.
    for width in [NARROW_WIDTH, WIDE_WIDTH] {
        let mut books_app = make_audiobookshelf_books_app();
        let books = capture(&mut books_app, width, HEIGHT);
        let mut music_app = make_music_group_app();
        let music = capture(&mut music_app, width, HEIGHT);
        write_capture("_compare_books", width, &books);
        write_capture("_compare_music", width, &music);
        println!(
            "--- books vs music @ width={width} (equal={}) ---",
            books == music
        );
    }

    let _ = LayoutMain::default();
}
