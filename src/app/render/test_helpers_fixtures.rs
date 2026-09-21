#![allow(dead_code, unused_imports)]

use super::super::*;
use crate::app::layout::AppLayout;
use crate::app::shell::Model;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::types_browse::BrowseResting;
use crate::app::{App, PanelFocus};
use crate::app::{BrowseLevel, LibraryTab, QueueScope, RemoteSlotState, TabSelection};
use crate::config::Config;
use mbv_core::api::EmbyClient;
use mbv_core::api::EmbyItem;
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
    focused.people = vec![mbv_core::api::EmbyPerson {
        name: "Director Hidden".into(),
        role: String::new(),
        kind: "Director".into(),
    }];
    focused.production_year = 1988;
    focused.genres = vec!["Action".into()];

    let mut second = make_item("Second Movie", "Movie");
    second.id = "movie-second".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
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
                fetched_rows: 0,
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
                fetched_rows: 0,
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

/// The long album title the Grouped Music tree row tests share: it cannot fit
/// any supported Library browser width, so it exercises truncation, clipping,
/// and the marquee window.
pub const MUSIC_TREE_LONG_TITLE: &str =
    "A Suspiciously Long Album Title That Cannot Fit Any Library Browser Row";
pub const MUSIC_TREE_LONG_TITLE_YEAR: u32 = 1999;

/// Node ids of the tree projection over `make_music_tree_group_app`'s settled
/// entry order: the Alpha root, its 41 leaves, the Beta root, its 2 leaves.
pub const MUSIC_TREE_ALPHA_ROOT: usize = 0;
pub const MUSIC_TREE_BETA_ROOT: usize = 42;
pub const MUSIC_TREE_BETA_LEAF_0: usize = 43;
pub const MUSIC_TREE_BETA_LEAF_1: usize = 44;

/// The expanded projection length for `make_music_tree_group_app`: one Alpha
/// root over 41 leaves, one Beta root over 2 leaves.
pub const MUSIC_TREE_EXPANDED_PROJECTION_LEN: usize = 45;

/// The repository's existing Wide Grouped Music fixture corpus
/// (`music_app_many_albums`'s 40 Alpha albums) plus a long-titled album and a
/// second artist group, so the group-relative zebra reset has two groups to
/// cross.
pub fn make_music_tree_group_app() -> App {
    let mut app = make_music_group_app();
    app.panel_focus = PanelFocus::Library;
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    for i in 1..40 {
        let mut album = make_item(&format!("Album {i:02}"), "MusicAlbum");
        album.id = format!("album-extra-{i}");
        album.artist = "Alpha".into();
        level.items.push(album);
    }
    let mut long_album = make_item(MUSIC_TREE_LONG_TITLE, "MusicAlbum");
    long_album.id = "album-long".into();
    long_album.artist = "Alpha".into();
    long_album.production_year = MUSIC_TREE_LONG_TITLE_YEAR;
    level.items.push(long_album);

    let mut beta_one = make_item("Beta Session", "MusicAlbum");
    beta_one.id = "album-beta-1".into();
    beta_one.artist = "Beta".into();
    level.items.push(beta_one);
    let mut beta_two = make_item("Beta Nights", "MusicAlbum");
    beta_two.id = "album-beta-2".into();
    beta_two.artist = "Beta".into();
    level.items.push(beta_two);

    level.total_count = level.items.len();
    app
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
            fetched_rows: 0,
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
            fetched_rows: 0,
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
