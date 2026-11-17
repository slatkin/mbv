//! Tests of the input layer (D16): the contracts of `input.test.ts` that the
//! Dioxus path keeps — a keydown forwards the DOM fields to the core and a
//! routed key sends nothing, a pointer press carries its position and button,
//! a wheel delta converts to logical pixels, and the cancel paths (a
//! `pointercancel`, a `pointerleave`, a blur) forward the neutral cancel. The
//! dioxus event data is built from test fixtures over the public traits; no
//! document, window, or wait runs.

use std::rc::Rc;
use std::sync::mpsc::{Receiver, channel};

use dioxus_html::geometry::{ClientPoint, ElementPoint, PagePoint, ScreenPoint, WheelDelta};
use dioxus_html::input_data::{MouseButton, MouseButtonSet};
use dioxus_native::prelude::*;

use super::{
    KeyOutcome, KeyRoute, dom_cancel, dom_pointer, dom_wheel, forward_every_key, on_blur,
    on_keydown, on_pointer, on_pointer_cancel,
};
use crate::bridge::keys::KeyTarget;
use crate::bridge::pointer::DomPointerKind;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use mbv_ui_msg::{Key as NeutralKey, KeyModifiers, PointerKind};

/// A `HasKeyboardData` fixture over the fields `dom_key` reads.
struct TestKey {
    key: Key,
    code: Code,
    modifiers: Modifiers,
}

impl ModifiersInteraction for TestKey {
    fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}

impl HasKeyboardData for TestKey {
    fn key(&self) -> Key {
        self.key.clone()
    }

    fn code(&self) -> Code {
        self.code
    }

    fn location(&self) -> Location {
        Location::Standard
    }

    fn is_auto_repeating(&self) -> bool {
        false
    }

