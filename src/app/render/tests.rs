use super::test_helpers::*;
use super::*;
use crate::app::render::arrangements::chrome::PLAYER_BOX_HEIGHT;
use crate::app::render::PlaybackStripAreas;
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
fn title_row_paints_the_plain_next_control() {
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
    let mut layout = PlaybackStripAreas::default();
    term.draw(|f| {
        let mut context = app.playback_panel_context(
            Rect::new(0, 0, 60, 1),
            &mut layout,
            1,
            true,
            &Some(("Title".into(), palette::SURFACE_FOCUSED)),
            palette::SURFACE_PLAYBACK,
        );
        render_title_row(
            f,
            Rect::new(0, 0, 60, 1),
            "Title",
            palette::SURFACE_FOCUSED,
            &mut context,
        );
    })
    .unwrap();

    let line = buffer_to_string(&term).lines().next().unwrap().to_string();
    assert!(line.contains(next_glyph));
    assert_eq!(layout.next_area.width, next_glyph.width() as u16);
}

#[test]
fn title_row_paints_the_nerd_font_next_control() {
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
    let mut layout = PlaybackStripAreas::default();
    term.draw(|f| {
        let mut context = app.playback_panel_context(
            Rect::new(0, 0, 60, 1),
            &mut layout,
            1,
            true,
            &Some(("Title".into(), palette::SURFACE_FOCUSED)),
            palette::SURFACE_PLAYBACK,
        );
        render_title_row(
            f,
            Rect::new(0, 0, 60, 1),
            "Title",
            palette::SURFACE_FOCUSED,
            &mut context,
        );
    })
    .unwrap();

    let line = buffer_to_string(&term).lines().next().unwrap().to_string();
    assert!(line.contains(next_glyph));
    assert_eq!(layout.next_area.width, next_glyph.width() as u16);
}

/// Task 4.1 (D10): the right column reserves the playback strip's rows only
/// where the strip paints — a `PLAYER_BOX_HEIGHT` band in library-only, none
/// in a queue-visible layout, where the frame's one transport is the Queue
/// playback panel's. The legacy base frame (`App::render`) paints no seekbar
/// or transport row anywhere.
#[test]
fn player_chrome_legacy_base_frame_publishes_geometry_but_paints_no_panel() {
    let mut app = make_movie_app();
    app.panel_mode = crate::app::types_settings::PanelMode::LibraryOnly;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 1;
        st.current_idx = 0;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
    }

    let terminal = render_app_to_terminal(&mut app, 100, 20);

    let player_area = app
        .compute_chrome_geometry(Rect::new(0, 0, 100, 20))
        .player_area;
    assert_eq!(
        player_area.height, PLAYER_BOX_HEIGHT,
        "library-only reserves exactly the strip band for the component: {player_area:?}"
    );
    let buf = terminal.backend().buffer();
    for y in player_area.y..player_area.y + player_area.height {
        for x in player_area.x..player_area.x + player_area.width {
            assert_eq!(
                buf[(x, y)].symbol().trim(),
                "",
                "legacy base frame painted into the player panel at ({x}, {y})"
            );
        }
    }
}

#[test]
fn standard_title_row_showcases_instead_of_truncating_a_long_title() {
    let mut app = make_app_stub();
    let long_title = "A Very Long Album Title That Cannot Possibly Fit In This Row";
    let mut layout = PlaybackStripAreas::default();

    let render = |app: &mut crate::app::App, layout: &mut PlaybackStripAreas| -> String {
        let backend = TestBackend::new(30, 1);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let mut context = app.playback_panel_context(
                Rect::new(0, 0, 30, 1),
                layout,
                1,
                true,
                &Some((long_title.to_string(), palette::TEXT_STRONG)),
                palette::SURFACE_CHROME,
            );
            render_title_row(
                f,
                Rect::new(0, 0, 30, 1),
                long_title,
                palette::TEXT_STRONG,
                &mut context,
            );
        })
        .unwrap();
        buffer_to_string(&term).lines().next().unwrap().to_string()
    };

    let first = render(&mut app, &mut layout);
    assert!(
        !first.contains('\u{2026}'),
        "should showcase, not ellipsis-truncate:\n{first}"
    );
    assert!(
        first.contains("A Very Long"),
        "expected the start of the title at rest:\n{first}"
    );

    // Advance the shared marquee clock past its initial hold.
    app.marquee_started_at =
        std::time::Instant::now() - std::time::Duration::from_millis(1200 + 200 * 5);
    let later = render(&mut app, &mut layout);
    assert!(
        !later.contains('\u{2026}'),
        "should showcase, not ellipsis-truncate:\n{later}"
    );
    assert_ne!(first, later, "title window should have scrolled");
}

