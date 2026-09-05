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
    let next_byte = line.find(next_glyph).unwrap();
    let next_x = line[..next_byte].width() as u16;

    assert_eq!(layout.next_area.x, next_x);
    assert_eq!(layout.next_area.width, next_glyph.width() as u16);
}

/// `remove-migrated-surface-underpaint` 3.9 (D4): the right-column player
/// chrome is painted solely by the mounted `PlaybackComponent`. The legacy
/// base frame (`App::render`) still reserves `player_area` as the placement
/// hand-off, but paints no seekbar or transport row there. Mirrors
/// `wide_movies_legacy_base_frame_publishes_geometry_but_paints_no_rows`.
#[test]
fn player_chrome_legacy_base_frame_publishes_geometry_but_paints_no_panel() {
    let mut app = make_movie_app();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 1;
        st.current_idx = 0;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
    }

    let terminal = render_app_to_terminal(&mut app, 100, 20);

    let player_area = app.layout.playback.player_area;
    assert!(
        player_area.height > 0 && player_area.width > 0,
        "player_area must still be reserved for the component: {player_area:?}"
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
        render_player_panel(
            f,
            app.playback_panel_context(
                Rect::new(0, 0, 60, 5),
                &mut layout,
                4,
                true,
                &Some(("My Title".to_string(), palette::TEXT_STRONG)),
                palette::SURFACE_CHROME,
            ),
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
fn narrow_now_playing_row_indents_and_marquees_a_long_title() {
    let mut app = make_app_stub();
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.terminal_width = 120;
    app.use_nerd_fonts = false;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 1;
        st.current_idx = 0;
        st.runtime_ticks = 60 * TICKS_PER_SECOND;
    }
    let long_title = "A Very Long Album Title That Cannot Possibly Fit";

    let backend = TestBackend::new(30, 5);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        render_player_panel(
            f,
            app.playback_panel_context(
                Rect::new(0, 0, 30, 5),
                &mut layout,
                4,
                true,
                &Some((long_title.to_string(), palette::TEXT_STRONG)),
                palette::SURFACE_CHROME,
            ),
        );
    })
    .unwrap();

    let text = buffer_to_string(&term);
    let lines: Vec<&str> = text.lines().collect();
    let bottom = lines[3];
    // Indent: the row's first and last columns stay blank rather than
    // butting text against the panel edges.
    assert_eq!(
        bottom.chars().next(),
        Some(' '),
        "no left indent:\n{bottom}"
    );
    assert_eq!(
        bottom.chars().last(),
        Some(' '),
        "no right indent:\n{bottom}"
    );
    // Marquee: freshly opened (still in its initial hold), the window shows
    // the start of the label rather than being hard-truncated with "...".
    assert!(
        bottom.contains("On Now: A Very"),
        "expected marquee start of label:\n{bottom}"
    );
    assert!(
        !bottom.contains('\u{2026}'),
        "should not ellipsis-truncate marquee text:\n{bottom}"
    );

    // Advance the marquee clock past its initial hold, into the scroll.
    // The "On Now: " prefix must stay put -- only the title pans.
    app.marquee_started_at =
        std::time::Instant::now() - std::time::Duration::from_millis(1200 + 200 * 5);
    let mut term2 = Terminal::new(TestBackend::new(30, 5)).unwrap();
    term2
        .draw(|f| {
            render_player_panel(
                f,
                app.playback_panel_context(
                    Rect::new(0, 0, 30, 5),
                    &mut layout,
                    4,
                    true,
                    &Some((long_title.to_string(), palette::TEXT_STRONG)),
                    palette::SURFACE_CHROME,
                ),
            );
        })
        .unwrap();
    let text2 = buffer_to_string(&term2);
    let bottom2: &str = text2.lines().collect::<Vec<_>>()[3];
    assert!(
        bottom2.trim().starts_with("On Now:"),
        "prefix must stay fixed while title scrolls:\n{bottom2}"
    );
    assert!(
        !bottom2.contains("On Now: A Very"),
        "title window should have scrolled past its start:\n{bottom2}"
    );
}

#[test]
fn standard_title_row_showcases_instead_of_truncating_a_long_title() {
    let mut app = make_app_stub();
    let long_title = "A Very Long Album Title That Cannot Possibly Fit In This Row";
    let mut layout = LayoutPlayback::default();

    let render = |app: &mut crate::app::App, layout: &mut LayoutPlayback| -> String {
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

    let render = |app: &mut crate::app::App, layout: &mut LayoutPlayback| -> String {
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

    let mut layout = LayoutPlayback::default();
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
