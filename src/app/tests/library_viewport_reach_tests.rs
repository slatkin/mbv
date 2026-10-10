//! Shell coverage for the mouse-input scenario "Scrolling toward the loaded
//! edge fetches the next page" (wheel-scrolls-viewport task 3.1): after a
//! wheel scroll a paged library surface reports the last painted selectable
//! row's index through `ShellRequest::LibraryViewportReach`, and the shell
//! arms the next-page fetch with no navigation-idle gate (a gated wheel could
//! stop scrolling at the loaded edge with no later event to retry).

use super::*;
use mbv_ui_model::browse::{BrowseResting, ServerRows};
use mbv_ui_msg::{Msg, ShellRequest};

fn paged_movie_level(items_loaded: usize, total_count: usize, cursor: usize) -> BrowseLevel {
    BrowseLevel {
        rows: ServerRows::loaded(items_loaded, total_count),
        parent_id: "lib-movies".into(),
        title: "Movies".into(),
        items: make_items(items_loaded),
        resting: BrowseResting::new(cursor, 0),
        item_types: Some("Movie".into()),
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

#[test]
fn viewport_reach_within_prefetch_ahead_arms_the_next_page_fetch() {
    // PREFETCH_AHEAD is 25; 30 of 100 items loaded (not fully loaded).
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    app.libs.push(LibraryTab {
        nav_stack: vec![paged_movie_level(30, 100, 0)],
        ..LibraryTab::new(library)
    });
    let mut model = Model::new(app);

    // Reach index 5: 5 + 25 = 30, not < 30 -> within PREFETCH_AHEAD of the
    // loaded edge, so the next page is requested; the selection stays on row
    // 0 because a reach report only arms paging.
    model.handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::LibraryViewportReach { index: 5 })),
        &mut false,
        &mut false,
    );

    assert!(
        model.app.libs[0].nav_stack.last().unwrap().loading,
        "a reach within PREFETCH_AHEAD of the loaded edge must arm the next page"
    );
    assert_eq!(
        model.app.panel_focus,
        PanelFocus::Library,
        "a claimed wheel must focus Library"
    );
    assert_eq!(
        model.app.libs[0]
            .nav_stack
            .last()
            .unwrap()
            .resting()
            .cursor(),
        0,
        "a reach report must never move the selection"
    );
}
