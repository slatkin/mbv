//! Unarmed policy resolution: precedence, gates, rebinds, and registry parity.
use super::test_support::*;
use super::*;
use mbv_core::keybinds::{Chord, KEYBIND_ACTIONS};

#[test]
fn policy_entries_have_unique_ordered_names() {
    let mut names = KEY_POLICY
        .iter()
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len());
    assert_eq!(names.remove(0), "prefix_arm");
    assert_eq!(names.remove(0), "settings_open");
}

#[test]
fn panel_mode_cycle_falls_through_during_text_entry() {
    let key = crossterm::event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    let mut text_entry = snapshot();
    text_entry.overlays.text_entry_focused = true;
    assert_eq!(
        crate::app::input::router::resolve_router_outcome_with_focused(
            key,
            &text_entry,
            None,
            &keybinds()
        ),
        crate::app::input::router::RouterOutcome::FallThrough
    );

    let normal = snapshot();
    assert_eq!(
        crate::app::input::router::resolve_router_outcome_with_focused(
            key,
            &normal,
            None,
            &keybinds()
        ),
        crate::app::input::router::RouterOutcome::Command(Command::CyclePanelMode)
    );
}

#[test]
fn hide_visual_slot_routes_to_command_without_overlay() {
    let key = crossterm::event::KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
    assert_eq!(
        crate::app::input::router::resolve_router_outcome_with_focused(
            key,
            &snapshot(),
            None,
            &keybinds()
        ),
        crate::app::input::router::RouterOutcome::Command(Command::ToggleVisualSlotHidden)
    );
}

