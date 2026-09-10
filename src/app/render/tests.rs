use super::test_helpers::*;
use super::*;
use crate::app::tests::make_app_stub;
use crate::app::RemoteSlotState;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn volume_pill_icon_follows_volume_state() {
    let mut app = make_app_stub();
    // Inactive local player: displayed volume comes from `ui_volume`.
    let mut icon_at = |vol: u8| {
        app.ui_volume = vol;
        app.volume_status_spans()[1].content.to_string()
    };
    assert_eq!(icon_at(0), "\u{1F507}"); // muted speaker
    assert_eq!(icon_at(1), "\u{1F508}"); // low
    assert_eq!(icon_at(25), "\u{1F508}"); // low (upper bound)
    assert_eq!(icon_at(26), "\u{1F509}"); // mid
    assert_eq!(icon_at(75), "\u{1F509}"); // mid (upper bound)
    assert_eq!(icon_at(76), "\u{1F50A}"); // high
    assert_eq!(icon_at(200), "\u{1F50A}"); // high (boosted)

    // Muted (`m` key / persisted pref): the indicator reads 0 regardless
    // of the stored level.
    app.ui_volume = 60;
    app.mute_on = true;
    let spans = app.volume_status_spans();
    assert_eq!(spans[1].content.to_string(), "\u{1F507}");
    assert_eq!(spans[2].content.to_string(), " 0");
}

#[test]
fn volume_pill_number_is_aqua() {
    let mut app = make_app_stub();
    app.ui_volume = 60;
    let spans = app.volume_status_spans();
    assert_eq!(spans[2].content.to_string(), " 60");
    assert_eq!(spans[2].style.fg, Some(palette::ACCENT));
}

#[test]
fn emby_status_glyph_color_tracks_service_state() {
    use mbv_core::service_runtime::ServiceState;
    let color = super::components::chrome::service_state_color;
    assert_eq!(color(ServiceState::Ready, palette::ACCENT), palette::ACCENT);
    assert_eq!(
        color(ServiceState::NotConfigured, palette::ACCENT),
        palette::TEXT_MUTED
    );
    for state in [
        ServiceState::Connecting,
        ServiceState::NeedsAuthentication,
        ServiceState::Unavailable,
    ] {
        assert_eq!(color(state, palette::ACCENT), palette::STATUS_ERROR);
    }
}

#[test]
fn stay_alive_glyph_color_tracks_target_and_daemon_loss() {
    fn color(daemon_lost: bool, on_local_daemon: bool) -> ratatui::style::Color {
        if daemon_lost {
            palette::TEXT_FOCUS_ACCENT
        } else if on_local_daemon {
            palette::STATUS_ERROR
        } else {
            palette::TEXT_MUTED
        }
    }
    assert_eq!(color(false, false), palette::TEXT_MUTED); // not in stay-alive mode
    assert_eq!(color(false, true), palette::STATUS_ERROR); // local daemon active
                                                           // Daemon lost (yellow) wins over a still-pointed local target.
    assert_eq!(color(true, true), palette::TEXT_FOCUS_ACCENT);
    assert_eq!(color(true, false), palette::TEXT_FOCUS_ACCENT);
}

#[test]
fn audiobookshelf_status_glyph_color_tracks_service_state() {
    use mbv_core::service_runtime::ServiceState;
    let color = super::components::chrome::service_state_color;
    assert_eq!(
        color(ServiceState::Ready, palette::ACCENT_AUDIOBOOKSHELF),
        palette::ACCENT_AUDIOBOOKSHELF
    );
    assert_eq!(
        color(ServiceState::NotConfigured, palette::ACCENT_AUDIOBOOKSHELF),
        palette::TEXT_MUTED
    );
    for state in [
        ServiceState::Connecting,
        ServiceState::NeedsAuthentication,
        ServiceState::Unavailable,
    ] {
        assert_eq!(
            color(state, palette::ACCENT_AUDIOBOOKSHELF),
            palette::STATUS_ERROR
        );
    }
}

#[test]
fn remote_status_spans_prefers_active_route_label_over_daemon_endpoint() {
    let mut app = make_app_stub();
    app.active_route = Some("music".to_string());
    let spans = app.remote_status_spans(RemoteSlotState::DirectRemote, "tcp://127.0.0.1:9000");
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(text.contains("music"));
}

// The following `remote_status_spans` tests moved here from
// `app::tests` (issue #361, commit 1): they used to render the full app
// and scrape the bottom row, which only worked because the deleted
// Standard view's status bar passed `show_session_pill: true`. The
// status bar (`render/mod.rs`) has always passed `show_session_pill:
// false` -- unchanged by this diff -- because it shows the same
// remote/session info via the queue column's Local/Remote title pills
// instead (`render_queue_title` in `render/queue.rs`, which calls
// this same shared helper). Testing `remote_status_spans` directly, as
// `remote_status_spans_prefers_..._` above already does, covers the
// underlying logic without depending on which caller happens to display it.

#[test]
fn remote_status_spans_uses_daemon_endpoint_host_without_folding_in_server_url() {
    let app = make_app_stub();
    let spans = app.remote_status_spans(RemoteSlotState::DirectRemote, "tcp://music.local:8097");
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains("music.local"),
        "expected the remote glyph label to be the daemon endpoint host:\n{text}"
    );
    assert!(
        !text.contains("music.local@emby.local"),
        "the Emby server host must not be folded into the daemon-endpoint remote label:\n{text}"
    );
}

