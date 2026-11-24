//! Routing matrix: playback precedence and policy rows.

use super::support::*;
use mbv_client_core::dispatch::action::{PlaybackIntent, VisualIntent};
use mbv_client_core::input::resolver::KeyChord;
use mbv_client_core::input::router::{RouterOutcome, RouterSnapshot, resolve_router_outcome};
use mbv_client_core::tests::gui_client_capabilities;
use mbv_ui_msg::{ComponentId, Key, KeyModifiers, Msg, ShellRequest, TrackChipKind};

#[test]
fn playback_gating_esc_falls_through_to_consumed_leaf() {
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryBack)));
    let out = fold_tick(
        leaf,
        key(Key::Esc),
        Some(&ComponentId::Library),
        active_snapshot(),
    );
    assert_eq!(out.len(), 1);
    assert!(
        matches!(&out[0], Msg::Shell(shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryBack))
    );
}
#[test]
fn playback_gating_space_toggles_without_a_visual_selection() {
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char(' '))),
            &active_snapshot(),
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(PlaybackIntent::TogglePlayPause.into())
    );
}
#[test]
fn playback_gating_space_defers_to_a_visual_selection() {
    let snapshot = RouterSnapshot {
        visual_selection_active: true,
        ..active_snapshot()
    };
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char(' '))),
            &snapshot,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Deferred(PlaybackIntent::TogglePlayPause.into())
    );
}
#[test]
fn playback_gating_esc_falls_through_with_default_bindings() {
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Esc)),
            &active_snapshot(),
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough
    );
}
#[test]
fn playback_gating_default_stop_chord_commands_a_stop_when_the_gate_is_open() {
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key_mod(Key::Char('S'), KeyModifiers::SHIFT)),
            &active_snapshot(),
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(PlaybackIntent::Stop.into())
    );
}
#[test]
fn playback_gating_default_stop_chord_falls_through_during_text_entry() {
    let mut snapshot = active_snapshot();
    snapshot.overlays.text_entry_focused = true;
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key_mod(Key::Char('S'), KeyModifiers::SHIFT)),
            &snapshot,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough
    );
}
#[test]
fn playback_gating_esc_stop_override_claims_a_deferred_stop() {
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Esc)),
            &active_snapshot(),
            &disarmed(),
            None,
            &keybinds_with_override("stop", "Esc")
        ),
        RouterOutcome::Deferred(PlaybackIntent::Stop.into())
    );
}
#[test]
fn playback_policy_preserves_per_key_eligibility() {
    let active = active_snapshot();
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('<'))),
            &active,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(PlaybackIntent::SeekRelative(-5.0).into())
    );
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('#'))),
            &active,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(PlaybackIntent::CycleAudio.into())
    );
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('a'))),
            &active,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough
    );
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('m'))),
            &idle_snapshot(),
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(PlaybackIntent::ToggleMute.into())
    );
}
#[test]
fn visualizer_resolves_to_router_command() {
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('v'))),
            &idle_snapshot(),
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(VisualIntent::ToggleVisualizer.into())
    );
}
#[test]
fn playback_and_visualizer_commands_are_swallowed_under_blocking_overlay() {
    let snapshot = RouterSnapshot {
        playback: mbv_client_core::input::RouterPlaybackState {
            player_active: true,
            ..Default::default()
        },
        overlays: mbv_client_core::input::RouterOverlayState {
            focus: mbv_client_core::input::OverlayFocus::Blocking,
            ..Default::default()
        },
        ..RouterSnapshot::default()
    };

    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('v'))),
            &snapshot,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Swallow
    );
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('m'))),
            &snapshot,
            &disarmed(),
            None,
            &default_keybinds()
        ),
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
        playback: mbv_client_core::input::RouterPlaybackState {
            remote_target: mbv_client_core::input::RemoteControlTarget::DirectRemote,
            idle_feed_link_available: true,
            ..Default::default()
        },
        ..RouterSnapshot::default()
    };
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('o'))),
            &snapshot,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(VisualIntent::OpenIdleFeedLink.into())
    );

    let queue_only_idle = RouterSnapshot {
        panel_mode: mbv_ui_model::settings::PanelMode::QueueOnly,
        playback: mbv_client_core::input::RouterPlaybackState {
            queue_only_idle: true,
            ..snapshot.playback
        },
        ..snapshot
    };
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('o'))),
            &queue_only_idle,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough
    );

    // The gate follows the panel's presence, not the mode: the same panel
    // present + idle fact in the two-panel layout is suppressed too.
    let both_idle = RouterSnapshot {
        panel_mode: mbv_ui_model::settings::PanelMode::Both,
        playback: mbv_client_core::input::RouterPlaybackState {
            queue_only_idle: true,
            ..snapshot.playback
        },
        ..snapshot
    };
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('o'))),
            &both_idle,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough
    );

    // Library-only idle: the Queue playback panel is unmounted (the strip
    // displays the feed), so the link opens.
    let library_only_idle = RouterSnapshot {
        panel_mode: mbv_ui_model::settings::PanelMode::LibraryOnly,
        playback: mbv_client_core::input::RouterPlaybackState {
            queue_only_idle: false,
            ..snapshot.playback
        },
        ..snapshot
    };
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('o'))),
            &library_only_idle,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(VisualIntent::OpenIdleFeedLink.into())
    );
}

