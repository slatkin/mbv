use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::Terminal;

use super::{render_status_bar, StatusBarModel, VisualModeIndicator};
use crate::app::tests::{make_app_stub, make_item, make_remote_app_stub, make_session};
use crate::app::QueueScope;

/// A minimal pill-shaped span set with the same 1-1 shape the App-side
/// builders produce (space, ` label `, space).
fn pill(label: &str) -> Vec<Span<'static>> {
    let fill = Style::default();
    vec![
        Span::styled(" ", fill),
        Span::styled(format!(" {label} "), fill),
        Span::styled(" ", fill),
    ]
}

fn status_text(width: u16, model: &StatusBarModel) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    terminal
        .draw(|frame| {
            render_status_bar(frame, Rect::new(0, 0, width, 1), model);
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol().to_string())
        .collect()
}

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

/// The prefix-armed pill paints only while armed (task 7.3, design D7).
#[test]
fn armed_pill_paints_only_while_armed() {
    let armed = StatusBarModel {
        prefix_armed: Some(pill("PREFIX")),
        ..StatusBarModel::default()
    };
    let text = status_text(60, &armed);
    assert!(text.contains("PREFIX"), "armed: pill painted, got {text:?}");

    let idle = StatusBarModel::default();
    let text = status_text(60, &idle);
    assert!(!text.contains("PREFIX"), "idle: no pill, got {text:?}");
}

/// Width pressure follows the existing drop-order precedent: the armed
/// pill sits at the visual-mode tier, above volume and mute, so mute drops
/// first and the pill survives as long as any left pill is shown (task 7.3).
#[test]
fn armed_pill_outlasts_the_other_pills_under_width_pressure() {
    let model = StatusBarModel {
        mute: Some(pill("muted")),
        volume: pill("75"),
        prefix_armed: Some(pill("PREFIX")),
        ..StatusBarModel::default()
    };
    // Wide enough for armed + volume (10 + 6 + 1 gap = 17) but not for
    // mute too (27): the mute pill drops first, the armed pill and volume
    // remain.
    let text = status_text(17, &model);
    assert!(text.contains("PREFIX"), "armed pill persists: {text:?}");
    assert!(text.contains("75"), "volume outlasts mute: {text:?}");
    assert!(!text.contains("muted"), "mute dropped first: {text:?}");
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
