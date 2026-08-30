#![allow(dead_code, unused_imports)]

use super::super::screens::album_plan::GroupedAlbumDisplayRow;
use super::super::*;
use crate::app::components::{BrowserComponent, MusicWorkspaceComponent, TvWorkspaceComponent};
use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
use crate::app::shell::Model;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::types_audiobookshelf_browse::{
    build_surname_buckets, AudiobookshelfBookBrowseState,
};
use crate::app::types_browse::BrowseResting;
use crate::app::{App, PanelFocus};
use crate::app::{BrowseLevel, LibraryTab, QueueScope, RemoteSlotState, TabSelection};
use crate::config::Config;
use mbv_core::api::EmbyClient;
use mbv_core::api::EmbyItem;
use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfChapter, AudiobookshelfLibrary};
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;

pub fn make_movie_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let mut focused = make_item("Focused Movie", "Movie");
    focused.id = "movie-focused".into();
    focused.overview = "This overview should appear in the compact movie banner while the list remains visible underneath.".into();
    focused.director = "Director Hidden".into();
    focused.production_year = 1988;
    focused.genre = "Action".into();

    let mut second = make_item("Second Movie", "Movie");
    second.id = "movie-second".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![focused, second],
            total_count: 2,
            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    app
}

pub fn make_queue_app(item_count: usize) -> App {
    let mut app = make_movie_app();
    app.panel_focus = PanelFocus::Queue;
    app.player_tab.set_items(
        (0..item_count)
            .map(|i| make_item(&format!("Queue Item {i}"), "Movie"))
            .collect(),
        0,
    );
    app
}

pub fn make_remote_queue_app() -> App {
    let local_items = vec![make_item("Local Queue Item", "Movie")];
    let remote_items = vec![make_item("Remote Queue Item", "Movie")];
    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let mut app = App::new_remote(
        EmbyClient::new(Config::default()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Queue;
    app.queue_scope = QueueScope::Remote;
    app.player_tab.set_items(local_items, 0);
    app
}

pub fn make_music_group_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.music_levels = vec!["group".into(), "album".into()];

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();

    let group_names = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"];
    let groups: Vec<EmbyItem> = group_names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut it = make_item(n, "MusicArtist");
            it.id = format!("group-{i}");
            it.is_folder = true;
            it
        })
        .collect();

    let mut album = make_item("First Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = "Alpha".into();
    album.production_year = 2001;

    app.libs.push(LibraryTab {
        nav_stack: vec![
            BrowseLevel {
                parent_id: "lib-music".into(),
                title: "Music".into(),
                items: groups,
                total_count: group_names.len(),
                resting: BrowseResting::new(0, 0),
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
                music_grouping: None,
            },
            BrowseLevel {
                parent_id: "group-0".into(),
                title: "Alpha".into(),
                items: vec![album],
                total_count: 1,
                resting: BrowseResting::new(0, 0),
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
                music_grouping: None,
            },
        ],
        ..LibraryTab::new(library)
    });

    app
}

/// Builds on `make_music_group_app` by adding a second sibling album
/// ("Second Album", also by "Alpha") to the same nav level. Shared by the
/// cache-miss/loading and cache-hit/rendered inline-detail tests, which
/// both need a following album to assert framing around the selected one.
pub fn make_music_group_app_with_second_album() -> App {
    let mut app = make_music_group_app();
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second_album);
    app
}

/// Shared row-map assertions for the inline album-detail tests: the
/// selected album's inline detail (loading indicator or rendered tracks)
/// must sit between the selected album title and the following sibling
/// album, and every row in between must be non-selectable.
pub fn assert_inline_detail_frames_between_albums(
    lines: &[&str],
    layout: &LayoutMain,
    title_y: usize,
    detail_y: usize,
) {
    assert!(
        lines[title_y - 4].trim().is_empty(),
        "expected the colored top-padding row above the artist header to be blank:\n{}",
        lines.join("\n")
    );
    assert_eq!(
        lines.iter().filter(|line| line.trim() == "Alpha").count(),
        1,
        "plain album framing must not duplicate the artist name:\n{}",
        lines.join("\n")
    );
    assert!(
        detail_y > title_y,
        "expected the inline detail row to render below the selected album title:\n{}",
        lines.join("\n")
    );

    let second_album_y = lines
        .iter()
        .position(|l| l.contains("Second Album"))
        .expect("expected the following album row");
    assert!(
        second_album_y > detail_y,
        "expected the inline detail to render before sibling albums:\n{}",
        lines.join("\n")
    );

    let title_row_idx = layout
        .left_row_map
        .iter()
        .position(|r| *r == Some(0))
        .expect("expected the selected album (index 0) in the row map");
    let second_row_idx = layout
        .left_row_map
        .iter()
        .position(|r| *r == Some(1))
        .expect("expected the following album (index 1) in the row map");
    assert!(
        second_row_idx > title_row_idx,
        "expected the following album's row-map entry after the selected album's"
    );
    assert!(
        layout.left_row_map[title_row_idx + 1..second_row_idx]
            .iter()
            .all(Option::is_none),
        "expected every row between the two albums (borders, padding, detail) to be non-selectable:\n{:?}",
        layout.left_row_map
    );
}

pub fn make_home_video_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Home Videos", "CollectionFolder");
    library.id = "lib-homevideos".into();
    library.is_folder = true;
    library.collection_type = "homevideos".into();

    let mut first = make_item("Birthday Clip", "Video");
    first.id = "video-1".into();
    let mut second = make_item("Vacation Clip", "Video");
    second.id = "video-2".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-homevideos".into(),
            title: "Home Videos".into(),
            items: vec![first, second],
            total_count: 2,
            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    app
}

pub fn make_large_movie_library_app(library_total: usize) -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: Vec::new(),
            total_count: 0,
            resting: BrowseResting::new(0, 0),
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        library_total: Some(library_total),
        ..LibraryTab::new(library)
    });

    app
}

/// Book surface app for conformance tests (moved here from the deleted
/// `tests_audiobookshelf_books.rs` legacy-renderer suite, task 5.3d.13). Three
/// books span three surname buckets (Adams -> A-C, Mason -> J-L, Zephyr ->
/// V-Z), so the A-C bucket is selected by default and only "Alpha Tales" is in
/// range.
pub fn make_audiobookshelf_book_app() -> App {
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
            AudiobookshelfBook {
                library_item_id: "book-a".into(),
                title: "Alpha Tales".into(),
                author_display: Some("Adams".into()),
                author_sort_key: "Adams".into(),
                cover_path: None,
                duration_seconds: 0.0,
                narrator: None,
                published_year: None,
                genres: Vec::new(),
                description: None,
                series_name: None,
                chapters: Vec::new(),
                audio_files: Vec::new(),
            },
            AudiobookshelfBook {
                library_item_id: "book-m".into(),
                title: "Middle Ground".into(),
                author_display: Some("Mason".into()),
                author_sort_key: "Mason".into(),
                cover_path: None,
                duration_seconds: 0.0,
                narrator: None,
                published_year: None,
                genres: Vec::new(),
                description: None,
                series_name: None,
                chapters: Vec::new(),
                audio_files: Vec::new(),
            },
            AudiobookshelfBook {
                library_item_id: "book-z".into(),
                title: "Zenith Story".into(),
                author_display: Some("Zephyr".into()),
                author_sort_key: "Zephyr".into(),
                cover_path: None,
                duration_seconds: 0.0,
                narrator: None,
                published_year: None,
                genres: Vec::new(),
                description: None,
                series_name: None,
                chapters: Vec::new(),
                audio_files: Vec::new(),
            },
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
