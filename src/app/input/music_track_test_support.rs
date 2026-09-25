use super::*;
use crate::app::state::types::browse::BrowseResting;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, PanelFocus, TabSelection};

/// Music library sitting on the album-folder-listing nav
/// level (`is_viewing_album_folders` holds): a grouped `["group",
/// "album"]` config, mirroring `render_library`'s inline-detail
/// tests, with two albums at the album level and `album-1` selected.
pub(super) fn make_music_album_app() -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);
    app.music_levels = vec!["group".into(), "album".into()];

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();

    let mut group = make_item("Alpha", "MusicArtist");
    group.id = "group-0".into();
    group.is_folder = true;

    let mut album1 = make_item("First Album", "MusicAlbum");
    album1.id = "album-1".into();
    album1.is_folder = true;
    let mut album2 = make_item("Second Album", "MusicAlbum");
    album2.id = "album-2".into();
    album2.is_folder = true;

    app.libs.push(LibraryTab {
        nav_stack: vec![
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
            },
            BrowseLevel {
                fetched_rows: 0,
                parent_id: "group-0".into(),
                title: "Alpha".into(),
                items: vec![album1, album2],
                total_count: 2,
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
            },
        ],
        ..LibraryTab::new(library)
    });

    app
}

pub(super) fn push_tracks(app: &mut App, album_id: &str, count: usize) {
    let tracks: Vec<_> = (0..count)
        .map(|i| {
            let mut t = make_item(&format!("Track {i}"), "Audio");
            t.id = format!("{album_id}-track-{i}");
            t
        })
        .collect();
    app.album_tracks_cache.insert(album_id.to_string(), tracks);
}
