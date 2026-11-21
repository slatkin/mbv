//! The input layer (D8 of `replace-tauri-with-blitz`): the port of
//! `lib/bridge/input.ts`. Forwards Dioxus events to the core thread's
//! commands and interprets nothing. Every keydown first meets the key router
//! the window content installs — the Library panel's steps, which run before
//! the forward and take the keys they handle — and then, unless the router
//! took the key, goes to the core with its default action prevented. A router
//! lets an open text field take the key as [`KeyOutcome::Text`]: the core
//! receives no key and the field's own default types it. Pointer press,
//! release, move, and wheel events go to the core as [`DomPointer`]; an event
//! the neutral vocabulary cannot represent drops in the translation, as the
//! TUI's terminal edge drops it.

use dioxus_html::geometry::WheelDelta;
use dioxus_native::prelude::*;

use crate::bridge::keys::{DomKey, DomModifiers, KeyTarget, neutral_key};
use crate::bridge::pointer::{DomPointer, DomPointerKind, neutral_pointer};
use crate::window::channel::CoreLink;

/// What the key router did with one keydown (D8): `Core` forwards the key to
/// the core's keyboard router with the panel its pointer named; `Handled`
/// means the window content took it, so the core receives no key and no
/// default runs; `Text` means an open text field takes it, so the core
/// receives no key and the field's own default types it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    /// The core's keyboard router answers the key, for the panel the pointer
    /// resolved (design D2 of `gui-pointer-target-interaction`).
    Core(KeyTarget),
    /// The window content took the key.
    Handled,
    /// An open text field takes the key.
    Text,
}

/// The handler the forwarder runs before it sends a keydown (D8). The window
/// content installs one; its answer decides whether the core ever sees the
/// key. The route runs on the window's main thread only, and the Dioxus
/// signals the Library steps read are single-threaded storage (D15), so the
/// route carries no thread bound.
pub type KeyRoute = dyn Fn(&DomKey) -> KeyOutcome;

/// The router that takes nothing: every key goes to the core, with no Queue
/// target.
#[must_use]
pub fn forward_every_key(_key: &DomKey) -> KeyOutcome {
    KeyOutcome::Core(KeyTarget::None)
}

/// The DOM `KeyboardEvent` fields the key path carries (D8): the same fields
/// the `key` command carried from the window, now built from the Dioxus
/// event data in process. [`DomKey`] is the existing struct in
/// `bridge/keys.rs`, which stays the translation's input.
#[must_use]
pub fn dom_key(event: &KeyboardData) -> DomKey {
    let modifiers = event.modifiers();
    DomKey {
        key: event.key().to_string(),
        code: event.code().to_string(),
        meta_key: modifiers.contains(Modifiers::META),
        repeat: event.is_auto_repeating(),
        modifiers: DomModifiers {
            shift_key: modifiers.contains(Modifiers::SHIFT),
            ctrl_key: modifiers.contains(Modifiers::CONTROL),
            alt_key: modifiers.contains(Modifiers::ALT),
        },
    }
}

/// The modifier flags and position every pointer kind carries, from the
/// Dioxus event's client coordinates, buttons, and modifiers (D8).
fn pointer_fields(
    kind: DomPointerKind,
    event: &(impl PointerInteraction + InteractionLocation),
) -> DomPointer {
    let coordinates = event.client_coordinates();
    let modifiers = event.modifiers();
    DomPointer {
        kind,
        button: event
            .trigger_button()
            .map_or(-1, dioxus_html::input_data::MouseButton::into_web_code),
        buttons: dioxus_html::input_data::encode_mouse_button_set(event.held_buttons()),
        x: f32_from_f64(coordinates.x),
        y: f32_from_f64(coordinates.y),
        meta_key: modifiers.contains(Modifiers::META),
        modifiers: DomModifiers {
            shift_key: modifiers.contains(Modifiers::SHIFT),
            ctrl_key: modifiers.contains(Modifiers::CONTROL),
            alt_key: modifiers.contains(Modifiers::ALT),
        },
        delta_x: 0.0,
        delta_y: 0.0,
    }
}

/// A `pointerdown`, `pointerup`, or `pointermove` as a [`DomPointer`].
#[must_use]
pub fn dom_pointer(
    kind: DomPointerKind,
    event: &(impl PointerInteraction + InteractionLocation),
) -> DomPointer {
    pointer_fields(kind, event)
}

/// The pixels one DOM `deltaMode` line stands for. The core reads only the
/// delta's sign (`bridge/pointer.rs`: one wheel event is one uniform step),
/// so this conversion exists only to honour the DTO's logical-pixel unit, not
/// to size a scroll.
const PIXELS_PER_LINE: f64 = 16.0;

