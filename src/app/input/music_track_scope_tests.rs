use super::music_track_test_support::*;
use crate::app::components::library_panel::owner::LibraryContentOwner;
use crate::app::components::msg::Msg;
use crate::app::components::{ComponentId, ShellRequest};
use crate::app::shell::Model;
use tuirealm::event::{Key, KeyEvent as TuiKeyEvent, KeyModifiers as TuiKeyModifiers};

/// Wide music-group fixture with `album-1`'s cached tracks.
fn wide_track_focus_model(track_count: usize) -> (Model, ComponentId) {
    let mut app = make_music_album_app();
    push_tracks(&mut app, "album-1", track_count);
    let mut model = Model::new(app);
    model.app.terminal_width = 160;
    model.app.terminal_height = 40;
    model.push_music_workspace_content();
    model.sync_active_destination();
    let id = ComponentId::Library;
    (model, id)
}

fn enter_track_focus(model: &mut Model, _id: &ComponentId) {
    model.test_music_owner_mut().enter_track_focus();
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

    // `current_lib_item` takes the resolved cursor (task 4.3, R1); the
    // fixture sits at album-level cursor 0.
    let item = app.current_lib_item(0, 0);

    let item = item.expect("current_lib_item should resolve the selected album");
    assert_eq!(item.id, "album-1");
    assert!(item.is_folder, "list mode must resolve to the album folder");
}

#[test]
fn focused_music_track_in_track_mode_resolves_focused_track() {
    let (mut model, id) = wide_track_focus_model(3);
    enter_track_focus(&mut model, &id);

    // The component is authoritative: it resolves the owner-selected album and
    // track and carries both in the typed activation request (D4).
    let message = model.test_music_owner_mut().on_key(&TuiKeyEvent {
        code: Key::Enter,
        modifiers: TuiKeyModifiers::NONE,
    });
    let Some(Msg::Shell(shell_boxed)) = message else {
        panic!("expected a track activation, got {message:?}");
    };
    let ShellRequest::MusicTrackActivate { album_id, track } = *shell_boxed else {
        panic!("expected a track activation, got {shell_boxed:?}");
    };
    assert_eq!(album_id, "album-1");
    assert_eq!(track.id, "album-1-track-0");
    assert!(!track.is_folder, "track mode must resolve to the track");
}
