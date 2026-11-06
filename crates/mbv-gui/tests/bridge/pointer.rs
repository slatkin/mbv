//! Public-API tests of the `pointer` command's DTO wire contract
//! (`mbv_gui::bridge::pointer`): the modifier flags flatten into the same
//! object as the `key` command's, and the fields an older front end could omit
//! read as their defaults.

use mbv_gui::bridge::keys::DomModifiers;
use mbv_gui::bridge::pointer::{DomPointer, DomPointerKind};

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
