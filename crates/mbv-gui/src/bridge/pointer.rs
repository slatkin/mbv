//! The DOM pointer event that the front end sends with the `pointer` command
//! (design D4). The modifier flags flatten into the same object, as the `key`
//! command's event does.

use std::cmp::Ordering;

use crate::bridge::keys::{DomModifiers, neutral_modifiers};
use mbv_ui_msg::{PointerEvent, PointerKind, Position};
use serde::Deserialize;

/// The DOM pointer fields that the front end sends with the `pointer` command
/// (design D4). The field names follow the DOM and arrive in `camelCase`, with
/// the modifier flags flattened into the same object, as
/// [`DomKey`](crate::bridge::keys::DomKey) does.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomPointer {
    /// What happened to the pointer.
    pub kind: DomPointerKind,
    /// The DOM `button`: 0 primary, 1 auxiliary, 2 secondary, 3 back, 4 forward,
    /// and -1 when no button changed.
    pub button: i16,
    /// The DOM `buttons` bitmask: the buttons held at the event. Design D4
    /// fixes the pointer kinds as press, release, move, and wheel, so a drag
    /// arrives as a move with a held button; the Rust edge synthesizes the
    /// neutral drag kind from the held buttons. A front end that omits the
    /// field reads as no held buttons.
    #[serde(default)]
    pub buttons: u16,
    /// The pointer position in logical pixels, relative to the window.
    pub x: f32,
    pub y: f32,
    /// Meta (Cmd or the Windows key) is held. A front end that omits it reads
    /// as not held.
    #[serde(default)]
    pub meta_key: bool,
    /// The modifier flags that the neutral pointer modifiers can carry,
    /// flattened into the same object as in
    /// [`DomKey`](crate::bridge::keys::DomKey).
    #[serde(flatten)]
    pub modifiers: DomModifiers,
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

/// The DOM `button` value of the primary (left) button.
const DOM_PRIMARY_BUTTON: i16 = 0;
/// The DOM `button` value of the secondary (right) button.
const DOM_SECONDARY_BUTTON: i16 = 2;
/// The DOM `buttons` bitmask bit of the primary button, the held button that
/// makes a move the neutral drag.
const DOM_PRIMARY_HELD: u16 = 1;

/// Translates a DOM pointer event into the neutral `PointerEvent` (design D4).
///
/// The mapping follows the TUI's terminal edge
/// (`mbv_components::root::neutral_pointer`), so both Clients report one
/// neutral event for one input: a press names the neutral kind by its
/// `button`, a move with the primary button held is the neutral drag, and the
/// vertical delta's sign picks the wheel direction. An event the neutral
/// vocabulary cannot represent drops, exactly as the TUI's edge drops it:
/// the middle, back, and forward buttons, a secondary-button release, and a
/// horizontal-only scroll. The Meta flag drops with them, since the neutral
/// modifiers have no bit for it — the same fact the `key` translation answers
/// by nulling the key. The sender owns the notch sizing: one translated wheel
/// event is one uniform wheel step, so a continuous-delta sender reports one
/// event per notch it decides on.
#[must_use]
pub fn neutral_pointer(event: &DomPointer) -> Option<PointerEvent> {
    let kind = match event.kind {
        DomPointerKind::Press => match event.button {
            DOM_PRIMARY_BUTTON => PointerKind::Press,
            DOM_SECONDARY_BUTTON => PointerKind::RightPress,
            _ => return None,
        },
        DomPointerKind::Release => match event.button {
            DOM_PRIMARY_BUTTON => PointerKind::Release,
            _ => return None,
        },
        // A drag arrives as a move with the held buttons (design D4): the
        // primary's bit synthesizes the neutral drag kind, and any other held
        // combination reads as a plain move, as the TUI's `Moved` does.
        DomPointerKind::Move if event.buttons & DOM_PRIMARY_HELD != 0 => PointerKind::Drag,
        DomPointerKind::Move => PointerKind::Move,
        DomPointerKind::Wheel => match event.delta_y.partial_cmp(&0.0) {
            Some(Ordering::Less) => PointerKind::ScrollUp,
            Some(Ordering::Greater) => PointerKind::ScrollDown,
            // A zero delta is a horizontal-only scroll.
            _ => return None,
        },
    };
    Some(PointerEvent::new(
        kind,
        Position {
            x: event.x,
            y: event.y,
        },
        neutral_modifiers(&event.modifiers),
    ))
}