/// The canvas transport's registry actions (add-tauri-gui-client row 6.4,
/// the "back 10 s, forward 30 s" spec): `[`/`]` command the deep seeks in
/// the GUI Client and fall through in the TUI, whose gate stays closed.
#[test]
fn gui_context_seek_brackets_command_only_with_transport_chip_keys() {
    let gui = RouterSnapshot {
        client_capabilities: gui_client_capabilities(),
        ..active_snapshot()
    };
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('['))),
            &gui,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(PlaybackIntent::SeekRelative(-10.0).into())
    );
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char(']'))),
            &gui,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(PlaybackIntent::SeekRelative(30.0).into())
    );
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('['))),
            &active_snapshot(),
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough
    );
}

/// The track chips' popover actions (add-tauri-gui-client row 6.4, the
/// "every action is reachable from the keyboard" spec): `a`/`s` command
/// the popover toggles in the GUI Client — the same dispatch the chips'
/// `OpenTrackPopover` press routes — and fall through in the TUI, whose
/// gate stays closed and which paints no track chip.
#[test]
fn gui_context_popover_chords_command_only_with_popover_chord_keys() {
    let gui = RouterSnapshot {
        client_capabilities: gui_client_capabilities(),
        ..active_snapshot()
    };
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('a'))),
            &gui,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(PlaybackIntent::ToggleTrackPopover(TrackChipKind::Audio).into())
    );
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('s'))),
            &gui,
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(PlaybackIntent::ToggleTrackPopover(TrackChipKind::Subtitles).into())
    );
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('a'))),
            &active_snapshot(),
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough
    );
    assert_eq!(
        resolve_router_outcome(
            KeyChord::from_key(key(Key::Char('s'))),
            &active_snapshot(),
            &disarmed(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough
    );
}

/// The GUI-only Queue controls (gui-named-queues design D8, spec
/// `gui-queue-chooser` "The TUI keeps its keys"): `t`, `Ctrl+T`, and `Ctrl+W`
/// reach the chooser and its controls only in the GUI Client, where the
/// pointer target is the Queue list box. The TUI Client binds none of them, so
/// no queue is chosen, made, or closed by them there.
#[test]
fn gui_queue_control_chords_are_unbound_in_the_tui_client() {
    for chord in [
        key(Key::Char('t')),
        key_mod(Key::Char('t'), KeyModifiers::CONTROL),
        key_mod(Key::Char('w'), KeyModifiers::CONTROL),
    ] {
        assert_eq!(
            resolve_router_outcome(
                KeyChord::from_key(chord),
                &active_snapshot(),
                &disarmed(),
                None,
                &default_keybinds()
            ),
            RouterOutcome::FallThrough,
            "{chord:?} must reach no command in the TUI Client"
        );
    }
}
