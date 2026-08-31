//! Routing matrix: playback precedence and policy rows.

use super::tests_routing_matrix_support::*;
use crate::app::action::Command;
use crate::app::components::{BrowserKey, BrowserKind, ComponentId, Msg, ShellRequest};
use crate::app::router::{resolve_router_outcome_with_focused, RouterOutcome, RouterSnapshot};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::config::ServiceKind;

#[test]
fn playback_gating_space_first_press_falls_through() {
    let leaf = Some(Msg::Shell(ShellRequest::Quit));
    let out = fold_tick(
        leaf,
        key(KeyCode::Char(' ')),
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        active_snapshot(),
    );
    assert_eq!(
        out.len(),
        1,
        "first Space press must fall through (browse leaf keeps its request)"
    );
}
#[test]
fn playback_gating_esc_first_press_falls_through() {
    let leaf = Some(Msg::Shell(ShellRequest::BrowserBack));
    let out = fold_tick(
        leaf,
        key(KeyCode::Esc),
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib".into(),
            kind: BrowserKind::Generic,
        })),
        active_snapshot(),
    );
    assert_eq!(out.len(), 1);
    assert!(matches!(&out[0], Msg::Shell(ShellRequest::BrowserBack)));
}
#[test]
fn playback_gating_space_second_press_claims_toggle() {
    let mut snapshot = active_snapshot();
    snapshot.space_double_tap = true;
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char(' ')), &snapshot, None),
        RouterOutcome::Command(Command::TogglePlayPause)
    );
}
#[test]
fn playback_gating_esc_second_press_claims_stop() {
    let mut snapshot = active_snapshot();
    snapshot.esc_double_tap = true;
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Esc), &snapshot, None),
        RouterOutcome::Command(Command::Stop)
    );
}
#[test]
fn playback_policy_preserves_per_key_eligibility() {
    let active = active_snapshot();
    assert_eq!(
        resolve_router_outcome_with_focused(
            key(KeyCode::Char('<')),
            &active, None),
        RouterOutcome::Command(Command::SeekRelative(-5.0))
    );
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('a')), &active, None),
        RouterOutcome::Command(Command::ToggleMuteOrCycleAudio)
    );
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &active, None),
        RouterOutcome::FallThrough
    );
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('m')), &idle_snapshot(), None),
        RouterOutcome::Command(Command::ToggleMute)
    );
}
#[test]
fn visualizer_resolves_to_router_command() {
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('v')), &idle_snapshot(), None),
        RouterOutcome::Command(Command::ToggleVisualizer)
    );
}
#[test]
fn playback_and_visualizer_commands_are_swallowed_under_blocking_overlay() {
    let snapshot = RouterSnapshot {
        player_active: true,
        blocking_overlay_open: true,
        space_double_tap: true,
        ..RouterSnapshot::default()
    };

    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('v')), &snapshot, None),
        RouterOutcome::Swallow
    );
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('m')), &snapshot, None),
        RouterOutcome::Swallow
    );
}
#[test]
fn idle_feed_path_uses_connected_session_not_broad_playback_route() {
    let snapshot = RouterSnapshot {
        has_remote_session: true,
        idle_feed_link_available: true,
        ..RouterSnapshot::default()
    };
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('o')), &snapshot, None),
        RouterOutcome::Command(Command::OpenIdleFeedLink)
    );

    let connected = RouterSnapshot {
        connected_session_id_present: true,
        ..snapshot
    };
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('o')), &connected, None),
        RouterOutcome::FallThrough
    );
}