/// A `wheel` event as a [`DomPointer`], with Blitz's delta converted to the
/// DOM's sign and to logical pixels (D4: `DomPointer` carries logical pixels;
/// the window converts lines and pages before it sends). The conversion is
/// unit and sign only: the core decides what a wheel step means, so no notch or gesture policy
/// lives here. `viewport_height` is the window's inner height — the page unit
/// stands for it, as `window.innerHeight` did.
#[must_use]
pub fn dom_wheel(event: &WheelData, viewport_height: f64) -> DomPointer {
    let mut pointer = pointer_fields(DomPointerKind::Wheel, event);
    // deltaMode: 0 pixels, 1 lines, 2 pages (DOM_DELTA_*). A page stands for
    // the window's height.
    let (unit, delta_x, delta_y) = match event.delta() {
        WheelDelta::Pixels(delta) => (1.0, delta.x, delta.y),
        WheelDelta::Lines(delta) => (PIXELS_PER_LINE, delta.x, delta.y),
        WheelDelta::Pages(delta) => (viewport_height, delta.x, delta.y),
    };
    // Blitz hands the window winit's wheel delta unchanged: positive `y`
    // moves the content down (the wheel turned away from the user), where a
    // DOM `deltaY` is positive for the wheel turned toward the user. The
    // core reads the DOM sign (`bridge/pointer.rs`: a positive delta scrolls
    // down), so both axes flip here, and a wheel turned toward the user
    // moves the list's rows up and the scroll bar's thumb down.
    pointer.delta_x = f32_from_f64(-delta_x * unit);
    pointer.delta_y = f32_from_f64(-delta_y * unit);
    pointer
}

/// A cancelled gesture as a [`DomPointer`]: a `pointercancel` or a
/// `pointerleave` (with or without a button held). The window lost the
/// pointer, so a release may never arrive: the core drops the drag state and
/// the double-click pair, clears its hover state, and sends no move. An
/// element's blur is not one: Blitz blurs the root on every click that moves
/// the focus, so no blur forwards a cancel.
#[must_use]
pub fn dom_cancel(event: &(impl PointerInteraction + InteractionLocation)) -> DomPointer {
    let mut pointer = pointer_fields(DomPointerKind::Cancel, event);
    pointer.delta_x = 0.0;
    pointer.delta_y = 0.0;
    pointer
}

/// The keydown forward (D8): every keydown meets `route` first, and only a
/// key the router leaves to the core goes to the core with its default action
/// prevented — no default such as Tab's focus move runs. The window content's
/// own key handlers call `stop_propagation` before the root sees them, as the
/// the former client handlers did.
pub fn on_keydown(event: &KeyboardEvent, route: &KeyRoute, link: &CoreLink) {
    let key = dom_key(&event.data);
    match route(&key) {
        KeyOutcome::Core(target) => {
            event.prevent_default();
            link.key(neutral_key(key), target);
        }
        KeyOutcome::Handled => {
            // The window content took the key: no default runs for it either.
            event.prevent_default();
        }
        // `Text`: the field's own default runs, and the core gets no key.
        KeyOutcome::Text => {}
    }
}

/// The pointer forward (D8): press, release, and move go to the core as a
/// [`DomPointer`], and an event the neutral vocabulary cannot represent — the
/// middle button, a secondary-button release, a horizontal-only scroll —
/// drops in the translation.
pub fn on_pointer(kind: DomPointerKind, event: &PointerData, link: &CoreLink) {
    send_pointer(dom_pointer(kind, event), link);
}

/// The wheel forward (D8): the wheel goes to the core with its delta in
/// logical pixels. `viewport_height` is the window's inner height, for the
/// page unit.
pub fn on_wheel(event: &WheelData, viewport_height: f64, link: &CoreLink) {
    send_pointer(dom_wheel(event, viewport_height), link);
}

/// The context-menu default (D8): the window's own menu does not run — the
/// job the earlier client's `preventContextMenu` did. No pointer is forwarded:
/// the right click's `pointerdown` already carried the press to the core,
/// and a second forward would duplicate the neutral right press (the earlier
/// input module has no contextmenu listener for the same reason). The handler takes any
/// event type: Dioxus maps `contextmenu` to the mouse event, and only the
/// default matters.
pub fn on_contextmenu<T: 'static>(event: &Event<T>) {
    event.prevent_default();
}

/// The `pointercancel` forward (D8): the window lost the gesture, so the
/// neutral cancel goes to the core.
pub fn on_pointer_cancel(event: &PointerData, link: &CoreLink) {
    send_pointer(dom_cancel(event), link);
}

/// The `pointerleave` forward (D8): the pointer left the window, so the drag
/// it was carrying (if any) and every hover it armed drop — an unbuttoned
/// leave arms no release either, but the hover would stay set without this.
pub fn on_pointer_leave(event: &PointerData, link: &CoreLink) {
    send_pointer(dom_cancel(event), link);
}

/// Send one [`DomPointer`] to the core when the neutral vocabulary can
/// represent it (D8).
fn send_pointer(pointer: DomPointer, link: &CoreLink) {
    if let Some(event) = neutral_pointer(pointer) {
        link.pointer(event);
    }
}

/// The DOM's `clientX`/`clientY` are `f64` at the Dioxus layer, and the wheel
/// delta is `f64`; the wire `DomPointer` and the neutral `Position` are `f32`
/// — the same rounding the earlier wire decoding did. A coordinate
/// or delta that is not finite, or that lies outside `f32`'s range, reads as
/// the neutral 0.0. The measured rects (D9) and the natural-size caps (D7)
/// narrow their `f64` reads through it too.
pub(crate) fn f32_from_f64(value: f64) -> f32 {
    // The checked cast alone is not enough: num-traits' f64→f32 answer is
    // `Some(value as f32)` for every value, so a non-finite or out-of-range
    // value would round-trip NaN or infinity through. The range bound is the
    // widened `f32::MAX`, exact in f64.
    if value.is_finite() && value.abs() <= f64::from(f32::MAX) {
        num_traits::cast(value).unwrap_or(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests;
