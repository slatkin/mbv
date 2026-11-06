//! The neutral keyboard vocabulary (`extract-client-core` design D4).
//!
//! [`KeyEvent`] is the one key shape every Client translates its raw key
//! input into before routing. It carries no terminal- or toolkit-specific
//! state: the code and the modifiers are all chord resolution reads.

/// A key code, as both Clients can report it. `Null` covers a terminal key
/// the translation edge cannot represent; it matches no chord, exactly as
/// the unmapped codes did before the neutral vocabulary existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Backspace,
    Enter,
    Esc,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Insert,
    /// A function key; `Function(1)` is F1.
    Function(u8),
    /// A character key; `Char(' ')` is Space.
    Char(char),
    Null,
}

/// The modifier keys held during a key press or a pointer press. The set
/// carries only the modifiers the keybind registry can bind (Ctrl, Shift,
/// Alt), so the pressed chord and the configured chord compare losslessly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1);
    pub const CONTROL: Self = Self(2);
    pub const ALT: Self = Self(4);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Drop the modifiers in `other` from this set. The `BackTab` chord
    /// normalization (`KeyChord::new`) uses this: `BackTab` is Shift+Tab, so
    /// the redundant SHIFT flag is removed before matching.
    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// A neutral key event: the key code plus the modifiers held. Clients
/// translate their raw key events into this shape at their own edge; the
/// router resolves chords only from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub code: Key,
    pub modifiers: KeyModifiers,
}

impl KeyEvent {
    #[must_use]
    pub const fn new(code: Key, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }
}

impl From<Key> for KeyEvent {
    fn from(code: Key) -> Self {
        Self::new(code, KeyModifiers::NONE)
    }
}
