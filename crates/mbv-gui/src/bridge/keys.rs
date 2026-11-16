//! Translates a DOM `KeyboardEvent` into the neutral `KeyEvent` (design D6).
//!
//! The window prevents the default action of every key while the window has
//! focus, then sends the event as a [`DomKey`]. The mapping follows the TUI's
//! crossterm edge (`mbv_components::root::neutral_key`), so both Clients report
//! one chord for one physical press. The window also sends the [`KeyTarget`]
//! that the key's pointer resolved (design D2 of `gui-pointer-target-interaction`),
//! so the core knows which panel's keys act without a stored Panel focus.

use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

/// The panel whose keys a GUI key names (design D2, D3): the Queue list box
/// under the pointer, or no Queue target — the Library column is under the
/// pointer, or nothing is.
///
/// The window resolves it from the pointer target ([`crate::window::pointer_target`])
/// at every point where it sends a key to the core, and the core sets its
/// Panel focus from it per key: `Queue` routes the key with the Queue leaf,
/// and `None` leaves only the leafless global and transport chords.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyTarget {
    /// The Queue list box is under the pointer: the Queue's keys act.
    Queue,
    /// No Queue target: the pointer is over the Library column, over neither
    /// panel, or outside the window.
    None,
}

/// The DOM `KeyboardEvent` fields that the window sends with the `key`
/// command (design D4). The field names follow the DOM and are spelled as the DOM spells them,
/// with the modifier flags flattened into the same object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomKey {
    /// The logical key value, such as `n`, ` `, `ArrowLeft`, or `F5`.
    pub key: String,
    /// The physical key code, such as `KeyN`. The translation reads only `key`.
    pub code: String,
    /// Meta (Cmd or the Windows key) is held.
    pub meta_key: bool,
    /// Auto-repeat. The neutral vocabulary has no repeat flag, so it is not read.
    pub repeat: bool,
    pub modifiers: DomModifiers,
}

/// The neutral modifiers the DOM modifier flags carry: SHIFT, CONTROL, and
/// ALT. Meta is not here because the neutral set has no bit for it; see
/// [`DomKey::meta_key`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DomModifiers {
    pub shift_key: bool,
    pub ctrl_key: bool,
    pub alt_key: bool,
}

/// The neutral modifiers a DOM modifier-flag object carries (design D6):
/// SHIFT, CONTROL, and ALT map onto their neutral bits, and Meta drops
/// because the neutral set has no bit for it.
#[must_use]
pub fn neutral_modifiers(modifiers: &DomModifiers) -> KeyModifiers {
    let mut mapped = KeyModifiers::NONE;
    if modifiers.shift_key {
        mapped = mapped.union(KeyModifiers::SHIFT);
    }
    if modifiers.ctrl_key {
        mapped = mapped.union(KeyModifiers::CONTROL);
    }
    if modifiers.alt_key {
        mapped = mapped.union(KeyModifiers::ALT);
    }
    mapped
}

/// Translates a DOM key press into the neutral `KeyEvent`.
///
/// The event is consumed, as the TUI's crossterm edge consumes its event, and
/// the translation reads only the fields it names — `code` and `repeat` are
/// carried but unread, so the destructure leaves them behind. SHIFT, CONTROL,
/// and ALT map to the neutral modifiers. A press with Meta held becomes
/// `Key::Null`, which matches no chord: the neutral modifiers have no Meta, so
/// a bare-letter binding would otherwise fire for a Cmd chord. A key that the
/// vocabulary cannot name, such as `CapsLock`, also becomes `Key::Null`.
#[must_use]
pub fn neutral_key(event: DomKey) -> KeyEvent {
    let DomKey {
        key,
        meta_key,
        modifiers,
        ..
    } = event;
    let modifiers = neutral_modifiers(&modifiers);
    let code = if meta_key {
        Key::Null
    } else {
        logical_key(&key, modifiers)
    };
    KeyEvent::new(code, modifiers)
}

/// Resolves the logical key that a DOM key value names. A single character is
/// that character; a named key follows the UI Events key-value table.
fn logical_key(name: &str, modifiers: KeyModifiers) -> Key {
    let mut chars = name.chars();
    if let (Some(character), None) = (chars.next(), chars.next()) {
        // Under SHIFT an uppercase letter folds to lowercase, as the TUI does:
        // Shift+N is `n` with SHIFT.
        let character = if modifiers.contains(KeyModifiers::SHIFT) {
            character.to_ascii_lowercase()
        } else {
            character
        };
        return Key::Char(character);
    }
    match name {
        "Backspace" => Key::Backspace,
        "Enter" => Key::Enter,
        "Escape" => Key::Esc,
        "ArrowLeft" => Key::Left,
        "ArrowRight" => Key::Right,
        "ArrowUp" => Key::Up,
        "ArrowDown" => Key::Down,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "Delete" => Key::Delete,
        "Insert" => Key::Insert,
        // The DOM reports Shift+Tab as Tab with SHIFT. The neutral key is
        // `BackTab` with SHIFT kept, as crossterm reports it.
        "Tab" if modifiers.contains(KeyModifiers::SHIFT) => Key::BackTab,
        "Tab" => Key::Tab,
        _ => function_key(name).unwrap_or(Key::Null),
    }
}

/// A DOM function-key value: `F` followed by decimal digits.
fn function_key(name: &str) -> Option<Key> {
    let digits = name.strip_prefix('F')?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().map(Key::Function)
}
