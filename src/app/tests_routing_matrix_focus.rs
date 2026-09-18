//! Routing matrix: focus precedence and policy rows.

use super::tests_routing_matrix_support::*;
use crate::app::components::{ComponentId, Msg, QueueRequest, ShellRequest};
use crate::app::types_playback::QueueScope;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn queue_focus_routes_queue_chord_to_queue_owner() {
    let leaf = Some(Msg::Queue(QueueRequest::Scope(QueueScope::Local)));
    let out = fold_tick(
        leaf,
        key(KeyCode::Char('[')),
        Some(ComponentId::Queue),
        idle_snapshot(),
    );
    assert_eq!(out.len(), 1, "Queue's own scope request must stand");
    assert!(matches!(&out[0], Msg::Queue(QueueRequest::Scope(_))));
}
#[test]
fn library_focus_routes_bracket_to_library_leaf() {
    let leaf = Some(Msg::Shell(ShellRequest::EmbyLibraryCycleLetterPill {
        delta: -1,
    }));
    let out = fold_tick(
        leaf,
        key(KeyCode::Char('[')),
        Some(ComponentId::Library),
        idle_snapshot(),
    );
    assert_eq!(out.len(), 1);
    assert!(matches!(
        &out[0],
        Msg::Shell(ShellRequest::EmbyLibraryCycleLetterPill { delta: -1 })
    ));
}
#[test]
fn ctrl_a_under_library_focus_is_enqueue_not_audio_toggle() {
    let leaf = Some(Msg::Shell(ShellRequest::EmbyLibraryEnqueue {
        item: crate::app::tests::make_item("item", "Movie"),
    }));
    let out = fold_tick(
        leaf,
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        Some(ComponentId::Library),
        active_snapshot(),
    );
    assert_eq!(
        out.len(),
        1,
        "Ctrl+a must reach the leaf as enqueue-selected"
    );
    assert!(matches!(
        &out[0],
        Msg::Shell(ShellRequest::EmbyLibraryEnqueue { .. })
    ));
    // Ctrl+a resolves through no policy layer (exact-chord matching keeps
    // superset chords inert), so the audio toggle cannot claim it even with
    // playback active — the FallThrough above is the enqueue-before-playback
    // guarantee (#209).
}
#[test]
fn lib_key_ctrl_catchall_swallows_unmapped_chord() {
    let out = fold_tick(
        None,
        KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
        Some(ComponentId::Library),
        idle_snapshot(),
    );
    assert!(
        out.is_empty(),
        "an unmapped Ctrl chord under Library focus must be swallowed (no queue undo)"
    );
}
