use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use super::{render_status_bar, StatusBarModel, VisualModeIndicator};
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

#[test]
fn visual_indicator_paints_count_and_retains_clear_region() {
    let model = StatusBarModel {
        visual_mode: Some(VisualModeIndicator { count: 4 }),
        ..StatusBarModel::default()
    };
    let mut terminal = Terminal::new(TestBackend::new(60, 1)).unwrap();
    let mut regions = None;
    terminal
        .draw(|frame| {
            regions = Some(render_status_bar(frame, Rect::new(0, 0, 60, 1), &model));
        })
        .unwrap();
    let regions = regions.expect("status bar rendered");
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(text.contains("-- VISUAL (4) --"));
    assert!(regions.visual_clear.is_some());
}

/// A direct-remote connection resolves the direct label.
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
