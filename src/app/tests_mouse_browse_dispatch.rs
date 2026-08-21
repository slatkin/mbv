//! Guard tests for the Section 5 browse mouse + completed-frame destination
//! tag (design §4). The `LayoutMain::browse_destination` tag is set only on
//! the completed, installed layout; browse mouse handling no-ops unless the
//! tag matches the normalized selected destination. These tests pin that a
//! mouse gesture after a destination switch cannot consume another Service's
//! layout state, and that two Emby libraries never share an index space.

use super::*;
use crate::app::tests::{make_app_stub, make_item, make_items};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

fn make_library_app(cursor: usize) -> App {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.collection_type = "movies".into();
    library.is_folder = true;

    let items = make_items(2);
    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items,
            total_count: 2,
            cursor,
            scroll: 0,
            item_types: Some("Movie".into()),
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
    app.layout.main.left_area = Rect {
        x: 10,
        y: 5,
        width: 20,
        height: 5,
    };
    app
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

/// 5.2: after the user switches tabs so `self.tab` differs from the installed
/// `browse_destination`, a browse mouse event is a no-op: no Service state
/// mutates (this is the "previous destination's layout fields describe the
/// wrong destination until redraw" race guard from design §4).
#[test]
fn browse_mouse_noops_when_installed_layout_describes_another_destination() {
    let mut app = make_library_app(0);
    // The installed completed-frame layout was rendered for Home, but the
    // user has since selected the Emby library (no redraw for it yet).
    app.layout.main.browse_destination = Some(TabSelection::Home);
    app.layout.main.left_row_map = vec![None, Some(1)];

    // Wheel scroll over the left pane would move the Emby cursor (delta +1).
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 12, 6));
    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 0,
        "stale-destination wheel scroll must not move the Emby cursor"
    );

    // A single click on row 1 would select item 1 if it reached the Emby
    // handler; the stale layout must swallow it.
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 12, 6));
    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 0,
        "stale-destination single click must not move the Emby cursor"
    );

    // A double-click (second click) would activate row 1 (drill into a
    // folder); it must be swallowed too.
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 12, 6));
    assert_eq!(
        app.libs[0].nav_stack.len(),
        1,
        "stale-destination double-click must not activate an Emby row"
    );
}

/// Control for the above: when the installed `browse_destination` DOES match
/// the selected destination, browse mouse handling proceeds normally.
#[test]
fn browse_mouse_proceeds_when_installed_layout_describes_selected_destination() {
    let mut app = make_library_app(0);
    app.layout.main.browse_destination = Some(TabSelection::EmbyLibrary(0));
    app.layout.main.left_row_map = vec![Some(1)];

    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 12, 6));
    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 1,
        "matching-destination wheel scroll must move the Emby cursor"
    );
}

/// 5.2b: the tag gate (design §4) governs only Service browse geometry. A
/// queue click is not a browse surface, so it stays live during the one-frame
/// window in which the installed completed-frame layout still describes a
/// previous destination -- queue focus works the instant a tab switch lands.
#[test]
fn queue_click_stays_live_when_installed_browse_layout_is_stale() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::Home;
    // The installed frame was rendered for the Emby library, but the user has
    // switched to Home before a redraw (the stale one-frame window).
    app.layout.main.browse_destination = Some(TabSelection::EmbyLibrary(0));
    app.layout.main.queue_area = Rect::new(0, 0, 20, 11);
    for i in 0..5 {
        let mut item = make_item(&format!("Queued {i}"), "Movie");
        item.id = format!("queued-{i}");
        app.player_tab.append_item(item);
    }
    app.player_tab.queue_cursor = 0;
    // Row 3 of the queue pane maps to queue item index 2.
    app.layout.main.queue_row_map = vec![None, None, None, Some(2)];

    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 5, 3));

    assert_eq!(
        app.panel_focus,
        PanelFocus::Queue,
        "queue click must focus the queue even while a stale browse layout is installed"
    );
    assert_eq!(
        app.player_tab.queue_cursor, 2,
        "queue click must move the queue cursor despite the stale browse layout"
    );
}

/// 5.4: an Audiobookshelf-visited layout (_show-mode_ `left_item_rows` plus
/// the tag) cannot drive an Emby mouse gesture after a destination switch:
/// the Emby cursor and ABS selection must both stay untouched, proving no
/// Service consumes another's published fields.
#[test]
fn abs_layout_state_cannot_drive_emby_mouse_after_switch() {
    let mut app = make_library_app(0);
    // A parsed Audiobookshelf library peer exists.
    let library = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut state =
        super::types_audiobookshelf_browse::AudiobookshelfBrowseState::new(library.clone());
    state.append_page(
        0,
        20,
        1,
        vec![mbv_core::audiobookshelf::AudiobookshelfShow {
            library_item_id: "show-a".into(),
            title: "Show A".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    state.episode_selection = Some(0);
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_browse.push(state);

    // Imitate a completed Audiobookshelf episode-mode frame: the ABS
    // published fields are installed and the tag names Audiobookshelf.
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.layout.main.browse_destination = Some(TabSelection::AudiobookshelfLibrary(0));
    app.layout.main.audiobookshelf_episode_rows = vec![(
        Rect {
            x: 10,
            y: 5,
            width: 20,
            height: 1,
        },
        1usize,
    )];
    app.layout.main.left_item_rows = vec![vec![0u64 as usize]];
    app.layout.main.left_screen_offset = 0;

    // The user switches to the Emby library before a fresh frame redraws.
    app.tab = TabSelection::EmbyLibrary(0);

    // A click at the Audiobookshelf episode-row location: with the tag
    // naming Audiobookshelf but the tab naming Emby, the whole gesture is
    // a no-op -- neither the Emby cursor nor ABS episode selection changes.
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 12, 5));
    assert_eq!(
        app.libs[0].nav_stack[0].cursor, 0,
        "ABS geometry must not move the Emby cursor"
    );
    assert_eq!(
        app.audiobookshelf_browse[0].episode_selection.unwrap(),
        0,
        "Emby-directed click must not re-interpret ABS episode rows"
    );
}

#[test]
fn narrow_tv_episode_target_precedes_parent_hero() {
    let mut app = make_library_app(0);
    app.libs[0].library.collection_type = "tvshows".into();
    app.libs[0].nav_stack[0].items[0].item_type = "Series".into();
    app.layout.main.browse_destination = Some(TabSelection::EmbyLibrary(0));
    app.layout.main.hero_area = Rect::new(10, 5, 20, 8);
    app.layout.main.tv_wide_episode_rows = vec![(Rect::new(11, 8, 18, 1), 0)];

    assert!(app.click_set_cursor(12, 8));
    assert_eq!(app.libs[0].series_selection, Some(0));
}
