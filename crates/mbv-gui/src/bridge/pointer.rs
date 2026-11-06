//! The DOM pointer event that the front end sends with the `pointer` command
//! (design D4).

use serde::Deserialize;

/// The DOM pointer fields that the front end sends with the `pointer` command
/// (design D4). The field names follow the DOM and arrive in `camelCase`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomPointer {
    /// What happened to the pointer.
    pub kind: DomPointerKind,
    /// The DOM `button`: 0 primary, 1 auxiliary, 2 secondary, 3 back, 4 forward,
    /// and -1 when no button changed.
    pub button: i16,
    /// The pointer position in logical pixels, relative to the window.
    pub x: f32,
    pub y: f32,
    /// The horizontal wheel delta in logical pixels; zero for the other kinds.
    /// The front end converts a DOM delta in lines or pages before it sends it.
    #[serde(default)]
    pub delta_x: f32,
    /// The vertical wheel delta in logical pixels, positive downwards; zero for
    /// the other kinds. The front end converts a DOM delta in lines or pages
    /// before it sends it.
    #[serde(default)]
    pub delta_y: f32,
}

/// The pointer event kinds that the front end reports (design D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DomPointerKind {
    /// A button went down.
    Press,
    /// A button went up.
    Release,
    /// The pointer moved.
    Move,
    /// The wheel scrolled.
    Wheel,
}