#[test]
fn playback_gate_uses_per_key_resolution_and_idle_feed_path() {
    let mut active = snapshot();
    active.playback.player_active = true;
    assert_eq!(
        resolve_policy(
            chord(KeyCode::Char(' '), KeyModifiers::NONE),
            &active,
            &keybinds()
        )
        .unwrap()
        .name,
        "toggle_play_pause"
    );
    assert_eq!(
        resolve_policy(
            chord(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &active,
            &keybinds()
        )
        .map(|entry| entry.name),
        None
    );

    let mut idle_feed = snapshot();
    idle_feed.playback.idle_feed_link_available = true;
    assert_eq!(
        resolve_policy(
            chord(KeyCode::Char('o'), KeyModifiers::NONE),
            &idle_feed,
            &keybinds()
        )
        .unwrap()
        .name,
        "open_idle_feed_link"
    );

    // A focused text entry (e.g. Inline Search) keeps every playback letter
    // as a typed character rather than routing it to a playback command.
    idle_feed.overlays.text_entry_focused = true;
    assert_eq!(
        resolve_policy(
            chord(KeyCode::Char('o'), KeyModifiers::NONE),
            &idle_feed,
            &keybinds()
        )
        .map(|entry| entry.name),
        None
    );
    let mut typing = snapshot();
    typing.playback.player_active = true;
    typing.overlays.text_entry_focused = true;
    assert_eq!(
        resolve_policy(
            chord(KeyCode::Char(' '), KeyModifiers::NONE),
            &typing,
            &keybinds()
        )
        .map(|entry| entry.name),
        None
    );
}

/// Task 5.1: `gate: Playback` is a routing bucket, not a shared
/// eligibility condition. Each split action resolves from its default
/// chord under exactly the condition its key had before the split —
/// gated keys need an active player or a remote session, ungated keys
/// fire idle, and the idle-feed link keeps its own availability gate.
#[test]
fn rebound_transport_action_fires_on_configured_chord_only() {
    // Ungated rebind: volume_up off the `+`/`=` alias onto `k`.
    let keybinds = rebound("volume_up", "k");
    let new_chord = chord(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(
        resolve_policy(new_chord, &snapshot(), &keybinds)
            .unwrap()
            .name,
        "volume_up"
    );
    assert_eq!(
        command_for_policy(KeyPolicyBinding::VolumeUp, new_chord),
        Some(Command::AdjustVolume(VOLUME_STEP))
    );
    for inert in ['+', '='] {
        assert_eq!(
            resolve_policy(
                chord(KeyCode::Char(inert), KeyModifiers::NONE),
                &snapshot(),
                &keybinds
            )
            .map(|entry| entry.name),
            None,
            "declared default `{inert}` of volume_up is inert after the rebind"
        );
    }

    // Gated rebind: toggle_play_pause off Space onto Ctrl+p; fires only
    // under its own eligibility condition.
    let keybinds = rebound("toggle_play_pause", "Ctrl+p");
    let new_chord = chord(KeyCode::Char('p'), KeyModifiers::CONTROL);
    let mut active = snapshot();
    active.playback.player_active = true;
    assert_eq!(
        resolve_policy(new_chord, &active, &keybinds).unwrap().name,
        "toggle_play_pause"
    );
    assert_eq!(
        command_for_policy(KeyPolicyBinding::TogglePlayPause, new_chord),
        Some(Command::TogglePlayPause)
    );
    assert_eq!(
        resolve_policy(new_chord, &snapshot(), &keybinds).map(|entry| entry.name),
        None,
        "a rebound gated action stays gated"
    );
    assert_eq!(
        resolve_policy(
            chord(KeyCode::Char(' '), KeyModifiers::NONE),
            &active,
            &keybinds
        )
        .map(|entry| entry.name),
        None,
        "the declared default Space is inert after the rebind"
    );
}

/// Task 5.2 (D9): the blocking swallow guard stays outside the registry —
/// not configurable, not listed — and keeps its literal match.
#[test]
fn alt_swallow_stays_outside_the_registry_and_still_swallows() {
    // Not configurable: no declared action to assign a chord to.
    assert!(action_by_id("alt_swallow").is_none());
    // Not listed, structurally for any future blocking-only entry: a
    // blocking policy layer must never name a registry action.
    for entry in KEY_POLICY.iter().filter(|entry| entry.blocking) {
        assert!(
            action_by_id(entry.name).is_none(),
            "blocking entry `{}` must stay outside the registry (D9)",
            entry.name
        );
    }
    // Still swallows: an Alt-modified chord resolves to the guard and the
    // router folds it to Swallow.
    let key = chord(KeyCode::Char('k'), KeyModifiers::ALT);
    assert_eq!(
        resolve_policy(key, &snapshot(), &keybinds()).unwrap().name,
        "alt_swallow"
    );
    assert_eq!(
        crate::app::input::router::resolve_router_outcome_with_focused(
            crossterm::event::KeyEvent::new(KeyCode::Char('k'), KeyModifiers::ALT),
            &snapshot(),
            None,
            &keybinds()
        ),
        crate::app::input::router::RouterOutcome::Swallow
    );
}

/// Task 5.1: the split transport entries are exactly the registry's
/// `Playback`-gated actions, one policy layer each, still named by the
/// action id so chord matching resolves through the registry.
#[test]
fn clear_queue_is_gated_when_context_menu_is_open() {
    let key = chord(KeyCode::Char('c'), KeyModifiers::NONE);
    assert_eq!(
        resolve_policy(key, &snapshot(), &keybinds()).unwrap().name,
        "clear_queue_prompt_c"
    );

    let mut menu = snapshot();
    menu.overlays.focus = OverlayFocus::ContextMenu;
    assert_ne!(
        resolve_policy(key, &menu, &keybinds()).map(|entry| entry.name),
        Some("clear_queue_prompt_c")
    );
}

#[test]
fn exact_chord_narrowing_keeps_accidental_superset_chords_inert() {
    // Deliberate exact-chord semantics (design D3): only the declared
    // chords fire an action. Chords that the old permissive literals
    // happened to catch — Ctrl+C for the clear-queue prompt, the
    // Ctrl+Shift terminal encodings of `/` for search — stay inert.
    assert_eq!(
        resolve_policy(
            chord(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &snapshot(),
            &keybinds()
        )
        .map(|entry| entry.name),
        None,
        "Ctrl+C must not fire clear_queue_prompt_c"
    );
    for code in [KeyCode::Char('/'), KeyCode::Char('_')] {
        assert_eq!(
            resolve_policy(
                chord(code, KeyModifiers::CONTROL | KeyModifiers::SHIFT),
                &snapshot(),
                &keybinds()
            )
            .map(|entry| entry.name),
            None,
            "Ctrl+Shift+`/` (both terminal encodings) must not fire search_open"
        );
    }
}

#[test]
fn defaults_reproduce_todays_resolution() {
    // With the default configuration every declared action matches
    // exactly its declared default chords, under the snapshot condition
    // its key carries (task 5.1: per-action transport eligibility).
    let keybinds = Keybinds::default();
    for action in KEYBIND_ACTIONS {
        for chord_str in action.default_chords {
            let key = KeyChord::from_keybinds_chord(Chord::parse(chord_str).unwrap());
            let mut snap = snapshot();
            match action.id {
                "panel_right" => snap.panel_focus = PanelFocus::Queue,
                "panel_left" => snap.panel_focus = PanelFocus::Library,
                // Gated transport keys: one opener (an active player) is
                // enough to prove the default chord still routes.
                "toggle_play_pause"
                | "stop"
                | "seek_back"
                | "seek_forward"
                | "next_track"
                | "previous_track"
                | "toggle_mute_or_cycle_audio" => {
                    snap.playback.player_active = true;
                }
                "open_idle_feed_link" => snap.playback.idle_feed_link_available = true,
                _ => {}
            }
            assert_eq!(
                resolve_policy(key, &snap, &keybinds).map(|entry| entry.name),
                Some(action.id),
                "default chord `{chord_str}` must resolve `{}` as today",
                action.id
            );
        }
    }
}
