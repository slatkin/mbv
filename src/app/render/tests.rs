use super::test_helpers::*;
use super::*;
use crate::app::layout::LayoutPlayback;
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
    assert_eq!(spans[2].style.fg, Some(palette::AQUA));
}

#[test]
fn emby_status_glyph_color_tracks_service_state() {
    use mbv_core::service_runtime::ServiceState;
    let color = super::chrome::emby_state_color;
    assert_eq!(color(ServiceState::Ready), palette::AQUA);
    assert_eq!(color(ServiceState::NotConfigured), palette::MUTED);
    for state in [
        ServiceState::Connecting,
        ServiceState::NeedsAuthentication,
        ServiceState::Unavailable,
    ] {
        assert_eq!(color(state), palette::RED);
    }
}

#[test]
fn stay_alive_glyph_color_tracks_target_and_daemon_loss() {
    let color = super::chrome::stay_alive_color;
    assert_eq!(color(false, false), palette::MUTED); // not in stay-alive mode
    assert_eq!(color(false, true), palette::RED); // local daemon active
                                                  // Daemon lost (yellow) wins over a still-pointed local target.
    assert_eq!(color(true, true), palette::YELLOW);
    assert_eq!(color(true, false), palette::YELLOW);
}

#[test]
fn audiobookshelf_status_glyph_color_tracks_service_state() {
    use mbv_core::service_runtime::ServiceState;
    let color = super::chrome::audiobookshelf_state_color;
    assert_eq!(color(ServiceState::Ready), palette::AMBER);
    assert_eq!(color(ServiceState::NotConfigured), palette::MUTED);
    for state in [
        ServiceState::Connecting,
        ServiceState::NeedsAuthentication,
        ServiceState::Unavailable,
    ] {
        assert_eq!(color(state), palette::RED);
    }
}

#[test]
fn title_row_next_area_matches_rendered_next_glyph_width_and_position() {
    let mut app = make_app_stub();
    app.use_nerd_fonts = false;
    let next_glyph = ">>";
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 2;
        st.current_idx = 0;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
    }

    let backend = TestBackend::new(60, 1);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        app.render_title_row(
            f,
            Rect::new(0, 0, 60, 1),
            "Title",
            palette::BG_GREEN,
            &mut layout,
            palette::SURFACE_PLAYBACK,
        );
    })
    .unwrap();

    let line = buffer_to_string(&term).lines().next().unwrap().to_string();
    let next_byte = line.find(next_glyph).unwrap();
    let next_x = line[..next_byte].width() as u16;

    assert_eq!(layout.next_area.x, next_x);
    assert_eq!(layout.next_area.width, next_glyph.width() as u16);
}

#[test]
fn title_row_next_area_matches_nerd_font_glyph_width_and_position() {
    let mut app = make_app_stub();
    app.use_nerd_fonts = true;
    let next_glyph = "\u{f051}";
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 2;
        st.current_idx = 0;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
    }

    let backend = TestBackend::new(60, 1);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        app.render_title_row(
            f,
            Rect::new(0, 0, 60, 1),
            "Title",
            palette::BG_GREEN,
            &mut layout,
            palette::SURFACE_PLAYBACK,
        );
    })
    .unwrap();

    let line = buffer_to_string(&term).lines().next().unwrap().to_string();
    let next_byte = line.find(next_glyph).unwrap();
    let next_x = line[..next_byte].width() as u16;

    assert_eq!(layout.next_area.x, next_x);
    assert_eq!(layout.next_area.width, next_glyph.width() as u16);
}

#[test]
fn narrow_queue_only_panel_puts_title_on_bottom_now_playing_row() {
    let mut app = make_app_stub();
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.terminal_width = 120; // >= MINI_VIEW_THRESHOLD, so stored panel_mode applies
    app.use_nerd_fonts = false;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 1;
        st.current_idx = 0;
        st.runtime_ticks = 60 * TICKS_PER_SECOND;
    }

    let backend = TestBackend::new(60, 5);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        app.render_player_panel(
            f,
            Rect::new(0, 0, 60, 5),
            &mut layout,
            4,
            true,
            &Some(("My Title".to_string(), palette::WHITE)),
            palette::SURFACE_CHROME,
        );
    })
    .unwrap();

    let text = buffer_to_string(&term);
    let lines: Vec<&str> = text.lines().collect();
    // Title row (y+1) must NOT contain the title.
    assert!(
        !lines[1].contains("My Title"),
        "title row held title:\n{}",
        lines[1]
    );
    // Bottom row (y+3) must carry the prefixed title.
    assert!(
        lines[3].contains("On Now: My Title"),
        "bottom row:\n{}",
        lines[3]
    );
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
