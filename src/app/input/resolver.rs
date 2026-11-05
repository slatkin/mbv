//! Central input normalization: the chord shape (`KeyChord`) the Keyboard
//! Router's ordered policy (`key_policy.rs` + `router.rs`) matches against,
//! plus the neutral-to-crossterm key conversion. See
//! `docs/adr/0002-centralized-input-handling.md`.
//!
//! The legacy `CONTEXT_STACK` and its handler functions were removed in the
//! keyboard-endpoint deletion (task 8.1), and the bucketed playback resolver
//! (`InputContext`/`resolve_key`) in the add-configurable-keybinds transport
//! split (task 5.1). All input resolution is done by the central Keyboard
//! Router.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A normalized key press: physical key code plus active modifiers, with the
/// terminal-specific `kind`/`state` fields of `KeyEvent` dropped. This is the
/// unit the resolver matches bindings against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct KeyChord {
    pub(in crate::app) code: KeyCode,
    pub(in crate::app) mods: KeyModifiers,
}

impl KeyChord {
    /// Crossterm delivers Shift+Tab as `BackTab` with SHIFT set, while the
    /// registry stores the `previous_library_tab` default as a bare `BackTab`
    /// (the legacy literal matched the code with no modifier check). SHIFT is
    /// redundant on `BackTab` — `BackTab` *is* Shift+Tab — so it is normalized
    /// away here for both the pressed chord (`from_key`) and the configured
    /// chord (`from_keybinds_chord`). Other non-Char codes keep SHIFT:
    /// literal bindings such as the queue-column-width entry match on it.
    pub(in crate::app) fn new(code: KeyCode, mods: KeyModifiers) -> Self {
        let mods = if code == KeyCode::BackTab {
            mods - KeyModifiers::SHIFT
        } else {
            mods
        };
        Self { code, mods }
    }

    pub(in crate::app) fn from_key(key: KeyEvent) -> Self {
        Self::new(key.code, key.modifiers)
    }

    /// Convert a configured chord from the keybind registry
    /// (`mbv_keybinds::Chord`) into the resolver's normalized shape so
    /// policy matching can compare it against the pressed chord. The registry
    /// carries only Ctrl/Shift/Alt, which map losslessly onto crossterm
    /// modifiers.
    pub(in crate::app) fn from_keybinds_chord(chord: mbv_keybinds::Chord) -> Self {
        use mbv_keybinds::Key as RegistryKey;
        let mods = registry_mods_to_modifiers(chord.mods);
        let code = match chord.key {
            RegistryKey::Backspace => KeyCode::Backspace,
            RegistryKey::Enter => KeyCode::Enter,
            RegistryKey::Esc => KeyCode::Esc,
            RegistryKey::Left => KeyCode::Left,
            RegistryKey::Right => KeyCode::Right,
            RegistryKey::Up => KeyCode::Up,
            RegistryKey::Down => KeyCode::Down,
            RegistryKey::Home => KeyCode::Home,
            RegistryKey::End => KeyCode::End,
            RegistryKey::PageUp => KeyCode::PageUp,
            RegistryKey::PageDown => KeyCode::PageDown,
            RegistryKey::Tab => KeyCode::Tab,
            RegistryKey::BackTab => KeyCode::BackTab,
            RegistryKey::Delete => KeyCode::Delete,
            RegistryKey::Insert => KeyCode::Insert,
            RegistryKey::F(n) => KeyCode::F(n),
            RegistryKey::Char(c) => KeyCode::Char(c),
        };
        Self::new(code, mods)
    }
}

/// Map the registry's Ctrl/Shift/Alt modifier set onto crossterm modifiers.
/// Extracted from `KeyChord::from_keybinds_chord` (issue #803); the mapping
/// is lossless in both directions.
fn registry_mods_to_modifiers(mods: mbv_keybinds::KeyMods) -> KeyModifiers {
    use mbv_keybinds::KeyMods;
    let mut out = KeyModifiers::empty();
    if mods.contains(KeyMods::CTRL) {
        out.insert(KeyModifiers::CONTROL);
    }
    if mods.contains(KeyMods::SHIFT) {
        out.insert(KeyModifiers::SHIFT);
    }
    if mods.contains(KeyMods::ALT) {
        out.insert(KeyModifiers::ALT);
    }
    out
}

