//! Tests of the destinations the GUI Library presents and the window-facing
//! seams it routes (add-gui-library-panel design D2 and D3 rule 3; row 8.1
//! presents the plain Emby libraries beside Music): the frame's tabs name
//! every Emby library in catalog order and mark the active one current, a
//! Music item's row menu leaves out the played toggle, the openUrl command
//! opens an http or https url through the shared opener and no other scheme,
//! and an interrupted art fetch's retry is due in the window that holds its
//! retry time. The tests sit in their own child module of `gui_library` so
//! that file stays within its line ceiling.

use super::*;
/// A stub `App` whose catalog holds two Grouped Music libraries around a
/// Movies library, so the tabs present every Emby library in catalog order
/// (row 8.1 presents the plain Emby libraries beside Music). No Service
/// client: the tab projection reads the catalog alone.
fn app_with_two_music_libraries() -> App {
    let mut app = make_app_stub();
    app.seed_music_levels_for_test(&["group", "album"]);
    for (name, collection_type) in [
        ("Music A", "music"),
        ("Movies", "movies"),
        ("Music B", "music"),
    ] {
        let mut library = make_item(name, "CollectionFolder");
        library.collection_type = collection_type.to_string();
        library.is_folder = true;
        app.push_library_for_test(LibraryTab::new(library));
    }
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(0));
    app
}

/// The frame's destination tabs (row 4.3, design D2; row 8.1 presents the
/// plain Emby libraries beside Music): one tab per Emby library in catalog
/// order, each naming the position the open-destination command takes.
#[test]
fn the_tabs_present_every_emby_library_in_catalog_order() {
    let app = app_with_two_music_libraries();

    let tabs = gui_library_tabs(&app).tabs;

    assert_eq!(
        tabs,
        vec![
            GuiLibraryTab {
                position: 1,
                label: "Music A".to_string(),
            },
            GuiLibraryTab {
                position: 2,
                label: "Movies".to_string(),
            },
            GuiLibraryTab {
                position: 3,
                label: "Music B".to_string(),
            },
        ],
        "every Emby library presents a tab, and the positions are the open command's"
    );
}

/// The current tab (row 4.3, design D2): the active tab's position while the
/// active destination is one of the presented ones.
#[test]
fn the_current_tab_names_the_active_presented_destination() {
    let mut app = app_with_two_music_libraries();
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(2));

    assert_eq!(gui_library_tabs(&app).current, Some(3));
}

/// The current tab names the plain Emby-library destination the row 8.1
/// presented set added (row 4.3, design D2): the Movies library's tab is
/// current while it is active.
#[test]
fn the_current_tab_names_the_movies_destination() {
    let mut app = app_with_two_music_libraries();
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(1));

    assert_eq!(gui_library_tabs(&app).current, Some(2));
}

/// A Music item's right-click item menu offers the played-state entries no
/// item kind (the context-menu spec's music rule, through the GUI's menu
/// builder): music ignores its played state everywhere, so the mark entries
/// would name changes the UI never shows. An album-folder music item carries
/// the standard folder entries — Play All, Shuffle, Add to Queue — and
/// nothing else.
#[test]
fn a_music_items_row_menu_leaves_out_the_played_toggle() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let app = app_with_stored_groups(
        vec![stored_album(
            "alb-ill",
            "Licensed to Ill",
            "Beastie Boys",
            1986,
        )],
        Vec::new(),
    );
    let mut app = app;
    app.set_panel_focus(PanelFocus::Library);
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::RowMenu {
            row: GuiLibraryMenuRow::Item("alb-ill".into()),
        },
    );

    let menu = gui_library_row_menu(&mut app).expect("the album has a menu");
    let labels: Vec<&str> = menu
        .entries
        .iter()
        .map(|entry| entry.label.as_str())
        .collect();
    assert!(!labels.iter().any(|label| label.contains("Mark")));
    assert_eq!(labels, ["Play", "Add to Queue"]);
}

/// The opener the openUrl seam stages: it records the url the core handed
/// the opener, so the test observes the open without a real browser spawn.
static OPENED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn record_open(url: &str) {
    OPENED.lock().unwrap().push(url.to_string());
}

/// The `openUrl` command's trust boundary: the core opens an http or https
/// url through the shared opener, and any other scheme — the url comes from
/// the window — reaches no opener.
#[test]
fn open_url_opens_an_http_url_and_drops_any_other_scheme() {
    let _guard = crate::app::test_seams::OPEN_URL_TEST_LOCK.lock().unwrap();
    *crate::app::test_seams::OPEN_URL_OVERRIDE.lock().unwrap() = Some(record_open);
    let mut app = make_app_stub();
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::OpenUrl {
            url: "https://www.imdb.com/title/tt0088763/".to_string(),
        },
    );
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::OpenUrl {
            url: "javascript:alert(1)".to_string(),
        },
    );
    *crate::app::test_seams::OPEN_URL_OVERRIDE.lock().unwrap() = None;

    assert_eq!(
        *OPENED.lock().unwrap(),
        vec!["https://www.imdb.com/title/tt0088763/".to_string()],
        "the https url opened and the javascript url dropped"
    );
}

/// An interrupted art fetch's retry is reported by the window that holds its
/// retry time (design D3 rule 3, invariant 27): the core keeps each window's
/// start, so the next window does not report it again.
#[test]
fn an_interrupted_art_retry_comes_due_in_the_window_that_holds_it() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let start = std::time::Instant::now();
    app.interrupt_card_image_fetch_for_test("alb-1:P");
    let hour = std::time::Duration::from_secs(3600);
    let later = start + hour;

    assert!(
        gui_art_retry_due(&app, start, later),
        "the window holds the retry time"
    );
    assert!(
        !gui_art_retry_due(&app, later, later + hour),
        "the next window starts after the retry time"
    );
}
