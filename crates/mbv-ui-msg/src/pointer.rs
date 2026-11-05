//! The neutral pointer vocabulary (`add-gui-front-end` design D4).
//!
//! [`PointerEvent`] is the one pointer shape every Client translates its raw
//! pointer input into before the core resolves it against a hit map.
//! Positions are in `f32` logical units: the TUI converts cell coordinates,
//! the GUI uses egui points, and the core never sees either unit.

/// A pointer position in logical units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// What happened with the pointer. The set is the minimum both Clients can
/// report today; the translation tasks widen it only when a surface needs
/// the kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerKind {
    /// The primary button was pressed.
    Press,
    /// The primary button was released.
    Release,
    /// The pointer moved with no button held.
    Move,
    /// The wheel scrolled one notch down.
    ScrollDown,
    /// The wheel scrolled one notch up.
    ScrollUp,
}

/// A neutral pointer event: the kind plus where it happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    pub kind: PointerKind,
    pub position: Position,
}

impl PointerEvent {
    #[must_use]
    pub const fn new(kind: PointerKind, position: Position) -> Self {
        Self { kind, position }
    }
}
