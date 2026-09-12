#![allow(dead_code, unused_imports)]

use super::super::*;
use crate::app::components::browser_content::BrowserContent as BrowserOwner;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::Msg;
use crate::app::render::make_movie_app;
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::types_browse::BrowseResting;
use crate::app::{
    App, BrowseLevel, ContextAction, FeedHomeVideoGroup, FeedHomeVideoState, LibraryTab,
    PanelFocus, PanelMode, TabSelection,
};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

/// The embedded Movies/HomeVideos/Generic owner inside the mounted
/// `LibraryPanel` (task 6.1) — the shared-borrow read for pure state checks.
pub(super) fn browser_owner(model: &Model) -> &BrowserOwner {
    let (_, key, _) = model
        .active_migrated_browser_owner()
        .expect("the active library's owner has migrated");
    model
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .owner(&key)
        .and_then(|owner| owner.as_any().downcast_ref::<BrowserOwner>())
        .expect("browser owner installed")
}

/// The embedded owner, mutably (drives its own local key interpretation).
pub(super) fn browser_owner_mut(model: &mut Model) -> &mut BrowserOwner {
    let (_, key, _) = model
        .active_migrated_browser_owner()
        .expect("the active library's owner has migrated");
    model
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        .and_then(|panel| panel.owner_mut(&key))
        .and_then(|owner| owner.as_any_mut().downcast_mut::<BrowserOwner>())
        .expect("browser owner installed")
}

/// Drive one key through the focused `LibraryPanel` into the embedded
/// owner — the panel is the library area's one event boundary, so the old
/// `drive_browser_key(&BrowserComponent)` contract becomes a panel delivery.
pub(super) fn drive_owner_key(model: &mut Model, key: Key, modifiers: KeyModifiers) -> Option<Msg> {
    model
        .application
        .get_component_mut(&ComponentId::Library)
        .expect("Library panel mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: key,
            modifiers,
        }))
}

/// Paint one full frame through `Model::draw_frame` (the live shell paint
/// path) at the given size — the migrated owner's surface, the mounted
/// `LibraryPanel`.
pub(super) fn draw_owner_model(model: &mut Model, width: u16, height: u16) {
    model.app.terminal_width = width;
    model.app.terminal_height = height;
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    // One throwaway draw publishes `root_frame` (tasks 2.1-2.2); the
    // recorded draw then paints the mounted panels.
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
}

/// A generic (non-Movies) Emby library with `n` flat Movie items: below
/// the 82-column breakpoint it never takes the wide-Movies Wide hero
/// rail, so whatever column count the painted pane derives is the plain
/// flat-list stride for both the App and the mounted browser.
pub(super) fn browser_app_with_flat_movies(n: usize) -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Films", "CollectionFolder");
    library.id = "lib-films".into();
    library.is_folder = true;
    library.collection_type = "generic".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-films".into(),
            title: "Films".into(),
            items: make_items(n),
            total_count: n,
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

fn mounted_music_model() -> Model {
    let mut model = Model::new(crate::app::render::make_music_group_app());
    model.sync_music_workspace();
    model.sync_active_destination();
    model
}

fn mounted_tv_model() -> Model {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
    }
    // Wide breakpoint is now driven synchronously by terminal size
    // (`wide_tv_library_area`), not this previous-frame paint rect.
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut model = Model::new(app);
    model.sync_tv_workspace();
    model.sync_active_destination();
    model
}

fn dispatch_component_key(
    model: &mut Model,
    id: &ComponentId,
    code: Key,
    modifiers: KeyModifiers,
) -> ShellRequest {
    let message = model
        .application
        .get_component_mut(id)
        .expect("workspace mounted")
        .on(&Event::Keyboard(KeyEvent { code, modifiers }));
    let Some(Msg::Shell(request)) = message else {
        panic!("workspace key must emit a shell request");
    };
    let mut music_resize = false;
    let mut tv_resize = false;
    model.handle_terminal_message(
        Msg::Shell(request.clone()),
        &mut music_resize,
        &mut tv_resize,
    );
    request
}

#[test]
fn shell_music_ctrl_p_runs_library_play_effect() {
    let mut model = mounted_music_model();
    let id = model
        .music_workspace_id
        .clone()
        .expect("Music workspace mounted");
    let request = dispatch_component_key(&mut model, &id, Key::Char('p'), KeyModifiers::CONTROL);

    assert!(matches!(
        request,
        ShellRequest::EmbyLibraryPlay { ref item } if item.id == "album-1"
    ));
    assert_eq!(model.app.status, "Emby is unavailable");
}

#[test]
fn shell_music_ctrl_a_runs_library_enqueue_effect() {
    let mut model = mounted_music_model();
    let id = model
        .music_workspace_id
        .clone()
        .expect("Music workspace mounted");
    let request = dispatch_component_key(&mut model, &id, Key::Char('a'), KeyModifiers::CONTROL);

    assert!(matches!(
        request,
        ShellRequest::EmbyLibraryEnqueue { ref item } if item.id == "album-1"
    ));
    let queued = model.app.player_tab.emby_items();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].id, "album-1");
}

#[test]
fn shell_tv_ctrl_w_runs_library_toggle_watched_effect() {
    let mut model = mounted_tv_model();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
    let request = dispatch_component_key(&mut model, &id, Key::Char('w'), KeyModifiers::CONTROL);

    assert!(matches!(
        request,
        ShellRequest::EmbyLibraryToggleWatched { ref item } if item.id == "movie-focused"
    ));
    assert_eq!(model.app.status, "Emby is unavailable");
}

#[test]
fn shell_tv_ctrl_s_runs_library_shuffle_effect() {
    let mut model = mounted_tv_model();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
    let request = dispatch_component_key(&mut model, &id, Key::Char('s'), KeyModifiers::CONTROL);

    assert!(matches!(
        request,
        ShellRequest::EmbyLibraryShuffle { ref item } if item.id == "movie-focused"
    ));
    assert_eq!(model.app.status, "Emby is unavailable");
}

#[test]
fn shell_music_ctrl_r_runs_library_rescan_effect() {
    let mut model = mounted_music_model();
    let id = model
        .music_workspace_id
        .clone()
        .expect("Music workspace mounted");
    let request = dispatch_component_key(&mut model, &id, Key::Char('r'), KeyModifiers::CONTROL);

    assert_eq!(request, ShellRequest::EmbyLibraryRescan);
    match model.app.pending_overlay.as_ref() {
        Some(crate::app::types_overlay::OverlayRequest::Confirm(modal)) => {
            assert!(matches!(
                modal.on_confirm,
                crate::app::ConfirmAction::RescanLibrary(0)
            ));
        }
        _ => panic!("Ctrl+R must raise the rescan confirmation"),
    }
}

#[test]
fn shell_tv_refresh_runs_library_refresh_effect() {
    let mut model = mounted_tv_model();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
    let request = dispatch_component_key(&mut model, &id, Key::Char('r'), KeyModifiers::NONE);

    assert_eq!(request, ShellRequest::EmbyLibraryRefresh);
    assert!(model.app.libs[0].nav_stack[0].loading);
}

/// A single Emby Movies library holding one folder (`Folder A`) and one
/// playable movie (`Movie B`), shared by the shell browser effect, geometry
/// and position tests.
pub(super) fn browser_app_with_folder_and_movie() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let mut folder = make_item("Folder A", "CollectionFolder");
    folder.id = "folder-a".into();
    folder.is_folder = true;

    let mut movie = make_item("Movie B", "Movie");
    movie.id = "movie-b".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![folder, movie],
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
