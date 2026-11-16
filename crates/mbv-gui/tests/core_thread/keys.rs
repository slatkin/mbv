//! The GUI key's target, as the core routes it (`gui-pointer-target-interaction`
//! design D2 and D3): the window's key router names the panel the pointer is
//! over, and the core sets its Panel focus from that target per key instead of
//! keeping one of its own. `ComponentId::Library` stays mounted, so a key with
//! no Queue target meets only the leafless global and transport chords.

use mbv::stub_app_with_playing_queue;
use mbv_gui::bridge::keys::KeyTarget;
use mbv_gui::core_thread::CoreInput;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

use crate::{queue_key, test_core, test_core_with};

/// The next keydown, with no Queue target: the pointer is over the Library
/// column, or over neither panel.
fn unqueued(key: Key) -> CoreInput {
    CoreInput::Key {
        key: KeyEvent::new(key, KeyModifiers::NONE),
        target: KeyTarget::None,
    }
}

/// Contract (design D3): a key with the Queue target routes with the Queue
/// leaf, so a movement moves the Queue's active row.
#[test]
fn a_queue_target_key_moves_the_queue_cursor() {
    let mut test = test_core(3);
    assert!(test.core.step());
    test.held();

    test.tx
        .send(queue_key(KeyEvent::new(Key::Down, KeyModifiers::NONE)))
        .unwrap();
    assert!(test.core.step());

    assert_eq!(test.held().queue().cursor, Some(2));
}

/// Contract (design D3): the next key with no Queue target reaches no Queue
/// leaf, so the Queue's cursor stays where it was, and Space still toggles
/// playback — the leafless global and transport chords are what a `None`
/// target leaves.
#[test]
fn a_key_with_no_queue_target_reaches_no_queue_leaf() {
    let mut test = test_core_with(stub_app_with_playing_queue(3));
    assert!(test.core.step());
    test.held();

    test.tx
        .send(queue_key(KeyEvent::new(Key::Down, KeyModifiers::NONE)))
        .unwrap();
    assert!(test.core.step());
    assert_eq!(test.held().queue().cursor, Some(2));

    test.tx.send(unqueued(Key::Down)).unwrap();
    assert!(test.core.step());
    assert_eq!(
        test.held().queue().cursor,
        Some(2),
        "no Queue leaf took the key"
    );

    test.tx.send(unqueued(Key::Char(' '))).unwrap();
    assert!(test.core.step());
    assert_eq!(
        test.held()
            .status()
            .toast
            .as_ref()
            .map(|toast| toast.text.as_str()),
        Some("Pause requested"),
        "the transport toggle fired with no Queue target"
    );
}