/// Convert a neutral `KeyEvent` (`add-gui-front-end` design D4) to a
/// crossterm `KeyEvent` for the central keyboard router, until the router
/// itself moves onto the neutral type.
pub(in crate::app) fn neutral_key_to_crossterm(
    key: mbv_ui_msg::KeyEvent,
) -> crossterm::event::KeyEvent {
    use mbv_ui_msg::Key as NeutralKey;
    let code = match key.code {
        NeutralKey::Backspace => KeyCode::Backspace,
        NeutralKey::Enter => KeyCode::Enter,
        NeutralKey::Esc => KeyCode::Esc,
        NeutralKey::Left => KeyCode::Left,
        NeutralKey::Right => KeyCode::Right,
        NeutralKey::Up => KeyCode::Up,
        NeutralKey::Down => KeyCode::Down,
        NeutralKey::Home => KeyCode::Home,
        NeutralKey::End => KeyCode::End,
        NeutralKey::PageUp => KeyCode::PageUp,
        NeutralKey::PageDown => KeyCode::PageDown,
        NeutralKey::Tab => KeyCode::Tab,
        NeutralKey::BackTab => KeyCode::BackTab,
        NeutralKey::Delete => KeyCode::Delete,
        NeutralKey::Insert => KeyCode::Insert,
        NeutralKey::Function(n) => KeyCode::F(n),
        NeutralKey::Char(c) => KeyCode::Char(c),
        NeutralKey::Null => KeyCode::Null,
    };
    let mut modifiers = crossterm::event::KeyModifiers::empty();
    if key.modifiers.contains(mbv_ui_msg::KeyModifiers::SHIFT) {
        modifiers.insert(crossterm::event::KeyModifiers::SHIFT);
    }
    if key.modifiers.contains(mbv_ui_msg::KeyModifiers::CONTROL) {
        modifiers.insert(crossterm::event::KeyModifiers::CONTROL);
    }
    if key.modifiers.contains(mbv_ui_msg::KeyModifiers::ALT) {
        modifiers.insert(crossterm::event::KeyModifiers::ALT);
    }
    crossterm::event::KeyEvent::new(code, modifiers)
}

/// Convert a crossterm `KeyEvent` to the neutral `KeyEvent` the observer
/// payload carries. Test-only: production input enters through the `TuiRealm`
/// listener and is translated in `mbv-components` (`UiRoot` observer edge);
/// the routing-matrix and shell tests drive the seam with crossterm keys.
#[cfg(test)]
pub(in crate::app) fn crossterm_key_to_neutral(
    key: crossterm::event::KeyEvent,
) -> mbv_ui_msg::KeyEvent {
    use mbv_ui_msg::Key as NeutralKey;
    const KEY_CODES: &[(KeyCode, NeutralKey)] = &[
        (KeyCode::Backspace, NeutralKey::Backspace),
        (KeyCode::Enter, NeutralKey::Enter),
        (KeyCode::Esc, NeutralKey::Esc),
        (KeyCode::Left, NeutralKey::Left),
        (KeyCode::Right, NeutralKey::Right),
        (KeyCode::Up, NeutralKey::Up),
        (KeyCode::Down, NeutralKey::Down),
        (KeyCode::Home, NeutralKey::Home),
        (KeyCode::End, NeutralKey::End),
        (KeyCode::PageUp, NeutralKey::PageUp),
        (KeyCode::PageDown, NeutralKey::PageDown),
        (KeyCode::Tab, NeutralKey::Tab),
        (KeyCode::BackTab, NeutralKey::BackTab),
        (KeyCode::Delete, NeutralKey::Delete),
        (KeyCode::Insert, NeutralKey::Insert),
        (KeyCode::Null, NeutralKey::Null),
    ];
    let code = match key.code {
        KeyCode::Char(c) => NeutralKey::Char(c),
        KeyCode::F(n) => NeutralKey::Function(n),
        code => KEY_CODES
            .iter()
            .find_map(|(xterm_code, mapped_code)| (*xterm_code == code).then_some(*mapped_code))
            .unwrap_or(NeutralKey::Null),
    };
    let mut modifiers = mbv_ui_msg::KeyModifiers::NONE;
    if key
        .modifiers
        .contains(crossterm::event::KeyModifiers::SHIFT)
    {
        modifiers = modifiers.union(mbv_ui_msg::KeyModifiers::SHIFT);
    }
    if key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        modifiers = modifiers.union(mbv_ui_msg::KeyModifiers::CONTROL);
    }
    if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
        modifiers = modifiers.union(mbv_ui_msg::KeyModifiers::ALT);
    }
    mbv_ui_msg::KeyEvent::new(code, modifiers)
}
