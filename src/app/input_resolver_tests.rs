use super::*;
use crate::app::action::Command;

fn snap(active: bool, remote: bool) -> InputSnapshot {
    InputSnapshot {
        player_active: active,
        has_remote_session: remote,
        track_select_active: false,
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
fn playback_context_esc_stops_when_track_select_inactive() {
    let mut snapshot = snap(true, false);
    snapshot.track_select_active = false;
    let r = resolve_key(
        InputContext::Playback,
        &snapshot,
        KeyChord::new(KeyCode::Esc, KeyModifiers::NONE),
    );
    assert_eq!(r, KeyResolution::Command(Command::Stop));
}

#[test]
fn playback_context_esc_falls_through_when_track_select_active() {
    // Esc must not stop a playing track while inline album
    // track-selection mode is active -- it should fall through so the
    // `album_track_mode` context can treat it as "exit
    // track-selection mode" instead (same as Backspace).
    let mut snapshot = snap(true, false);
    snapshot.track_select_active = true;
    let r = resolve_key(
        InputContext::Playback,
        &snapshot,
        KeyChord::new(KeyCode::Esc, KeyModifiers::NONE),
    );
    assert_eq!(r, KeyResolution::FallThrough);
}
