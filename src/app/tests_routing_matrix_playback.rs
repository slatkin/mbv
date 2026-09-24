//! Routing matrix: playback precedence and policy rows.

use super::tests_routing_matrix_support::*;
use crate::app::dispatch::action::Command;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::input::router::{resolve_router_outcome_with_focused, RouterOutcome, RouterSnapshot};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn playback_gating_space_falls_through_to_consumed_leaf() {
    let leaf = Some(Msg::Shell(ShellRequest::Quit));
    let out = fold_tick(
        leaf,
        key(KeyCode::Char(' ')),
        Some(ComponentId::Library),
        active_snapshot(),
    );
    assert_eq!(
        out.len(),
        1,
        "consumed Space press must fall through (browse leaf keeps its request)"
    );
}
#[test]
fn playback_gating_esc_falls_through_to_consumed_leaf() {
    let leaf = Some(Msg::Shell(ShellRequest::EmbyLibraryBack));
    let out = fold_tick(
        leaf,
        key(KeyCode::Esc),
        Some(ComponentId::Library),
        active_snapshot(),
    );
    assert_eq!(out.len(), 1);
    assert!(matches!(&out[0], Msg::Shell(ShellRequest::EmbyLibraryBack)));
}
#[test]
fn playback_gating_space_claims_deferred_toggle() {
    let snapshot = active_snapshot();
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char(' ')), &snapshot, None, &default_keybinds()),
        RouterOutcome::Deferred(Command::TogglePlayPause)
    );
}
#[test]
fn playback_gating_esc_claims_deferred_stop() {
    let snapshot = active_snapshot();
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Esc), &snapshot, None, &default_keybinds()),
        RouterOutcome::Deferred(Command::Stop)
    );
}
#[test]
fn playback_policy_preserves_per_key_eligibility() {
    let active = active_snapshot();
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('<')), &active, None, &default_keybinds()),
        RouterOutcome::Command(Command::SeekRelative(-5.0))
    );
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('a')), &active, None, &default_keybinds()),
        RouterOutcome::Command(Command::ToggleMuteOrCycleAudio)
    );
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &active,
            None
        , &default_keybinds()),
        RouterOutcome::FallThrough
    );
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('m')), &idle_snapshot(), None, &default_keybinds()),
        RouterOutcome::Command(Command::ToggleMute)
    );
}
#[test]
fn visualizer_resolves_to_router_command() {
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('v')), &idle_snapshot(), None, &default_keybinds()),
        RouterOutcome::Command(Command::ToggleVisualizer)
    );
}
#[test]
fn playback_and_visualizer_commands_are_swallowed_under_blocking_overlay() {
    let snapshot = RouterSnapshot {
        player_active: true,
        blocking_overlay_open: true,
        ..RouterSnapshot::default()
    };

    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('v')), &snapshot, None, &default_keybinds()),
        RouterOutcome::Swallow
    );
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('m')), &snapshot, None, &default_keybinds()),
        RouterOutcome::Swallow
    );
}
/// Task 3.8: the idle-feed open-link gate follows the Queue playback panel's
/// presence, not the panel mode — `queue_only_idle` is the shell's
/// "Queue playback panel mounted and nothing playing" fact (task 3.5 mounts
/// the panel in every queue-visible layout, idle included). The link is
/// suppressed in idle `both` and `queue-only`, and fires in idle
/// `library-only`, where the Library playback panel's strip displays the feed.
#[test]
fn idle_feed_path_uses_connected_session_not_broad_playback_route() {
    let snapshot = RouterSnapshot {
        has_remote_session: true,
        idle_feed_link_available: true,
        ..RouterSnapshot::default()
    };
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('o')), &snapshot, None, &default_keybinds()),
        RouterOutcome::Command(Command::OpenIdleFeedLink)
    );

    let queue_only_idle = RouterSnapshot {
        panel_mode: crate::app::state::types::settings::PanelMode::QueueOnly,
        queue_only_idle: true,
        ..snapshot
    };
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('o')), &queue_only_idle, None, &default_keybinds()),
        RouterOutcome::FallThrough
    );

    // The gate follows the panel's presence, not the mode: the same panel
    // present + idle fact in the two-panel layout is suppressed too.
    let both_idle = RouterSnapshot {
        panel_mode: crate::app::state::types::settings::PanelMode::Both,
        queue_only_idle: true,
        ..snapshot
    };
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('o')), &both_idle, None, &default_keybinds()),
        RouterOutcome::FallThrough
    );

    // Library-only idle: the Queue playback panel is unmounted (the strip
    // displays the feed), so the link opens.
    let library_only_idle = RouterSnapshot {
        panel_mode: crate::app::state::types::settings::PanelMode::LibraryOnly,
        queue_only_idle: false,
        ..snapshot
    };
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('o')), &library_only_idle, None, &default_keybinds()),
        RouterOutcome::Command(Command::OpenIdleFeedLink)
    );

    let connected = RouterSnapshot {
        connected_session_id_present: true,
        ..snapshot
    };
    assert_eq!(
        resolve_router_outcome_with_focused(key(KeyCode::Char('o')), &connected, None, &default_keybinds()),
        RouterOutcome::FallThrough
    );
}
