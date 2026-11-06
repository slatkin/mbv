//! Public-API tests of the `pointer` command's bridge
//! (`mbv_gui::bridge::pointer`): the DTO wire contract (the modifier flags
//! flatten into the same object as the `key` command's, and the fields an
//! older front end could omit read as their defaults), and the
//! command-to-core translation the command pushes after (design D4).

use mbv_gui::bridge::keys::DomModifiers;
use mbv_gui::bridge::pointer::{DomPointer, DomPointerKind, neutral_pointer};
use mbv_ui_msg::{KeyModifiers, PointerEvent, PointerKind, Position};
use rstest::rstest;

/// The DOM pointer payload for `kind` with `button` as the changed button,
/// `buttons` as the held set, and `delta_y` as the wheel delta: the event
/// happens at (10, 20) with no modifier held, as a plain event carries.
fn dom(kind: DomPointerKind, button: i16, buttons: u16, delta_y: f32) -> DomPointer {
    DomPointer {
        kind,
        button,
        buttons,
        x: 10.0,
        y: 20.0,
        meta_key: false,
        modifiers: DomModifiers::default(),
        delta_x: 0.0,
        delta_y,
    }
}

/// The neutral event at (10, 20) for `kind` with no modifier held.
fn neutral(kind: PointerKind) -> PointerEvent {
    PointerEvent::new(kind, Position { x: 10.0, y: 20.0 }, KeyModifiers::NONE)
}

/// The pointer payload for a primary press with Ctrl held (design D4): the
/// modifier flags sit at the top level of the same object, as the `key`
/// command's payload does.
#[test]
fn pointer_payload_reads_the_top_level_modifier_flags() {
    let event: DomPointer = serde_json::from_str(
        r#"{"kind":"press","button":0,"buttons":1,"x":10.5,"y":20.0,
            "metaKey":false,"ctrlKey":true,"shiftKey":false,"altKey":false}"#,
    )
    .unwrap();
    assert_eq!(event.kind, DomPointerKind::Press);
    assert_eq!(
        event.modifiers,
        DomModifiers {
            shift_key: false,
            ctrl_key: true,
            alt_key: false
        }
    );
    assert!(!event.meta_key);
}

/// A payload from before the `buttons` and `metaKey` fields existed leaves
/// them at their defaults: no held buttons, no meta.
#[test]
fn pointer_payload_without_the_newer_fields_reads_their_defaults() {
    let event: DomPointer = serde_json::from_str(
        r#"{"kind":"move","button":-1,"x":1.0,"y":2.0,
            "ctrlKey":false,"shiftKey":false,"altKey":false}"#,
    )
    .unwrap();
    assert_eq!(event.buttons, 0);
    assert!(!event.meta_key);
}

/// Each DOM pointer event translates to the neutral `PointerEvent` the TUI's
/// terminal edge gives the same input (design D4, the command-to-core
/// boundary): the button kinds, the drag a primary-held move synthesizes,
/// the wheel delta's direction, and the drops the neutral vocabulary cannot
/// represent.
#[rstest]
#[case::primary_press(
    dom(DomPointerKind::Press, 0, 1, 0.0),
    Some(neutral(PointerKind::Press))
)]
#[case::secondary_press_is_the_right_press(
    dom(DomPointerKind::Press, 2, 2, 0.0),
    Some(neutral(PointerKind::RightPress))
)]
#[case::middle_press_drops(dom(DomPointerKind::Press, 1, 4, 0.0), None)]
#[case::primary_release(
    dom(DomPointerKind::Release, 0, 0, 0.0),
    Some(neutral(PointerKind::Release))
)]
#[case::secondary_release_drops(dom(DomPointerKind::Release, 2, 0, 0.0), None)]
#[case::plain_move(
    dom(DomPointerKind::Move, -1, 0, 0.0),
    Some(neutral(PointerKind::Move))
)]
#[case::move_with_the_primary_held_is_the_drag(
    dom(DomPointerKind::Move, -1, 1, 0.0),
    Some(neutral(PointerKind::Drag))
)]
#[case::move_with_another_button_held_reads_as_a_plain_move(
    dom(DomPointerKind::Move, -1, 2, 0.0),
    Some(neutral(PointerKind::Move))
)]
#[case::wheel_downwards(
    dom(DomPointerKind::Wheel, -1, 0, 120.0),
    Some(neutral(PointerKind::ScrollDown))
)]
#[case::wheel_upwards(
    dom(DomPointerKind::Wheel, -1, 0, -120.0),
    Some(neutral(PointerKind::ScrollUp))
)]
#[case::horizontal_only_wheel_drops(
    DomPointer {
        delta_x: 60.0,
        ..dom(DomPointerKind::Wheel, -1, 0, 0.0)
    },
    None
)]
fn dom_pointer_translates_to_the_neutral_pointer(
    #[case] dom_event: DomPointer,
    #[case] expected: Option<PointerEvent>,
) {
    assert_eq!(neutral_pointer(&dom_event), expected);
}

/// A Ctrl-held press keeps CONTROL: the click's selection choice reads the
/// neutral modifiers the same way both Clients report it (design D4).
#[test]
fn a_modifier_held_press_keeps_its_neutral_modifier() {
    let event = DomPointer {
        modifiers: DomModifiers {
            ctrl_key: true,
            ..DomModifiers::default()
        },
        ..dom(DomPointerKind::Press, 0, 1, 0.0)
    };
    assert_eq!(
        neutral_pointer(&event),
        Some(PointerEvent::new(
            PointerKind::Press,
            Position { x: 10.0, y: 20.0 },
            KeyModifiers::CONTROL
        ))
    );
}

/// A Meta-held press loses the flag: the neutral modifiers have no bit for
/// it, the same fact the `key` translation answers by nulling the key.
#[test]
fn a_meta_held_press_loses_the_meta_flag() {
    let event = DomPointer {
        meta_key: true,
        ..dom(DomPointerKind::Press, 0, 1, 0.0)
    };
    assert_eq!(neutral_pointer(&event), Some(neutral(PointerKind::Press)));
}