    fn is_composing(&self) -> bool {
        false
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A pointer-event fixture over the traits the input layer reads.
struct TestPointer {
    button: Option<MouseButton>,
    buttons: MouseButtonSet,
    client: ClientPoint,
    modifiers: Modifiers,
}

impl InteractionLocation for TestPointer {
    fn client_coordinates(&self) -> ClientPoint {
        self.client
    }

    fn screen_coordinates(&self) -> ScreenPoint {
        ScreenPoint::new(0.0, 0.0)
    }

    fn page_coordinates(&self) -> PagePoint {
        PagePoint::new(0.0, 0.0)
    }
}

impl InteractionElementOffset for TestPointer {
    fn element_coordinates(&self) -> ElementPoint {
        ElementPoint::new(0.0, 0.0)
    }
}

impl ModifiersInteraction for TestPointer {
    fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}

impl PointerInteraction for TestPointer {
    fn trigger_button(&self) -> Option<MouseButton> {
        self.button
    }

    fn held_buttons(&self) -> MouseButtonSet {
        self.buttons
    }
}

impl HasMouseData for TestPointer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl HasPointerData for TestPointer {
    fn pointer_id(&self) -> i32 {
        0
    }

    fn width(&self) -> f64 {
        0.0
    }

    fn height(&self) -> f64 {
        0.0
    }

    fn pressure(&self) -> f32 {
        0.0
    }

    fn tangential_pressure(&self) -> f32 {
        0.0
    }

    fn tilt_x(&self) -> i32 {
        0
    }

    fn tilt_y(&self) -> i32 {
        0
    }

    fn twist(&self) -> i32 {
        0
    }

    fn pointer_type(&self) -> String {
        "mouse".to_owned()
    }

    fn is_primary(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A `WheelData` fixture: the pointer fields delegate to a [`TestPointer`],
/// the delta is the wheel's own.
struct TestWheel {
    pointer: TestPointer,
    delta: WheelDelta,
}

impl InteractionLocation for TestWheel {
    fn client_coordinates(&self) -> ClientPoint {
        self.pointer.client
    }

    fn screen_coordinates(&self) -> ScreenPoint {
        ScreenPoint::new(0.0, 0.0)
    }

    fn page_coordinates(&self) -> PagePoint {
        PagePoint::new(0.0, 0.0)
    }
}

impl InteractionElementOffset for TestWheel {
    fn element_coordinates(&self) -> ElementPoint {
        ElementPoint::new(0.0, 0.0)
    }
}

impl ModifiersInteraction for TestWheel {
    fn modifiers(&self) -> Modifiers {
        self.pointer.modifiers
    }
}

impl PointerInteraction for TestWheel {
    fn trigger_button(&self) -> Option<MouseButton> {
        None
    }

    fn held_buttons(&self) -> MouseButtonSet {
        MouseButtonSet::empty()
    }
}

impl HasMouseData for TestWheel {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl HasWheelData for TestWheel {
    fn delta(&self) -> WheelDelta {
        self.delta
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The pointer fixture the press and cancel tests share: primary button held,
/// at (10, 20), Control held.
fn press_pointer() -> TestPointer {
    TestPointer {
        button: Some(MouseButton::Primary),
        buttons: MouseButtonSet::only(MouseButton::Primary),
        client: ClientPoint::new(10.0, 20.0),
        modifiers: Modifiers::CONTROL,
    }
}

/// The core link and the receiver the assertions read.
fn link() -> (CoreLink, Receiver<CoreInput>) {
    let (sender, receiver) = channel();
    (CoreLink::new(sender), receiver)
}

/// The one input the core received.
fn one_input(receiver: &Receiver<CoreInput>) -> CoreInput {
    receiver
        .try_recv()
        .expect("the forward reached the core once")
}

/// A keydown event over a fixture.
fn keydown(key: Key, code: Code, modifiers: Modifiers) -> KeyboardEvent {
    Event::new(
        Rc::new(KeyboardData::new(TestKey {
            key,
            code,
            modifiers,
        })),
        false,
    )
}

/// Contract 1 (task 3.3): a keydown the router leaves to the core forwards
/// the DOM key fields' translation — Shift+Alt+N arrives as the neutral `n`
/// with SHIFT and ALT, and no Meta.
#[test]
fn a_keydown_forwards_the_dom_fields_to_the_core() {
    let (link, receiver) = link();

    on_keydown(
        &keydown(
            Key::Character("N".into()),
            Code::KeyN,
            Modifiers::SHIFT | Modifiers::ALT,
        ),
        &forward_every_key,
        &link,
    );

    let CoreInput::Key { key: event, target } = one_input(&receiver) else {
        panic!("the key reached the core as a key")
    };
    assert_eq!(target, KeyTarget::None, "no Queue target was resolved");
    assert_eq!(event.code, NeutralKey::Char('n'));
    assert!(event.modifiers.contains(KeyModifiers::SHIFT));
    assert!(event.modifiers.contains(KeyModifiers::ALT));
}

/// A key the router took — `handled` or `text` — sends nothing to the core.
#[rstest::rstest]
#[case::handled(KeyOutcome::Handled)]
#[case::text(KeyOutcome::Text)]
fn a_key_the_router_took_sends_nothing(#[case] outcome: KeyOutcome) {
    let (link, receiver) = link();
    let route: Box<KeyRoute> = Box::new(move |_key| outcome);

    on_keydown(
        &keydown(Key::Character("n".into()), Code::KeyN, Modifiers::empty()),
        route.as_ref(),
        &link,
    );

    receiver.try_recv().unwrap_err();
}

/// Contract 2 (task 3.3): a pointer press carries its position, button, and
/// held-button bitmask — the DOM fields the core's gesture policy reads.
#[test]
fn a_pointer_press_carries_its_position_button_and_modifiers() {
    let pointer = dom_pointer(DomPointerKind::Press, &press_pointer());

    assert_eq!(pointer.kind, DomPointerKind::Press);
    assert_eq!(pointer.button, 0);
    assert_eq!(pointer.buttons, 1);
    assert_eq!(pointer.x, 10.0);
    assert_eq!(pointer.y, 20.0);
    assert!(pointer.modifiers.ctrl_key);
    assert_eq!(pointer.delta_x, 0.0);
    assert_eq!(pointer.delta_y, 0.0);
}

/// A pointer press forwards to the core as the neutral press.
#[test]
fn a_pointer_press_forwards_to_the_core() {
    let (link, receiver) = link();

    on_pointer(
        DomPointerKind::Press,
        &PointerData::new(press_pointer()),
        &link,
    );

    let CoreInput::Pointer(event) = one_input(&receiver) else {
        panic!("the press reached the core as a pointer")
    };
    assert_eq!(event.kind, PointerKind::Press);
    assert_eq!((event.position.x, event.position.y), (10.0, 20.0));
    assert!(event.modifiers.contains(KeyModifiers::CONTROL));
}

/// Contract 3 (task 3.3): a pixel-mode wheel delta keeps its logical pixels,
/// and the vertical delta's sign picks the neutral scroll direction. Blitz's
/// delta carries winit's sign, so the window flips it to the DOM's (a wheel
/// turned toward the user is a positive `deltaY` and scrolls the list down).
#[test]
fn a_pixel_wheel_delta_keeps_its_logical_pixels() {
    let wheel = WheelData::new(TestWheel {
        pointer: TestPointer {
            button: None,
            buttons: MouseButtonSet::empty(),
            client: ClientPoint::new(4.0, 8.0),
            modifiers: Modifiers::empty(),
        },
        delta: WheelDelta::pixels(0.0, 120.0, 0.0),
    });

    let pointer = dom_wheel(&wheel, 600.0);

    assert_eq!(pointer.kind, DomPointerKind::Wheel);
    assert_eq!(pointer.delta_x, 0.0);
    assert_eq!(
        pointer.delta_y, -120.0,
        "Blitz's winit sign flips to the DOM's"
    );
}

/// Contract 4 (task 3.3): a line-mode wheel delta converts to pixels — three
/// lines at 16 pixels a line.
#[test]
fn a_line_mode_wheel_delta_converts_to_pixels() {
    let wheel = WheelData::new(TestWheel {
        pointer: TestPointer {
            button: None,
            buttons: MouseButtonSet::empty(),
            client: ClientPoint::new(0.0, 0.0),
            modifiers: Modifiers::empty(),
        },
        delta: WheelDelta::lines(0.0, 3.0, 0.0),
    });

    let pointer = dom_wheel(&wheel, 600.0);

    assert_eq!(pointer.delta_y, -48.0);
}

/// Contract 5 (task 3.3): a `pointercancel` forwards the neutral cancel, so
/// the core drops the drag state and sends no move.
#[test]
fn a_pointercancel_forwards_the_neutral_cancel() {
    let (link, receiver) = link();

    on_pointer_cancel(&PointerData::new(press_pointer()), &link);

    let CoreInput::Pointer(event) = one_input(&receiver) else {
        panic!("the cancel reached the core as a pointer")
    };
    assert_eq!(event.kind, PointerKind::Cancel);
    assert_eq!((event.position.x, event.position.y), (10.0, 20.0));
}

/// Contract 6 (task 3.3): a `pointerleave` always forwards the cancel — an
/// unbuttoned leave arms no release, but the pointer is gone; a held button's
/// leave loses a gesture in flight, the same cancel.
#[rstest::rstest]
#[case::unbuttoned(-1, 0)]
#[case::button_held(0, 1)]
fn a_pointer_leave_forwards_the_cancel_too(#[case] button: i16, #[case] buttons: u16) {
    let pointer = TestPointer {
        button: if button < 0 {
            None
        } else {
            Some(MouseButton::Primary)
        },
        buttons: if buttons == 0 {
            MouseButtonSet::empty()
        } else {
            MouseButtonSet::only(MouseButton::Primary)
        },
        client: ClientPoint::new(7.0, 9.0),
        modifiers: Modifiers::empty(),
    };

    let cancelled = dom_cancel(Some(&pointer));

    assert_eq!(cancelled.kind, DomPointerKind::Cancel);
    assert_eq!(cancelled.button, button);
    assert_eq!(cancelled.buttons, buttons);
    assert_eq!((cancelled.x, cancelled.y), (7.0, 9.0));
}

/// Contract 7 (task 3.3): a blur carries no pointer fields, so the neutral
/// cancel's placeholders go to the core.
#[test]
fn a_blur_forwards_the_neutral_cancel() {
    let (link, receiver) = link();

    on_blur(&link);

    let CoreInput::Pointer(event) = one_input(&receiver) else {
        panic!("the cancel reached the core as a pointer")
    };
    assert_eq!(event.kind, PointerKind::Cancel);
    assert_eq!((event.position.x, event.position.y), (0.0, 0.0));
}

/// A coordinate or delta that is not finite, or that lies outside `f32`'s
/// range, reads as the neutral 0.0 — none of it round-trips as NaN or an
/// infinity the neutral position cannot name (review P2-1 of
/// replace-tauri-with-blitz).
#[test]
fn a_non_finite_or_out_of_range_value_reads_as_the_neutral_zero() {
    assert_eq!(super::f32_from_f64(f64::NAN), 0.0);
    assert_eq!(super::f32_from_f64(f64::INFINITY), 0.0);
    assert_eq!(super::f32_from_f64(f64::NEG_INFINITY), 0.0);
    assert_eq!(super::f32_from_f64(1.0e300), 0.0);
    assert_eq!(super::f32_from_f64(-1.0e300), 0.0);
}
