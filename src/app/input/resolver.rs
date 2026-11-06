//! Central input normalization: the chord shape (`KeyChord`) the Keyboard
//! Router's ordered policy (`key_policy.rs` + `router.rs`) matches against,
//! built only from the neutral key vocabulary (`extract-client-core` design
//! D4). See `docs/adr/0002-centralized-input-handling.md`.
//!
//! The legacy `CONTEXT_STACK` and its handler functions were removed in the
//! keyboard-endpoint deletion (task 8.1), and the bucketed playback resolver
//! (`InputContext`/`resolve_key`) in the add-configurable-keybinds transport
//! split (task 5.1). All input resolution is done by the central Keyboard
//! Router.

use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

/// A normalized key press: physical key code plus active modifiers. This is
/// the unit the resolver matches bindings against; it is carried in the
/// neutral key vocabulary, so a TUI chord and a GUI chord for the same
/// physical press are one value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct KeyChord {
    pub(in crate::app) code: Key,
    pub(in crate::app) mods: KeyModifiers,
}

impl KeyChord {
    /// Normalizations applied to both the pressed chord (`from_key`) and the
    /// configured chord (`from_keybinds_chord`):
    ///
    /// * Crossterm delivers Shift+Tab as `BackTab` with SHIFT set, while the
    ///   registry stores the `previous_library_tab` default as a bare
    ///   `BackTab` (the legacy literal matched the code with no modifier
    ///   check). SHIFT is redundant on `BackTab` — `BackTab` *is* Shift+Tab —
    ///   so it is dropped here.
    /// * The modifier lowercase rule (the TUI translation edge applies it to
    ///   pressed keys; the keybind registry stores configured chords in
    ///   crossterm-case, `Shift+N` as `Char('N')` + SHIFT): an uppercase
    ///   ASCII character under SHIFT is folded to its lowercase letter with
    ///   SHIFT kept, so the pressed and configured chords compare equal.
    ///   Other non-Char codes keep SHIFT: literal bindings such as the
    ///   queue-column-width entry match on it.
    pub(in crate::app) fn new(code: Key, mods: KeyModifiers) -> Self {
        let mods = if code == Key::BackTab {
            mods.without(KeyModifiers::SHIFT)
        } else {
            mods
        };
        let code = match code {
            Key::Char(c) if mods.contains(KeyModifiers::SHIFT) && c.is_ascii_uppercase() => {
                Key::Char(c.to_ascii_lowercase())
            }
            code => code,
        };
        Self { code, mods }
    }

    pub(in crate::app) fn from_key(key: KeyEvent) -> Self {
        Self::new(key.code, key.modifiers)
    }

    /// Convert a configured chord from the keybind registry
    /// (`mbv_keybinds::Chord`) into the resolver's normalized shape so
    /// policy matching can compare it against the pressed chord. The registry
    /// carries only Ctrl/Shift/Alt, which map losslessly onto the neutral
    /// modifier set.
    pub(in crate::app) fn from_keybinds_chord(chord: mbv_keybinds::Chord) -> Self {
        use mbv_keybinds::Key as RegistryKey;
        let mods = registry_mods_to_modifiers(chord.mods);
        let code = match chord.key {
            RegistryKey::Backspace => Key::Backspace,
            RegistryKey::Enter => Key::Enter,
            RegistryKey::Esc => Key::Esc,
            RegistryKey::Left => Key::Left,
            RegistryKey::Right => Key::Right,
            RegistryKey::Up => Key::Up,
            RegistryKey::Down => Key::Down,
            RegistryKey::Home => Key::Home,
            RegistryKey::End => Key::End,
            RegistryKey::PageUp => Key::PageUp,
            RegistryKey::PageDown => Key::PageDown,
            RegistryKey::Tab => Key::Tab,
            RegistryKey::BackTab => Key::BackTab,
            RegistryKey::Delete => Key::Delete,
            RegistryKey::Insert => Key::Insert,
            RegistryKey::F(n) => Key::Function(n),
            RegistryKey::Char(c) => Key::Char(c),
        };
        Self::new(code, mods)
    }
}

/// Map the registry's Ctrl/Shift/Alt modifier set onto the neutral modifiers.
/// Extracted from `KeyChord::from_keybinds_chord` (issue #803); the mapping
/// is lossless in both directions.
fn registry_mods_to_modifiers(mods: mbv_keybinds::KeyMods) -> KeyModifiers {
    use mbv_keybinds::KeyMods;
    let mut out = KeyModifiers::NONE;
    if mods.contains(KeyMods::CTRL) {
        out = out.union(KeyModifiers::CONTROL);
    }
    if mods.contains(KeyMods::SHIFT) {
        out = out.union(KeyModifiers::SHIFT);
    }
    if mods.contains(KeyMods::ALT) {
        out = out.union(KeyModifiers::ALT);
    }
    out
}
