//! Routing matrix: focus precedence and policy rows.

use super::support::*;
use crate::app::components::{ComponentId, Msg, QueueRequest, ShellRequest};
use crate::app::state::types::playback::QueueScope;
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
    let leaf = Some(Msg::Shell(Box::new(
        ShellRequest::EmbyLibraryLatestSelected,
    )));
    let out = fold_tick(
        leaf,
        key(KeyCode::Char('[')),
        Some(ComponentId::Library),
        idle_snapshot(),
    );
    assert_eq!(out.len(), 1);
    assert!(matches!(
        &out[0],
        Msg::Shell(ref shell_boxed)
     if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryLatestSelected)));
}
#[test]
fn global_hide_visual_slot_discards_feeds_h_move_up_message() {
    // Spec scenario: the global `h` hide action takes precedence over Feeds'
    // local vim-style move-up, represented by its selection projection message.
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::SelectionProjection(
        crate::app::components::media_list::SelectionSummary {
            count: 1,
            origin: crate::app::components::media_list::SelectionOrigin::Library(
                crate::app::components::media_list::LibrarySelectionOrigin::Feeds,
            ),
        },
    ))));
    let out = fold_tick_focused(
        leaf,
        key(KeyCode::Char('h')),
        Some(ComponentId::Library),
        idle_snapshot(),
    );
    assert!(
        out.is_empty(),
        "global hide-visual-slot command must discard Feeds' local h message"
    );
}

#[test]
fn ctrl_a_under_library_focus_is_enqueue_not_audio_toggle() {
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryEnqueue {
        item: crate::app::tests::make_item("item", "Movie"),
    })));
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
        Msg::Shell(ref shell_boxed)
     if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryEnqueue { .. })));
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

/// Grouped Music's tree navigation chords are leaf-local: the router claims
/// none of them. Up/Down/Home/End/PageUp/PageDown and both plain horizontal
/// arrows always reach the focused leaf, in every panel mode — the panel
/// focus switch is `Ctrl+Left`/`Ctrl+Right` (user decision 2026-09-20: plain
/// Left conflicted with the tree's parent/collapse chord in the Both layout).
#[test]
fn library_focus_tree_navigation_chords_stay_leaf_local() {
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::MusicGroupSwitch {
        delta: 1,
    })));
    for code in [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
    ] {
        let out = fold_tick(
            leaf.clone(),
            key(code),
            Some(ComponentId::Library),
            idle_snapshot(),
        );
        assert_eq!(out.len(), 1, "{code:?} must reach the focused Music leaf");
    }

    let right = fold_tick(
        leaf.clone(),
        key(KeyCode::Right),
        Some(ComponentId::Library),
        idle_snapshot(),
    );
    assert_eq!(
        right.len(),
        1,
        "plain Right under Library focus reaches the tree (no panel_right)"
    );

    // Both layout, Library focused: plain Left is the tree's chord too, and
    // the panel command moved to Ctrl+Left.
    let left_both = fold_tick(
        leaf,
        key(KeyCode::Left),
        Some(ComponentId::Library),
        idle_snapshot(),
    );
    assert_eq!(
        left_both.len(),
        1,
        "plain Left under Library focus reaches the tree, not the panel command"
    );
}

/// The panel-focus switch is the Ctrl chord in both directions, and each
/// direction is gated on the opposite panel holding focus.
#[test]
fn panel_focus_switches_on_the_ctrl_arrows_only() {
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::MusicGroupSwitch {
        delta: 1,
    })));
    let ctrl_left = KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL);
    let ctrl_right = KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL);

    assert_eq!(
        fold_tick(
            leaf.clone(),
            ctrl_left,
            Some(ComponentId::Library),
            idle_snapshot()
        ),
        Vec::new(),
        "Ctrl+Left is the Both-layout panel command, not the tree's"
    );

    let mut queue = idle_snapshot();
    queue.panel_focus = crate::app::PanelFocus::Queue;
    assert_eq!(
        fold_tick(leaf.clone(), ctrl_right, Some(ComponentId::Queue), queue),
        Vec::new(),
        "Ctrl+Right is the panel command while the Queue holds focus"
    );

    // The same Ctrl chords are inert on the panel that already holds focus,
    // so the focused leaf's own message stands.
    let mut library = idle_snapshot();
    library.panel_focus = crate::app::PanelFocus::Library;
    library.panel_mode = crate::app::state::types::settings::PanelMode::Both;
    assert_eq!(
        fold_tick(
            leaf.clone(),
            ctrl_right,
            Some(ComponentId::Library),
            library
        )
        .len(),
        1,
        "Ctrl+Right with Library focus already held changes nothing"
    );
    assert_eq!(
        fold_tick(leaf, ctrl_left, Some(ComponentId::Queue), queue).len(),
        1,
        "Ctrl+Left with Queue focus already held changes nothing"
    );
}
