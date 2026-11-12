//! `KeyChord` conversion tests: the capture flow's round trip between the
//! resolver's normalized pressed chord and the keybind registry's configured
//! chord (`add-gui-settings-screen` task 1.4, design D6).

use super::{KeyChord, modifiers_to_registry_mods};
use crate::app::input::resolver::registry_mods_to_modifiers;
use mbv_ui_msg::{Key, KeyModifiers};

/// The capture invariant: a pressed chord converts to a configured chord
/// that converts back to the same pressed chord. `from_keybinds_chord`
/// applies the same `KeyChord::new` normalizations (crossterm-case folding,
/// the redundant SHIFT on `BackTab`), so the round trip is exact for a
/// pressed chord.
#[test]
fn capture_round_trips_a_named_key_with_modifiers() {
    let pressed = KeyChord::new(Key::Left, KeyModifiers::CONTROL.union(KeyModifiers::ALT));
    let configured = pressed
        .to_keybinds_chord()
        .expect("a named key must convert to a configured chord");
    assert_eq!(configured.to_string(), "Ctrl+Alt+Left");
    assert_eq!(
        KeyChord::from_keybinds_chord(configured),
        pressed,
        "the configured chord must resolve back to the pressed chord"
    );
}

#[test]
fn capture_round_trips_shift_n_in_crossterm_case() {
    // Crossterm delivers Shift+N as Char('N') + SHIFT; `KeyChord::new`
    // folds it to the lowercase letter with SHIFT kept, and the configured
    // chord spells the uppercase letter the registry's `Shift+N` default
    // and a hand-written `[keys]` entry carry — the spelling the load rules
    // compare against.
    let pressed = KeyChord::new(Key::Char('N'), KeyModifiers::SHIFT);
    let configured = pressed
        .to_keybinds_chord()
        .expect("a character key must convert to a configured chord");
    assert_eq!(configured.to_string(), "Shift+N");
    assert_eq!(
        configured,
        mbv_keybinds::Chord::parse("Shift+N").expect("a declared default chord must parse"),
        "the captured spelling must equal the declared default's"
    );
    assert_eq!(
        KeyChord::from_keybinds_chord(configured),
        pressed,
        "the configured chord must resolve back to the pressed chord"
    );
}

#[test]
fn capture_round_trips_back_tab_without_the_redundant_shift() {
    // Crossterm delivers Shift+Tab as BackTab + SHIFT; the SHIFT is
    // redundant (`BackTab` *is* Shift+Tab) and `KeyChord::new` drops it, so
    // the stored chord spells the bare `BackTab` the registry declares.
    let pressed = KeyChord::new(Key::BackTab, KeyModifiers::SHIFT);
    let configured = pressed
        .to_keybinds_chord()
        .expect("BackTab must convert to a configured chord");
    assert_eq!(configured.to_string(), "BackTab");
    assert_eq!(
        KeyChord::from_keybinds_chord(configured),
        pressed,
        "the configured chord must resolve back to the pressed chord"
    );
}

#[test]
fn null_key_converts_to_none() {
    let pressed = KeyChord::new(Key::Null, KeyModifiers::NONE);
    assert_eq!(
        pressed.to_keybinds_chord(),
        None,
        "the chord grammar cannot name Key::Null"
    );
}

#[test]
fn modifier_mapping_is_lossless_in_both_directions() {
    let all = KeyModifiers::SHIFT
        .union(KeyModifiers::CONTROL)
        .union(KeyModifiers::ALT);
    let registry = modifiers_to_registry_mods(all);
    assert!(registry.contains(mbv_keybinds::KeyMods::CTRL));
    assert!(registry.contains(mbv_keybinds::KeyMods::SHIFT));
    assert!(registry.contains(mbv_keybinds::KeyMods::ALT));
    assert_eq!(registry_mods_to_modifiers(registry), all);
    assert_eq!(
        modifiers_to_registry_mods(KeyModifiers::NONE),
        mbv_keybinds::KeyMods::NONE
    );
}
