//! Routing matrix: focus precedence and policy rows.

use super::tests_routing_matrix_support::*;
use crate::app::action::Command;
use crate::app::components::{BrowserKey, BrowserKind, ComponentId, ModalId, Msg, OverlayId, QueueRequest, ShellRequest};
use crate::app::components::msg::{ConfirmIntent, ContextMenuIntent};
use crate::app::input_resolver::KeyChord;
use crate::app::router::{resolve_router_outcome, resolve_router_outcome_with_focused, RouterOutcome, RouterSnapshot};
use crate::app::shell::apply_router_outcome;
use crate::app::types_playback::QueueScope;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::config::ServiceKind;

#[test]
fn queue_focus_routes_queue_chord_to_queue_owner() {
    let leaf = Some(Msg::Queue(QueueRequest::Scope(
        QueueScope::Local,
    )));
    let out = fold_tick(
        leaf,
        key(KeyCode::Char('[')),
        Some(ComponentId::Queue),
        idle_snapshot(),
    );
    assert_eq!(out.len(), 1, "Queue's own scope request must stand");
    assert!(matches!(
        &out[0],
        Msg::Queue(QueueRequest::Scope(_))
    ));
}
#[test]
fn library_focus_routes_bracket_to_library_leaf() {
    let leaf = Some(Msg::Shell(ShellRequest::BrowserCycleLetterPill { delta: -1 }));
    let out = fold_tick(
        leaf,
        key(KeyCode::Char('[')),
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        idle_snapshot(),
    );
    assert_eq!(out.len(), 1);
    assert!(matches!(
        &out[0],
        Msg::Shell(ShellRequest::BrowserCycleLetterPill { delta: -1 })
    ));
}
#[test]
fn ctrl_a_under_library_focus_is_enqueue_not_audio_toggle() {
    let leaf = Some(Msg::Shell(ShellRequest::BrowserEnqueue {
        item: crate::app::tests::make_item("item", "Movie"),
    }));
    let out = fold_tick(
        leaf,
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        active_snapshot(),
    );
    assert_eq!(out.len(), 1, "Ctrl+a must reach the leaf as enqueue-selected");
    assert!(matches!(
        &out[0],
        Msg::Shell(ShellRequest::BrowserEnqueue { .. })
    ));
    // The playback command table's `'a'` arm is `!ctrl`-guarded; assert the
    // audio toggle does NOT resolve for Ctrl+a even with playback active.
    let playback_cmd = crate::app::action::playback_command_for_key(
        KeyChord {
            code: KeyCode::Char('a'),
            mods: KeyModifiers::CONTROL,
        },
        true,
        false,
    );
    assert!(
        playback_cmd.is_none(),
        "audio toggle must not claim Ctrl+a (enqueue-before-playback, #209)"
    );
}
#[test]
fn lib_key_ctrl_catchall_swallows_unmapped_chord() {
    let out = fold_tick(
        None,
        KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        idle_snapshot(),
    );
    assert!(
        out.is_empty(),
        "an unmapped Ctrl chord under Library focus must be swallowed (no queue undo)"
    );
}
