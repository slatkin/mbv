use super::{BrowseLevel, LibraryTab, TabSelection};
use crate::app::state::app_struct::LevelFillState;
use crate::app::state::types::browse::BrowseResting;
use crate::app::state::types::events::LibEvent;
use crate::app::tests::{make_app_stub, make_item};
use mbv_core::api::EmbyItem;
use std::time::{Duration, Instant};

fn make_music_album_level(albums: Vec<EmbyItem>) -> BrowseLevel {
    BrowseLevel {
        fetched_rows: 0,
        parent_id: "group-0".into(),
        title: "Alpha".into(),
        items: albums,
        total_count: 0,
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

fn make_group_level() -> BrowseLevel {
    let mut group = make_item("Alpha", "MusicArtist");
    group.id = "group-0".into();
    group.is_folder = true;
    BrowseLevel {
        fetched_rows: 0,
        parent_id: "lib-music".into(),
        title: "Music".into(),
        items: vec![group],
        total_count: 1,
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}
fn make_music_library_tab() -> LibraryTab {
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();
    LibraryTab::new(library)
}

fn make_untagged_album(id: &str) -> EmbyItem {
    let mut album = make_item("Unknown Album", "MusicAlbum");
    album.id = id.into();
    album
}

fn make_music_app(albums: Vec<EmbyItem>) -> super::App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.music_levels = vec!["group".into(), "album".into()];

    app.libs.push(LibraryTab {
        nav_stack: vec![make_group_level(), make_music_album_level(albums)],
        ..make_music_library_tab()
    });
    app
}

mod candidate_state;
