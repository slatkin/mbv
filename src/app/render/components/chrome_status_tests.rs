use crate::app::tests::{make_app_stub, make_item, make_remote_app_stub, make_session};
use crate::app::QueueScope;

/// The playback target's host label preserves the attached session's device name.
#[test]
fn attached_session_label_preserves_device_name() {
    let mut app = make_app_stub();
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some(make_session("living-room", "Emby"));

    let (label, _) = app.playback_host_label_and_remote();

    assert_eq!(label, "living-room");
}

/// Local playback (no session, no direct remote) resolves to this
/// machine's device name, verbatim.
#[test]
fn local_playback_resolves_this_machine_device_name() {
    let app = make_app_stub();
    assert_eq!(
        app.playback_host_label_and_remote().0,
        mbv_core::api::device_name()
    );
}

/// A direct-remote connection resolves the direct-remote label.
#[test]
fn direct_remote_resolves_the_direct_label() {
    let mut app = make_remote_app_stub(
        vec![make_item("local", "Movie")],
        vec![make_item("remote", "Movie")],
    );
    app.direct_remote_label = Some("direct-device".into());
    app.queue_scope = QueueScope::Local;

    assert_eq!(app.playback_host_label_and_remote().0, "direct-device");
}
