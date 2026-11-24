//! The browse-root reset's own contract: the reset restores a feed-view
//! library's root to a plain folder listing.

use crate::app::tests::*;
use crate::app::{
    App, BrowseEvent, BrowseLevel, FeedHomeVideoState, LibEvent, LibraryTab, TabSelection,
};
use mbv_emby_model::EmbyItem;
use mbv_emby_model::test_support::make_item;
use mbv_ui_model::browse::{BrowseResting, ServerRows};

/// The unfiltered root level of the `YouTube` home-video library.
fn youtube_root_level(rows: ServerRows, items: Vec<EmbyItem>, loading: bool) -> BrowseLevel {
    BrowseLevel {
        rows,
        parent_id: "lib-youtube".into(),
        title: "YouTube".into(),
        items,
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

fn make_home_video_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
        nav_stack: vec![youtube_root_level(ServerRows::complete(0), vec![], true)],
        feed_home_video: Some(FeedHomeVideoState {
            loading: true,
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });

    app
}

fn seed_home_video_root_loaded(app: &mut App) {
    let mut empty = make_item("Empty Channel", "Folder");
    empty.id = "folder-empty".into();
    empty.is_folder = true;
    empty.path = "/videos/empty".into();

    let mut active = make_item("Active Channel", "Folder");
    active.id = "folder-active".into();
    active.is_folder = true;
    active.path = "/videos/active".into();

    app.handle_lib_event(
        LibEvent::Browse(BrowseEvent::Loaded {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            level: Box::new(youtube_root_level(
                ServerRows::loaded(2, 2),
                vec![empty, active],
                false,
            )),
        }),
        None,
    );
}

/// Contract (review P2 on task 5.1, commit 46b0ac739): the UI-state reset
/// restores a feed-view library's root to a plain folder listing — no
/// `item_types`, no unplayed-only flag — so a later refresh builds its fetch
/// from folder-listing fields instead of fetching videos onto the root and
/// destroying the pill source. Moved from
/// `app::tests::feeds` with the reset itself.
#[test]
fn ui_state_reset_restores_the_feed_view_root_to_a_folder_listing() {
    let mut app = make_home_video_app();
    seed_home_video_root_loaded(&mut app);
    if let Some(root) = app.libs[0].nav_stack.first_mut() {
        root.item_types = Some("Video".into());
        root.unplayed_only = true;
        root.sort_by = "DateCreated".into();
        root.sort_order = "Ascending".into();
    }

    app.reset_browse_root_defaults();

    let root = &app.libs[0].nav_stack[0];
    assert_eq!(root.item_types, None, "the reset drops the Video filter");
    assert!(
        !root.unplayed_only,
        "the reset drops the unplayed-only flag"
    );
    assert_eq!(root.sort_by, "SortName");
    assert_eq!(root.sort_order, "Ascending");
    assert_eq!(root.items.len(), 2, "the root keeps its folder items");
}
