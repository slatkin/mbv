//! Central input normalization: the chord shape (`KeyChord`) the Keyboard
//! Router's ordered policy (`key_policy.rs` + `router.rs`) matches against,
//! plus the TuiRealm-to-crossterm key conversion. See
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
    /// redundant on BackTab — BackTab *is* Shift+Tab — so it is normalized
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
    /// (`mbv_core::keybinds::Chord`) into the resolver's normalized shape so
    /// policy matching can compare it against the pressed chord. The registry
    /// carries only Ctrl/Shift/Alt, which map losslessly onto crossterm
    /// modifiers.
    pub(in crate::app) fn from_keybinds_chord(chord: mbv_core::keybinds::Chord) -> Self {
        use mbv_core::keybinds::{Key as RegistryKey, KeyMods};
        let mut mods = KeyModifiers::empty();
        if chord.mods.contains(KeyMods::CTRL) {
            mods.insert(KeyModifiers::CONTROL);
        }
        if chord.mods.contains(KeyMods::SHIFT) {
            mods.insert(KeyModifiers::SHIFT);
        }
        if chord.mods.contains(KeyMods::ALT) {
            mods.insert(KeyModifiers::ALT);
        }
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

/// Convert a TuiRealm `KeyEvent` to a crossterm `KeyEvent` for the
/// central keyboard router.
pub(in crate::app) fn tuirealm_key_to_crossterm(
    key: tuirealm::event::KeyEvent,
) -> crossterm::event::KeyEvent {
    use tuirealm::event::{Key as TuiKey, KeyModifiers as TuiMods};
    let code = match key.code {
        TuiKey::Backspace => crossterm::event::KeyCode::Backspace,
        TuiKey::Char(c) => crossterm::event::KeyCode::Char(c),
        TuiKey::Enter => crossterm::event::KeyCode::Enter,
        TuiKey::Left => crossterm::event::KeyCode::Left,
        TuiKey::Right => crossterm::event::KeyCode::Right,
        TuiKey::Up => crossterm::event::KeyCode::Up,
        TuiKey::Down => crossterm::event::KeyCode::Down,
        TuiKey::Home => crossterm::event::KeyCode::Home,
        TuiKey::End => crossterm::event::KeyCode::End,
        TuiKey::PageUp => crossterm::event::KeyCode::PageUp,
        TuiKey::PageDown => crossterm::event::KeyCode::PageDown,
        TuiKey::Tab => crossterm::event::KeyCode::Tab,
        TuiKey::BackTab => crossterm::event::KeyCode::BackTab,
        TuiKey::Delete => crossterm::event::KeyCode::Delete,
        TuiKey::Insert => crossterm::event::KeyCode::Insert,
        TuiKey::Esc => crossterm::event::KeyCode::Esc,
        TuiKey::Null => crossterm::event::KeyCode::Null,
        TuiKey::Function(n) => crossterm::event::KeyCode::F(n),
        _ => crossterm::event::KeyCode::Null,
    };
    let mut modifiers = crossterm::event::KeyModifiers::empty();
    if key.modifiers.contains(TuiMods::SHIFT) {
        modifiers.insert(crossterm::event::KeyModifiers::SHIFT);
    }
    if key.modifiers.contains(TuiMods::CONTROL) {
        modifiers.insert(crossterm::event::KeyModifiers::CONTROL);
    }
    if key.modifiers.contains(TuiMods::ALT) {
        modifiers.insert(crossterm::event::KeyModifiers::ALT);
    }
    crossterm::event::KeyEvent::new(code, modifiers)
}
