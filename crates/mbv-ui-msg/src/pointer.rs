//! The neutral pointer vocabulary (`extract-client-core` design D4).
//!
//! [`PointerEvent`] is the one pointer shape every Client translates its raw
//! pointer input into before the core resolves it against a hit map.
//! Positions are in `f32` logical units: the TUI converts cell coordinates,
//! a GUI uses its toolkit's logical points, and the core never sees either unit.

use super::key::KeyModifiers;

/// A pointer position in logical units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// What happened with the pointer. The set is the minimum both Clients can
/// report today; the translation tasks widen it only when a surface needs
/// the kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    /// The primary button was pressed.
    Press,
    /// The primary button was released.
    Release,
    /// The pointer moved with no button held.
    Move,
    /// The pointer moved with the primary button held.
    Drag,
    /// The wheel scrolled one notch down.
    ScrollDown,
    /// The wheel scrolled one notch up.
    ScrollUp,
    /// The secondary (right) button was pressed.
    RightPress,
}

/// A neutral pointer event: the kind, where it happened, and the modifiers
/// held. The modifiers carry the Ctrl/Shift choice a click's recognition
/// reads (`extract-client-core` row 4.8), so both Clients report a click the
/// same way and the choice lives once in the Client core.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    pub kind: PointerKind,
    pub position: Position,
    pub modifiers: KeyModifiers,
}

impl PointerEvent {
    #[must_use]
    pub const fn new(kind: PointerKind, position: Position, modifiers: KeyModifiers) -> Self {
        Self {
            kind,
            position,
            modifiers,
        }
    }
}