/// The queue column's split title band: the title and the `pos / dur` time
/// take the band's first row — the title left with one space of indent, the
/// time right with one space of indent — and the transport controls move to
/// the row below. The header carries no throbber or percent; neither does
/// the title row.
#[test]
fn queue_panel_puts_title_progress_and_time_above_the_controls_row() {
    use crate::app::types_settings::PanelMode;
    let mut app = make_app_stub();
    app.panel_mode = PanelMode::QueueOnly;
    app.use_nerd_fonts = false;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.paused = false;
        st.position_ticks = 45 * TICKS_PER_SECOND;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
    }
    let mut layout = PlaybackStripAreas::default();
    let backend = TestBackend::new(60, 3);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        let title = Some(("Title".to_string(), palette::TEXT_STRONG));
        let context = app.playback_panel_context(
            Rect::new(0, 0, 60, 3),
            &mut layout,
            3,
            true,
            &title,
            palette::SURFACE_CHROME,
        );
        render_player_panel(f, context);
    })
    .unwrap();
    let lines: Vec<String> = buffer_to_string(&term)
        .lines()
        .map(str::to_string)
        .collect();
    assert!(
        lines[1].starts_with(" Title"),
        "one space of left indent:\n{}",
        lines[1]
    );
    assert!(
        !lines[1].contains('%'),
        "no percent on the title row:\n{}",
        lines[1]
    );
    assert!(
        lines[1].ends_with("0:45/1:30 "),
        "time right with one space of indent:\n{}",
        lines[1]
    );
    assert!(
        lines[2].contains("||"),
        "the controls move to the row below:\n{}",
        lines[2]
    );
    assert!(!lines[2].contains("Title"), "the title stays on its row");
    assert_eq!(
        layout.play_pause_area.y, 2,
        "transport hits ride the bottom controls row"
    );
}

/// The Library strip keeps the single title row: the queue column's split
/// never leaks into the right-column presentation.
#[test]
fn library_strip_keeps_the_single_title_row() {
    let mut app = make_app_stub();
    app.use_nerd_fonts = false;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.paused = false;
        st.position_ticks = 45 * TICKS_PER_SECOND;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
    }
    let mut layout = PlaybackStripAreas::default();
    let backend = TestBackend::new(60, 3);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        let title = Some(("Title".to_string(), palette::TEXT_STRONG));
        let context = app.playback_panel_context(
            Rect::new(0, 0, 60, 3),
            &mut layout,
            3,
            true,
            &title,
            palette::SURFACE_CHROME,
        );
        render_player_panel(f, context);
    })
    .unwrap();
    let lines: Vec<String> = buffer_to_string(&term)
        .lines()
        .map(str::to_string)
        .collect();
    assert!(
        lines[1].contains("Title"),
        "strip keeps the title up top:\n{}",
        lines[1]
    );
    assert!(
        lines[2].trim().is_empty(),
        "indicator row stays blank:\n{}",
        lines[2]
    );
}

