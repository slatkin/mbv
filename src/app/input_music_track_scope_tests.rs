#![allow(dead_code, unused_imports)]

use super::music_track_test_support::*;
use super::*;
use crate::app::components::msg::Msg;
use crate::app::components::music_workspace::MusicWorkspaceComponent;
use crate::app::components::{ComponentId, ShellRequest};
use crate::app::shell::Model;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io::{Read, Write};
use tuirealm::component::AppComponent;
use tuirealm::event::{Event, Key, KeyEvent as TuiKeyEvent, KeyModifiers as TuiKeyModifiers};

/// Wide music-group fixture with `album-1`'s cached tracks.
fn wide_track_focus_model(track_count: usize) -> (Model, ComponentId) {
    let mut app = make_music_album_app();
    push_tracks(&mut app, "album-1", track_count);
    let mut model = Model::new(app);
    model.app.layout.main.wide_music_area = Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("wide Music workspace mounted");
    (model, id)
}

fn enter_track_focus(model: &mut Model, id: &ComponentId) {
    model
        .application
        .get_component_mut(id)
        .unwrap()
        .on(&Event::Keyboard(TuiKeyEvent {
            code: Key::Enter,
            modifiers: TuiKeyModifiers::NONE,
        }));
}

// ── Task 4: scope-correct actions (#145), re-homed at the shell boundary ──

#[test]
fn current_lib_item_in_list_mode_returns_album_folder_not_a_track() {
    // Regression: album-list mode must keep resolving to the selected album
    // folder itself, exactly as before Task 4. Focused-track resolution
    // lives at the shell/component boundary now (`focused_music_track`), not
    // in `current_lib_item`.
    let mut app = make_music_album_app();
    push_tracks(&mut app, "album-1", 3);

    let item = app.current_lib_item(0);

    let item = item.expect("current_lib_item should resolve the selected album");
    assert_eq!(item.id, "album-1");
    assert!(item.is_folder, "list mode must resolve to the album folder");
}

#[test]
fn focused_music_track_in_track_mode_resolves_focused_track() {
    let (mut model, id) = wide_track_focus_model(3);
    enter_track_focus(&mut model, &id);

    let (album_id, track) = model
        .focused_music_track(0)
        .expect("focused track should resolve");

    assert_eq!(album_id, "album-1");
    assert_eq!(track.id, "album-1-track-0");
    assert!(!track.is_folder, "track mode must resolve to the track");
}

#[test]
fn focused_music_track_falls_back_safely_when_cache_missing() {
    // Async fetch still in flight: `album_tracks_cache` has no entry for
    // "album-1" yet (the cursor index still resolves to nothing). Must not
    // panic and the shell target must stay `None`.
    let (mut model, _) = wide_track_focus_model(0);
    // `push_tracks(.., 0)` inserts an empty vec; drop even that so the cache
    // genuinely has no entry for the selected album.
    model.app.album_tracks_cache.remove("album-1");
    assert!(!model.app.album_tracks_cache.contains_key("album-1"));
    assert!(
        model.focused_music_track(0).is_none(),
        "cache-missing focused track must stay None, not panic"
    );
}

#[test]
fn enter_in_track_mode_with_missing_cache_does_not_panic() {
    // Track focused in the component but the cache is then dropped: the
    // activation message still fires, the shell resolves nothing, and the
    // nav stack is untouched.
    let (mut model, id) = wide_track_focus_model(2);
    enter_track_focus(&mut model, &id);
    model.app.album_tracks_cache.remove("album-1");
    let nav_len_before = model.app.libs[0].nav_stack.len();

    let msg = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(TuiKeyEvent {
            code: Key::Enter,
            modifiers: TuiKeyModifiers::NONE,
        }));
    assert!(matches!(
        msg,
        Some(Msg::Shell(ShellRequest::MusicTrackActivate))
    ));
    assert_eq!(model.app.libs[0].nav_stack.len(), nav_len_before);
}

#[test]
fn context_menu_in_list_mode_offers_folder_scoped_actions_for_selected_album() {
    // Regression: album-list mode's context menu must still target the
    // selected ALBUM's id via the folder-scoped actions.
    let mut app = make_music_album_app();

    app.open_context_menu(false, None);

    let menu = match app.pending_overlay.as_ref() {
        Some(crate::app::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("context menu should open"),
    };
    let actions: Vec<_> = menu
        .entries
        .iter()
        .filter_map(|e| e.action.clone())
        .collect();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContextAction::PlayFolder(id) if id == "album-1")),
        "expected PlayFolder(\"album-1\"), got: {actions:?}"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContextAction::ShuffleFolder(id) if id == "album-1")),
        "expected ShuffleFolder(\"album-1\"), got: {actions:?}"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContextAction::EnqueueFolder(item) if item.id == "album-1")),
        "expected EnqueueFolder(album-1), got: {actions:?}"
    );
}

#[test]
fn context_menu_for_focused_track_offers_track_scoped_actions_not_folder_actions() {
    // '.' in track mode reaches the shell as `MusicTrackContextMenu`, which
    // resolves the focused track and raises the menu for that item -- the
    // generic per-item actions, never album-folder scoped ones.
    let (mut model, id) = wide_track_focus_model(3);
    enter_track_focus(&mut model, &id);
    let (_, track) = model
        .focused_music_track(0)
        .expect("focused track should resolve");

    model.app.open_context_menu_for(track);

    let menu = match model.app.pending_overlay.as_ref() {
        Some(crate::app::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("context menu should open"),
    };
    let actions: Vec<_> = menu
        .entries
        .iter()
        .filter_map(|e| e.action.clone())
        .collect();
    assert!(
        actions.iter().any(|a| matches!(a, ContextAction::Play)),
        "track mode must offer the generic per-item Play action, got: {actions:?}"
    );
    assert!(
        actions.iter().any(|a| matches!(a, ContextAction::Enqueue)),
        "track mode must offer the generic per-item Enqueue action, got: {actions:?}"
    );
    assert!(
        !actions.iter().any(|a| matches!(
            a,
            ContextAction::PlayFolder(_)
                | ContextAction::ShuffleFolder(_)
                | ContextAction::EnqueueFolder(_)
        )),
        "track mode must not offer album-folder-scoped actions, got: {actions:?}"
    );
}
