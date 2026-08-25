use super::*;
use crate::app::action::Command;

fn snap(active: bool, remote: bool) -> InputSnapshot {
    InputSnapshot {
        player_active: active,
        has_remote_session: remote,
    }
}

#[test]
fn playback_context_maps_gated_key_to_command_when_active() {
    let r = resolve_key(
        InputContext::Playback,
        &snap(true, false),
        KeyChord::new(KeyCode::Char(' '), KeyModifiers::NONE),
    );
    assert_eq!(r, KeyResolution::Command(Command::TogglePlayPause));
}

#[test]
fn playback_context_falls_through_when_gate_closed() {
    // Space is a no-op that must reach the view handler when nothing plays.
    let r = resolve_key(
        InputContext::Playback,
        &snap(false, false),
        KeyChord::new(KeyCode::Char(' '), KeyModifiers::NONE),
    );
    assert_eq!(r, KeyResolution::FallThrough);
}

#[test]
fn playback_context_falls_through_on_unbound_key() {
    let r = resolve_key(
        InputContext::Playback,
        &snap(true, false),
        KeyChord::new(KeyCode::Char('x'), KeyModifiers::NONE),
    );
    assert_eq!(r, KeyResolution::FallThrough);
}

#[test]
fn playback_context_esc_stops_when_player_active() {
    // Inline album track-mode Esc handling moved into
    // `MusicWorkspaceComponent` (the component consumes Esc while a track is
    // focused, so the legacy Stop binding never sees it); the Playback gate
    // itself is a plain gated Stop with no track-mode special case left.
    let r = resolve_key(
        InputContext::Playback,
        &snap(true, false),
        KeyChord::new(KeyCode::Esc, KeyModifiers::NONE),
    );
    assert_eq!(r, KeyResolution::Command(Command::Stop));
}

#[test]
fn playback_context_esc_falls_through_when_playback_inactive() {
    let r = resolve_key(
        InputContext::Playback,
        &snap(false, false),
        KeyChord::new(KeyCode::Esc, KeyModifiers::NONE),
    );
    assert_eq!(r, KeyResolution::FallThrough);
}