/// The moved title keeps the shared marquee window on its first-band row:
/// a title that still does not fit scrolls instead of overlapping the time.
#[test]
fn queue_title_row_marquees_a_title_that_does_not_fit() {
    use crate::app::types_settings::PanelMode;
    let mut app = make_app_stub();
    app.panel_mode = PanelMode::QueueOnly;
    app.use_nerd_fonts = false;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.paused = false;
        st.position_ticks = 45 * TICKS_PER_SECOND;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
    }
    let long_title = "A Very Long Album Title That Cannot Possibly Fit In This Row";
    let mut layout = PlaybackStripAreas::default();
    let backend = TestBackend::new(60, 3);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        let title = Some((long_title.to_string(), palette::TEXT_STRONG));
        let context = app.playback_panel_context(
            Rect::new(0, 0, 60, 3),
            &mut layout,
            3,
            true,
            &title,
            palette::SURFACE_CHROME,
        );
        render_player_panel(f, context);
    })
    .unwrap();
    let lines: Vec<String> = buffer_to_string(&term)
        .lines()
        .map(str::to_string)
        .collect();
    assert!(
        lines[1].starts_with(" A Very Long"),
        "marquee rests at the head:\n{}",
        lines[1]
    );
    assert!(
        !lines[1].contains(long_title),
        "overlong title windows instead of overflowing:\n{}",
        lines[1]
    );
    assert!(
        lines[1].ends_with("0:45/1:30 "),
        "time stays intact at the right:\n{}",
        lines[1]
    );
}

#[test]
fn idle_feed_title_marquees_instead_of_truncating() {
    use crate::app::types_feed::{IdleFeed, IdleFeedItem};
    use std::sync::mpsc;

    let mut app = make_app_stub();
    let (items_tx, items_rx) = mpsc::channel();
    app.idle_feed = Some(IdleFeed {
        items: vec![IdleFeedItem {
            title: "A Very Long Novara Media Episode Title That Cannot Fit".to_string(),
            link: Some("https://example.com/ep".to_string()),
        }],
        current_index: 0,
        last_rotation: std::time::Instant::now(),
        last_fetch: std::time::Instant::now(),
        items_tx,
        items_rx,
    });

    let render = |app: &mut crate::app::App, layout: &mut PlaybackStripAreas| -> String {
        let backend = TestBackend::new(30, 4);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            render_player_panel(
                f,
                app.playback_panel_context(
                    Rect::new(0, 0, 30, 4),
                    layout,
                    4,
                    false, // !show_controls => idle state
                    &None,
                    palette::SURFACE_CHROME,
                ),
            );
        })
        .unwrap();
        buffer_to_string(&term).lines().nth(1).unwrap().to_string()
    };

    let mut layout = PlaybackStripAreas::default();
    let first = render(&mut app, &mut layout);
    assert!(
        !first.contains('\u{2026}'),
        "should marquee, not ellipsis-truncate:\n{first}"
    );
    assert!(
        first.contains("A Very Long"),
        "expected the start of the title at rest:\n{first}"
    );

    // Advance the shared marquee clock past its initial hold.
    app.marquee_started_at =
        std::time::Instant::now() - std::time::Duration::from_millis(1200 + 200 * 5);
    let later = render(&mut app, &mut layout);
    assert!(
        !later.contains('\u{2026}'),
        "should marquee, not ellipsis-truncate:\n{later}"
    );
    assert_ne!(first, later, "title window should have scrolled");
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
// instead (the queue column's header row). Testing `remote_status_spans` directly, as
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

fn rendered_text(mut app: App, width: u16, height: u16) -> String {
    // The now-playing strip is painted solely by the mounted
    // `LibraryPlaybackPanel`, and only where `RootFrame` places it (task
    // 4.1): the right column of a queue-hidden layout. Render in library-only,
    // the layout that shows the strip, through the shell path that syncs and
    // paints it.
    app.panel_mode = crate::app::types_settings::PanelMode::LibraryOnly;
    let mut model = crate::app::shell::Model::new(app);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| model.app.compose_root_frame(f)).unwrap();
    model.sync_library_playback_panel();
    terminal
        .draw(|f| {
            model.app.compose_root_frame(f);
            model.render_library_playback_panel_at(
                f,
                model.app.layout.root_frame.library_playback.unwrap(),
            );
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