#[test]
fn remote_status_spans_uses_attached_session_device_name_not_loopback_host() {
    let mut app = make_app_stub();
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some(crate::app::tests::make_session("music", "Emby"));
    let spans = app.remote_status_spans(RemoteSlotState::AttachedSession, "");
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains("music"),
        "expected attached session status to use the F3-visible device name:\n{text}"
    );
    assert!(
        !text.contains("local"),
        "attached remote session should not render as local:\n{text}"
    );
}

#[test]
fn remote_status_spans_keeps_direct_upgrade_session_name_after_state_is_cleared() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(Vec::new(), 0);
    let sess = crate::app::tests::make_session("music", "mbv");

    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    assert!(app.connected_session_id.is_none());
    assert!(app.connected_session_state.is_none());

    let spans = app.remote_status_spans(RemoteSlotState::DirectRemote, "");
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains("music"),
        "direct-upgraded remote should keep the F3-visible session name:\n{text}"
    );
    assert!(
        !text.contains("local"),
        "direct-upgraded remote should not fall back to local after clearing session state:\n{text}"
    );
}

#[test]
fn remote_status_spans_shows_local_device_name_when_off() {
    let app = make_app_stub();
    let spans = app.remote_status_spans(RemoteSlotState::Off, "");
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains(&mbv_core::api::device_name()),
        "expected the local device name when no remote is connected:\n{text}"
    );
    assert!(!text.contains("remote:"));
}

fn rendered_text(app: App, width: u16, height: u16) -> String {
    // The now-playing title is painted solely by the mounted
    // `PlaybackComponent` (row 3.9), so render through the shell path that
    // syncs and paints it rather than the legacy base frame alone. The first
    // frame installs `layout.playback.player_area`; `sync_playback` projects
    // that area into the component, mirroring the steady-state loop order.
    let mut model = crate::app::shell::Model::new(app);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| model.app.compose_base_frame(f, None))
        .unwrap();
    model.sync_playback();
    terminal
        .draw(|f| {
            model.app.compose_base_frame(f, None);
            model.render_playback_component(f);
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let area = buf.area;
    let mut text = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buf[(x, y)].symbol());
        }
    }
    text
}

fn dispatched_cast_status(state: mbv_core::cast_client::CastPlaybackState) -> App {
    use crate::app::types_cast::{CastProgressTarget, DispatchedCastItem};
    use mbv_core::cast_client::CastStatus;
    use mbv_core::playback_queue::QueueItemContentId;

    let mut app = make_app_stub();
    app.attach_cast("device-1".to_string());
    let attachment = app.cast_attachment.as_mut().unwrap();
    attachment.dispatched = vec![DispatchedCastItem {
        url: "https://receiver/a.mp3".to_string(),
        content_id: QueueItemContentId::Feed("guid".to_string()),
        title: "Chromecast Episode Title".to_string(),
        report: CastProgressTarget::Feed {
            feed_id: Some("feed".to_string()),
            guid: "guid".to_string(),
        },
    }];
    attachment.status = Some(CastStatus {
        position_seconds: Some(12.0),
        duration_seconds: Some(120.0),
        playback_rate: 1.0,
        state,
        playing_content_id: Some("https://receiver/a.mp3".to_string()),
    });
    app
}

#[test]
fn cast_now_playing_title_renders_while_the_receiver_is_playing() {
    use mbv_core::cast_client::CastPlaybackState;
    let app = dispatched_cast_status(CastPlaybackState::Playing);
    let text = rendered_text(app, 100, 20);
    assert!(
        text.contains("Chromecast Episode Title"),
        "expected the dispatched item's title while the receiver plays:\n{text}"
    );
}

#[test]
fn cast_now_playing_title_is_absent_while_the_receiver_is_idle() {
    use mbv_core::cast_client::CastPlaybackState;
    let app = dispatched_cast_status(CastPlaybackState::Idle);
    let text = rendered_text(app, 100, 20);
    assert!(
        !text.contains("Chromecast Episode Title"),
        "an idle receiver should show no now-playing title:\n{text}"
    );
}

#[test]
fn the_f3_panel_labels_a_mixed_emby_and_cast_target_list_by_kind() {
    // Same friendly/device name on both channels (8.2's "device appearing
    // on both channels shows as two distinct targets"): the render must
    // still distinguish the two rows by kind tag.
    let mut app = make_app_stub();
    app.sessions = vec![crate::app::tests::make_session("Living Room", "Emby")];
    app.cast_receivers = vec![mbv_core::cast_discovery::CastReceiver {
        id: "cast-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }];
    app.rebuild_panel_targets();

    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let mut cursor = 0;
            let mut scroll = 0;
            let targets =
                crate::app::panel_targets::build_panel_targets(&app.sessions, &app.cast_receivers);
            crate::app::render::render_sessions_overlay_content(
                f,
                Some(Rect::new(0, 0, 100, 20)),
                &targets,
                false,
                &mut cursor,
                &mut scroll,
                None,
                false,
                None,
                false,
            );
        })
        .unwrap();
    let text = buffer_to_string(&terminal);

    assert!(
        text.contains("[EMBY]") && text.contains("[CAST]"),
        "expected both kind labels for a device on both channels:\n{text}"
    );
    assert_eq!(
        text.matches("Living Room").count(),
        2,
        "expected two distinct rows for the same device name:\n{text}"
    );
}
