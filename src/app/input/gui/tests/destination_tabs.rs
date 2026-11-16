//! The GUI Library's destination steps (`gui-pointer-target-interaction`
//! design D2): the window content takes neither Tab nor Shift+Tab, so the
//! keys reach the core's keyboard router, and the numbers reach it as a
//! numbered destination. Each opens through the one open step the window's
//! open-destination command takes, so a step opens as a press on the tab does
//! and never opens a destination the GUI does not present.

use mbv_emby_model::test_support::make_item;
use mbv_ui_model::library_tab::LibraryTab;
use mbv_ui_model::tab_selection::TabSelection;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::app::input::gui::GuiInput;
use crate::app::tests::make_app_stub;

/// A catalog with a Grouped Music library at position 1 and a Shows library
/// at position 2, Music active: the shortest catalog in which a tab step has
/// a presented destination to land on.
fn music_and_shows_app() -> App {
    let mut app = make_app_stub();
    app.music_levels = vec!["group".to_string(), "album".to_string()];
    let mut music = make_item("Music", "CollectionFolder");
    music.id = "lib-music".to_string();
    music.collection_type = "music".to_string();
    app.libs.push(LibraryTab::new(music));
    let mut shows = make_item("Shows", "CollectionFolder");
    shows.id = "lib-tv".to_string();
    shows.collection_type = "tvshows".to_string();
    let mut shows_tab = LibraryTab::new(shows);
    shows_tab.library_total = Some(0);
    app.libs.push(shows_tab);
    app.tab = TabSelection::EmbyLibrary(0);
    app
}

/// Press one key through the GUI's routing entry with no focused leaf, as the
/// window's forwarder does for a key no surface takes.
fn press(app: &mut App, key: Key) {
    assert!(
        !app.route_gui_input(GuiInput::Key(KeyEvent::new(key, KeyModifiers::NONE)), None),
        "{key:?} asked to quit"
    );
}

// Contract (spec `gui-client`, "A router destination step opens a presented
// destination"): Tab from Music opens the next presented destination (Shows)
// and starts its root load, as a press on its tab does.
#[test]
fn tab_from_music_opens_shows_and_starts_its_root_load() {
    let mut app = music_and_shows_app();
    app.enable_gui_client();

    press(&mut app, Key::Tab);

    assert_eq!(app.tab, TabSelection::EmbyLibrary(1));
    assert!(
        app.gui_tv_shows.contains_key(&1),
        "the open seeded the Shows destination's mode, as the window's open step does"
    );
    assert!(
        app.libs[1]
            .nav_stack
            .first()
            .is_some_and(|level| level.loading),
        "the Shows destination's root load started"
    );
}

// Contract (spec `gui-client`, "Tab skips a destination that the GUI does not
// present"): a step visits only the presented tabs, so a position past the
// presented row opens nothing. No catalog tab is unpresented today (every
// library, Audiobookshelf library, and Feeds tab presents), so the
// unpresented position is the one past the row's end — the same case the
// window's open-destination command guards.
#[test]
fn a_router_step_opens_no_destination_the_gui_does_not_present() {
    let mut app = music_and_shows_app();
    app.enable_gui_client();

    // Position 3 names no tab: Home is 0 and the two libraries take 1 and 2.
    press(&mut app, Key::Char('4'));

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert!(
        app.libs[1].nav_stack.is_empty(),
        "the unpresented position started no load"
    );
}
