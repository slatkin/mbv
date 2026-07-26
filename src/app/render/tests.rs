use super::album_plan::GroupedAlbumDisplayRow;
use super::*;
use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, QueueScope, RemoteSlotState};
use crate::config::Config;
use mbv_core::api::EmbyClient;
use mbv_core::api::MediaItem;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
    let buf = term.backend().buffer();
    let area = *buf.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
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
            palette::GREEN,
            &mut layout,
        );
    })
    .unwrap();

    let line = buffer_to_string(&term).lines().next().unwrap().to_string();
    let next_byte = line.find(next_glyph).unwrap();
    let next_x = line[..next_byte].width() as u16;

    assert_eq!(layout.next_area.x, next_x);
    assert_eq!(layout.next_area.width, next_glyph.width() as u16);
    assert!(
        line.starts_with("|| X >> Title"),
        "expected stop then next glyph between play/pause and title:\n{line}"
    );
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
            palette::GREEN,
            &mut layout,
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
fn title_row_truncates_long_title_before_transport_status_and_badges() {
    let mut app = make_app_stub();
    app.use_nerd_fonts = false;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 2;
        st.current_idx = 0;
        st.position_ticks = 65 * TICKS_PER_SECOND;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
        st.video_height = 1080;
        st.audio_lang = "en".into();
    }

    let backend = TestBackend::new(53, 1);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        app.render_title_row(
            f,
            Rect::new(0, 0, 53, 1),
            "This is an extremely long title that would otherwise push controls away",
            palette::GREEN,
            &mut layout,
        );
    })
    .unwrap();

    let line = buffer_to_string(&term).lines().next().unwrap().to_string();

    assert!(
        line.contains('\u{2026}'),
        "expected long title to be truncated with ellipsis:\n{line}"
    );
    assert!(
        line.contains("1:05 / 1:30"),
        "expected time cluster to remain visible:\n{line}"
    );
    assert!(
        line.ends_with("RES 1080p  AUD en  SUB off"),
        "expected status badges to remain right-aligned:\n{line}"
    );
    assert!(layout.next_area.x + layout.next_area.width <= 53);
}

#[test]
fn status_bar_remote_hitbox_tracks_visible_pill_after_alive_marker() {
    let mut app = make_app_stub();
    let (app_end, _relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
    app.stay_alive_ctrl = Some(crate::app::stay_alive::StayAliveCtrl::for_test(app_end));

    let backend = TestBackend::new(80, 1);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        app.render_status_bar(f, Rect::new(0, 0, 80, 1), &mut layout, true, true);
    })
    .unwrap();

    let line = buffer_to_string(&term).lines().next().unwrap().to_string();
    let heart_byte = line.find('\u{2665}').unwrap();
    let remote_byte = line.find('\u{1F5A7}').unwrap();
    let heart_x = line[..heart_byte].width() as u16;
    let remote_x = line[..remote_byte].width() as u16;

    assert!(
        layout.ind_rc.contains((remote_x, 0).into()),
        "expected the remote hitbox to cover the rendered remote pill:\n{line}"
    );
    assert!(
        !layout.ind_rc.contains((heart_x, 0).into()),
        "expected the stay-alive heart to stay outside the sessions hitbox:\n{line}"
    );
}

#[test]
fn status_bar_omits_alive_marker_when_overflow_chooses_without_alive() {
    let mut app = make_app_stub();
    let (app_end, _relay_end) = std::os::unix::net::UnixStream::pair().unwrap();
    app.stay_alive_ctrl = Some(crate::app::stay_alive::StayAliveCtrl::for_test(app_end));

    let remote_status = app.remote_status_spans(RemoteSlotState::Off, "");
    let playlist_status = app.playlist_status_spans();
    let width = App::status_width(&remote_status) + App::status_width(&playlist_status) + 1;

    let backend = TestBackend::new(width, 1);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        app.render_status_bar(f, Rect::new(0, 0, width, 1), &mut layout, true, true);
    })
    .unwrap();

    let line = buffer_to_string(&term).lines().next().unwrap().to_string();
    assert!(
        !line.contains('\u{2665}'),
        "expected overflow to drop the stay-alive marker before rendering:\n{line}"
    );
    assert!(
        line.contains('\u{1F5A7}') && line.contains('\u{1F5AD}'),
        "expected remote and playlist pills to remain visible:\n{line}"
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
// instead (`render_power_queue_title` in `render/queue.rs`, which calls
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

    app.switch_to_direct_remote(&sess, remote, remote_rx);
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

#[test]
fn remote_status_spans_colors_icon_white_and_label_black_when_off_or_aqua_when_remote() {
    let app = make_app_stub();
    let spans = app.remote_status_spans(RemoteSlotState::Off, "");
    assert_eq!(spans[1].style.fg, Some(ratatui::style::Color::White));
    assert_eq!(spans[2].style.fg, Some(ratatui::style::Color::Black));

    let mut app = make_app_stub();
    app.active_route = Some("music".to_string());
    let spans = app.remote_status_spans(RemoteSlotState::DirectRemote, "");
    assert_eq!(spans[1].style.fg, Some(ratatui::style::Color::White));
    assert_eq!(spans[2].style.fg, Some(palette::AQUA));
}

#[test]
fn status_label_style_uppercases_and_bolds_selected_label() {
    let mut spans = vec![
        Span::raw(" "),
        Span::raw("icon"),
        Span::styled("  living-room", Style::default().fg(palette::SUBTLE)),
        Span::raw(" "),
    ];

    App::uppercase_status_label(&mut spans);
    App::set_status_label_bold(&mut spans, true);

    assert_eq!(spans[2].content.as_ref(), "  LIVING-ROOM");
    assert!(spans[2].style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn expired_toast_clears_before_status_bar_render_decides_overlay() {
    let mut app = make_app_stub();
    app.status = "Saved [Y]".to_string();
    app.status_expires = Some(std::time::Instant::now() - std::time::Duration::from_millis(1));

    let backend = TestBackend::new(80, 24);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();

    let last_line = buffer_to_string(&term).lines().last().unwrap().to_string();
    assert!(
        !last_line.contains("Saved"),
        "expected expired toast text to clear before the status bar chooses its row:\n{last_line}"
    );
    assert!(
        last_line.contains('\u{1F5AD}'),
        "expected the persistent status bar to render after an expired toast clears:\n{last_line}"
    );
    assert!(app.status.is_empty());
    assert!(app.status_expires.is_none());
}

fn render_sidebar_scrollbar_column(total: usize, visible: u16, scroll: usize) -> String {
    let backend = TestBackend::new(1, visible);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        App::render_sidebar_scrollbar(f, Rect::new(0, 0, 0, visible), total, scroll);
    })
    .unwrap();
    buffer_to_string(&term)
}

#[test]
fn sidebar_scrollbar_reaches_top_and_bottom_with_paragraph_offsets() {
    let top = render_sidebar_scrollbar_column(10, 5, 0);
    let bottom = render_sidebar_scrollbar_column(10, 5, 5);

    assert!(top.lines().next().is_some_and(|line| line != "│"));
    assert!(bottom.lines().last().is_some_and(|line| line != "│"));
    assert_eq!(
        top.lines().filter(|line| *line != "│").count(),
        bottom.lines().filter(|line| *line != "│").count()
    );
    assert!(top.chars().all(|c| c == '│' || c == '▕' || c == '\n'));
    assert_ne!(top, bottom);
}

#[test]
fn sidebar_scrollbar_uses_scrollbar_color() {
    let backend = TestBackend::new(1, 5);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        App::render_sidebar_scrollbar(f, Rect::new(0, 0, 0, 5), 10, 0);
    })
    .unwrap();

    let buf = term.backend().buffer();
    assert_ne!(buf[(0, 0)].symbol(), "│");
    assert_eq!(buf[(0, 0)].fg, palette::SCROLLBAR);
    assert_eq!(buf[(0, 4)].symbol(), "│");
    assert_eq!(buf[(0, 4)].fg, palette::SCROLLBAR);
}

#[test]
fn power_view_uses_triangle_resampling() {
    assert_eq!(POWER_RENDER_FILTER, ratatui_image::FilterType::Triangle);
}

fn render_power_scrollbar_column(height: u16, max_offset: usize, offset: usize) -> String {
    let backend = TestBackend::new(1, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_power_scrollbar(f, Rect::new(0, 0, 1, height), max_offset, offset);
    })
    .unwrap();
    buffer_to_string(&term)
}

fn render_power_scrollbar_column_with_viewport(
    height: u16,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
) -> String {
    let backend = TestBackend::new(1, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_power_scrollbar_with_viewport(
            f,
            Rect::new(0, 0, 1, height),
            content_length,
            viewport_content_length,
            offset,
        );
    })
    .unwrap();
    buffer_to_string(&term)
}

#[test]
fn power_scrollbar_is_proportional_and_reaches_both_ends() {
    let top = render_power_scrollbar_column(7, 3, 0);
    let bottom = render_power_scrollbar_column(7, 3, 3);

    assert!(top.lines().next().is_some_and(|line| line != " "));
    assert!(bottom.lines().last().is_some_and(|line| line != " "));
    assert!(top.chars().filter(|&c| c == '▕').count() > 2);
    assert!(top.chars().all(|c| c == '▕' || c == ' ' || c == '\n'));
}

#[test]
fn power_scrollbar_respects_custom_viewport_units() {
    let top = render_power_scrollbar_column_with_viewport(7, 10, 2, 0);
    let bottom = render_power_scrollbar_column_with_viewport(7, 10, 2, 8);

    assert!((1..=2).contains(&top.matches('▕').count()));
    assert!((1..=2).contains(&bottom.matches('▕').count()));
    assert!(top.chars().all(|c| c == '▕' || c == ' ' || c == '\n'));
    assert!(bottom.chars().all(|c| c == '▕' || c == ' ' || c == '\n'));
}

#[test]
fn queue_scrollbar_uses_queue_unfocused_color() {
    let backend = TestBackend::new(1, 7);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_power_scrollbar(f, Rect::new(0, 0, 1, 7), 3, 0);
    })
    .unwrap();

    assert_eq!(term.backend().buffer()[(0, 0)].fg, palette::SCROLLBAR);
}

/// Renders a pill bar of the given labels/ids into a `width`-wide row and
/// returns the resulting `(rect, id)` hitboxes.
fn render_pill_bar_hitboxes(
    labels: &[String],
    ids: &[usize],
    selected_pos: usize,
    width: u16,
) -> Vec<(Rect, usize)> {
    let backend = TestBackend::new(width, 1);
    let mut term = Terminal::new(backend).unwrap();
    let mut tabs = Vec::new();
    term.draw(|f| {
        tabs = render_pill_bar(
            f,
            Rect::new(0, 0, width, 1),
            PillBar {
                labels,
                ids,
                selected_pos,
                prefix: None,
                underlay: PillUnderlay::Blank { fill: true },
            },
        );
    })
    .unwrap();
    tabs
}

#[test]
fn pill_bar_hitboxes_carry_caller_ids_not_display_positions() {
    // ids are deliberately offset from positions (mirroring Home's
    // section_idx = position + 10 here) so a regression that returned the
    // display offset instead of the id would be caught.
    let labels: Vec<String> = ["Alpha", "Beta", "Gamma"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let ids = vec![10usize, 11, 12];

    // Wide enough to show every pill: all ids returned, in order.
    let tabs = render_pill_bar_hitboxes(&labels, &ids, 0, 60);
    assert_eq!(
        tabs.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        vec![10, 11, 12],
    );
    // Hitboxes are left-to-right and non-overlapping.
    for pair in tabs.windows(2) {
        assert!(pair[0].0.x + pair[0].0.width <= pair[1].0.x);
    }
}

#[test]
fn pill_bar_scrolls_to_keep_selected_visible_and_maps_its_id() {
    // Six pills in a narrow row force horizontal scrolling; selecting the
    // last one must scroll it into view and report its caller id (25).
    let labels: Vec<String> = (0..6).map(|i| format!("Group{i}")).collect();
    let ids: Vec<usize> = (0..6).map(|i| 20 + i).collect();

    let tabs = render_pill_bar_hitboxes(&labels, &ids, 5, 18);

    assert!(!tabs.is_empty(), "expected at least one visible pill");
    // The selected pill (id 25) must be among the visible hitboxes.
    assert!(
        tabs.iter().any(|(_, id)| *id == 25),
        "selected pill's id should be visible after scrolling, got {:?}",
        tabs.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
    );
    // Every visible id is one we supplied (never a bare display offset).
    assert!(tabs.iter().all(|(_, id)| (20..=25).contains(id)));
    // Overflow occurred, so scrolling dropped at least one leading pill.
    assert!(
        tabs.len() < labels.len(),
        "narrow row should not fit all six pills"
    );
}

fn render_power_library_to_terminal(
    app: &mut App,
    layout: &mut LayoutMain,
) -> Terminal<TestBackend> {
    render_power_library_to_terminal_focused(app, layout, true)
}

fn render_power_library_to_terminal_focused(
    app: &mut App,
    layout: &mut LayoutMain,
    focused: bool,
) -> Terminal<TestBackend> {
    // 20 rows is comfortably enough for the " N items" header row (that
    // `render_power_list` draws unconditionally for a focused library
    // panel) plus the selected row and the compact banner's
    // content-dependent height (#263) for the short test overviews used
    // by callers of this helper.
    let backend = TestBackend::new(60, 20);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_power_library(f, Rect::new(0, 0, 60, 20), focused, layout);
    })
    .unwrap();
    term
}

fn render_power_library_to_string(app: &mut App, layout: &mut LayoutMain) -> String {
    let term = render_power_library_to_terminal(app, layout);
    buffer_to_string(&term)
}

fn render_power_view_to_terminal(
    app: &mut App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, LayoutMain) {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        app.render_main(
            f,
            Rect::new(0, 0, width, height),
            &mut layout,
            &mut LayoutPlayback::default(),
            &mut Rect::default(),
            &mut Rect::default(),
            0,
            false,
            &None,
        );
    })
    .unwrap();
    (term, layout)
}

fn render_power_view(app: &mut App, width: u16, height: u16) -> LayoutMain {
    render_power_view_to_terminal(app, width, height).1
}

#[test]
fn expanded_power_view_tab_panel_has_two_column_side_gutters() {
    let mut app = make_app_stub();
    app.queue_column_width = 40;

    let layout = render_power_view(&mut app, 80, 24);

    assert_eq!(layout.left_area.x, 40 + POWER_TAB_LEFT_PAD);
    assert_eq!(layout.left_area.width, 40 - 2 * POWER_TAB_LEFT_PAD);
}

#[test]
fn expanded_power_panel_bounds_follow_sidebar_resize() {
    let mut app = make_app_stub();
    app.queue_column_width = 31;
    let first = render_power_view(&mut app, 80, 24);
    assert_eq!(first.panel_area, Rect::new(0, 0, 31, 24));
    assert_eq!(first.panel_content_area, Rect::new(2, 3, 27, 19));

    app.queue_column_width = 47;
    let second = render_power_view(&mut app, 80, 24);
    assert_eq!(second.panel_area, Rect::new(0, 0, 47, 24));
    assert_eq!(second.panel_content_area, Rect::new(2, 3, 43, 19));
}

#[test]
fn power_panel_shell_paints_opaque_sidebar_and_active_local_header() {
    let backend = TestBackend::new(20, 8);
    let mut term = Terminal::new(backend).unwrap();
    let sidebar = Rect::new(3, 1, 10, 6);
    term.draw(|f| {
        f.render_widget(
            Block::default().style(Style::default().bg(palette::IRIS)),
            sidebar,
        );
        App::render_panel_shell_at(f, sidebar, "HELP", "Esc Close", true);
    })
    .unwrap();

    let buffer = term.backend().buffer();
    for y in sidebar.y..sidebar.bottom() {
        for x in sidebar.x..sidebar.right() {
            if y >= sidebar.bottom() - 2
                || (y == sidebar.y + 1 && (sidebar.x + 2..sidebar.right()).contains(&x))
            {
                continue;
            }
            assert_eq!(buffer[(x, y)].bg, palette::PLAYBACK_PANEL_BG);
        }
    }
    assert_eq!(
        buffer[(sidebar.x + 2, sidebar.y + 1)].bg,
        palette::QUEUE_BUTTON_FOCUSED_BG
    );
    assert_eq!(buffer[(sidebar.x + 2, sidebar.y + 1)].fg, palette::TEXT);
    assert!(buffer[(sidebar.x + 2, sidebar.y + 1)]
        .modifier
        .contains(ratatui::style::Modifier::BOLD));
    assert_eq!(
        buffer[(sidebar.x + sidebar.width - 3, sidebar.y + 1)].bg,
        palette::QUEUE_BUTTON_FOCUSED_BG
    );
    for x in sidebar.x..sidebar.right() {
        assert_eq!(buffer[(x, sidebar.y + 2)].bg, palette::PLAYBACK_PANEL_BG);
        assert_eq!(buffer[(x, sidebar.y + 2)].symbol(), " ");
    }
}

#[test]
fn settings_content_has_two_column_and_one_row_insets() {
    let mut app = make_app_stub();
    let backend = TestBackend::new(20, 10);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        app.render_settings_panel(f, &mut layout, None);
    })
    .unwrap();

    assert_eq!(layout.settings_content_area, Rect::new(2, 4, 15, 3));
    let buffer = term.backend().buffer();
    for x in 0..19 {
        assert_eq!(buffer[(x, 3)].symbol(), " ");
    }
    for x in [0, 1, 18, 19] {
        assert_eq!(buffer[(x, 4)].bg, palette::PANEL_BG);
    }
    for x in 2..17 {
        assert_eq!(buffer[(x, 7)].bg, palette::PANEL_BG);
    }
}

#[test]
fn expanded_power_right_scrollbar_uses_first_right_gutter_column() {
    let backend = TestBackend::new(80, 5);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_power_right_scrollbar(f, Rect::new(2, 0, 76, 5), 3, 0);
    })
    .unwrap();

    let buffer = term.backend().buffer();
    assert_eq!(buffer[(77, 0)].symbol(), " ");
    assert_eq!(buffer[(78, 0)].symbol(), "▕");
    assert_eq!(buffer[(79, 0)].symbol(), " ");
}

#[test]
fn collapsed_power_right_panel_keeps_one_column_after_scrollbar() {
    let right_panel = Rect::new(0, 0, 80, 24);
    let content = power_right_panel_content_area(right_panel, true);
    assert_eq!(content.x + content.width, right_panel.right() - 1);
}

fn make_power_movie_app() -> App {
    let mut app = make_app_stub();
    app.library_tab = 1;

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let mut focused = make_item("Focused Movie", "Movie");
    focused.id = "movie-focused".into();
    focused.overview = "This overview should appear in the compact movie banner while the list remains visible underneath.".into();
    focused.director = "Director Hidden".into();
    focused.production_year = 1988;
    focused.genre = "Action".into();

    let mut second = make_item("Second Movie", "Movie");
    second.id = "movie-second".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![focused, second],
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

fn make_power_queue_app(item_count: usize) -> App {
    let mut app = make_power_movie_app();
    app.panel_focus = PanelFocus::Queue;
    app.player_tab.set_items(
        (0..item_count)
            .map(|i| make_item(&format!("Queue Item {i}"), "Movie"))
            .collect(),
        0,
    );
    app
}

fn make_power_remote_queue_app() -> App {
    let local_items = vec![make_item("Local Queue Item", "Movie")];
    let remote_items = vec![make_item("Remote Queue Item", "Movie")];
    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let mut app = App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, false);
    app.library_tab = 1;
    app.panel_focus = PanelFocus::Queue;
    app.queue_scope = QueueScope::Remote;
    app.player_tab.set_items(local_items, 0);
    app
}

#[test]
fn movie_library_unfocused_selected_banner_keeps_text_right_of_indicator() {
    let mut app = make_power_movie_app();
    let mut layout = LayoutMain::default();

    let term = render_power_library_to_terminal_focused(&mut app, &mut layout, false);
    let out = buffer_to_string(&term);
    let lines: Vec<&str> = out.lines().collect();

    // The colored-block look removes the green `▌` indicator entirely
    // (both focused and unfocused); the selected title sits inside the
    // MEDIA_SELECTED_BG block with a 2-col leading pad instead.
    let selected_line = lines
        .iter()
        .find(|line| line.contains("Focused Movie"))
        .expect("expected selected movie row");
    assert_eq!(
        selected_line.find('▌'),
        None,
        "expected no green selected-row indicator inside the colored block while unfocused:\n{out}"
    );

    let overview_line = lines
        .iter()
        .find(|line| line.contains("compact movie banner"))
        .expect("expected compact overview line");
    assert_eq!(
        overview_line.find('▌'),
        None,
        "expected no green banner bar inside the colored block while unfocused:\n{out}"
    );
}

#[test]
fn power_view_uses_configured_left_column_width() {
    let mut app = make_power_movie_app();
    app.queue_column_width = 55;

    let layout = render_power_view(&mut app, 100, 28);

    assert_eq!(layout.queue_area.width, 51);
}

#[test]
fn collapsed_power_left_column_gives_library_full_width() {
    let mut app = make_power_movie_app();
    app.queue_column_width = 55;
    app.queue_column_collapsed = true;

    let layout = render_power_view(&mut app, 100, 28);

    assert_eq!(layout.queue_area, Rect::default());
    assert_eq!(layout.left_area.x, 0);
    assert_eq!(layout.left_area.width, 99);
}

#[test]
fn short_power_view_keeps_queue_in_left_column() {
    let mut app = make_power_movie_app();
    app.queue_column_width = 40;

    let layout = render_power_view(&mut app, 100, 12);

    assert!(
        layout.queue_area.x < app.queue_column_width,
        "expected short-height queue to stay in the left column, got {:?}",
        layout.queue_area
    );
    assert!(
        layout.left_area.x >= app.queue_column_width,
        "expected library area to remain in the right column, got {:?}",
        layout.left_area
    );
}

#[test]
fn power_queue_panel_uses_selected_media_frame_and_background() {
    let mut app = make_power_queue_app(2);

    let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
    let buf = term.backend().buffer();
    let top_y = layout.queue_area.y - 1;
    let bottom_y = layout.queue_area.y + layout.queue_area.height;
    let x = layout.queue_area.x;

    assert_eq!(buf[(x, top_y)].symbol(), "\u{2594}");
    assert_eq!(buf[(x, top_y)].fg, palette::SEEK_TRACK);
    assert_eq!(buf[(x, bottom_y)].symbol(), "\u{2581}");
    assert_eq!(buf[(x, bottom_y)].fg, palette::SEEK_TRACK);
    assert_eq!(buf[(x, layout.queue_area.y)].bg, palette::MEDIA_SELECTED_BG);
    assert_eq!(
        buf[(x, layout.queue_area.y - 1)].bg,
        palette::MEDIA_SELECTED_BG
    );
    assert_eq!(
        buf[(x, layout.queue_area.y + layout.queue_area.height - 1)].bg,
        palette::MEDIA_SELECTED_BG
    );
}

#[test]
fn power_queue_panel_fills_remaining_left_column_with_short_queue() {
    let mut app = make_power_queue_app(1);

    let (_term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
    let bottom_y = layout.queue_area.y + layout.queue_area.height;

    assert_eq!(bottom_y, 26);
    assert!(
        layout.queue_area.height > 1,
        "expected queue viewport inside full-height panel, got {:?}",
        layout.queue_area
    );
}

#[test]
fn power_queue_panel_empty_state_is_inside_panel() {
    let mut app = make_power_queue_app(0);

    let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
    let out = buffer_to_string(&term);
    let empty_y = out
        .lines()
        .position(|line| line.contains("Add items with p"))
        .expect("expected queue empty-state message");

    assert_eq!(empty_y as u16, layout.queue_area.y);
    assert_eq!(
        term.backend().buffer()[(layout.queue_area.x, empty_y as u16)].bg,
        palette::MEDIA_SELECTED_BG
    );
}

#[test]
fn power_queue_panel_remains_visible_when_unfocused() {
    let mut app = make_power_queue_app(1);
    app.panel_focus = PanelFocus::Library;

    let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
    let buf = term.backend().buffer();
    let top_y = layout.queue_area.y - 1;
    let bottom_y = layout.queue_area.y + layout.queue_area.height;

    assert_eq!(buf[(layout.queue_area.x, top_y)].symbol(), "\u{2594}");
    assert_eq!(buf[(layout.queue_area.x, bottom_y)].symbol(), "\u{2581}");
    assert_eq!(
        buf[(layout.queue_area.x, layout.queue_area.y)].bg,
        palette::LIBRARY_SIDE_BG,
        "unfocused queue panel should use the dimmed background, not the focused MEDIA_SELECTED_BG"
    );
}

#[test]
fn power_queue_title_and_scope_pills_stay_outside_panel() {
    let mut app = make_power_remote_queue_app();
    app.use_nerd_fonts = false;

    let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
    let top_y = layout.queue_area.y - 1;
    let out = buffer_to_string(&term);
    let header = out
        .lines()
        .nth(layout.queue_scope_local_area.y as usize)
        .expect("expected queue header row");
    let device_name = mbv_core::api::device_name();
    let upper_device_name = device_name.to_uppercase();

    assert!(layout.queue_scope_local_area.y < top_y);
    assert!(layout.queue_scope_remote_area.y < top_y);
    assert!(layout.queue_scope_remote_area.x > layout.queue_scope_local_area.x);
    assert_eq!(
        layout.queue_scope_local_area.width + layout.queue_scope_remote_area.width,
        layout.queue_area.width
    );
    assert!(
        header.matches(&upper_device_name).count() >= 2,
        "expected local and remote queue controls to use session-style hostname pills:\n{out}"
    );
    assert!(
        !header.contains('\u{F0AFE}'),
        "expected non-Nerd-Font queue header to avoid private-use glyphs:\n{out}"
    );
}

#[test]
fn power_queue_title_does_not_render_playlist_pill() {
    let mut app = make_power_remote_queue_app();
    app.queue_source = crate::config::QueueSource::Playlist {
        id: None,
        name: "Road Mix".into(),
    };

    let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
    let out = buffer_to_string(&term);
    let header = out
        .lines()
        .nth(layout.queue_scope_local_area.y as usize)
        .expect("expected queue header row");
    let device_name = mbv_core::api::device_name();
    let upper_device_name = device_name.to_uppercase();

    assert!(
        header.contains(&upper_device_name),
        "expected session hostname pill in queue header:\n{out}"
    );
    assert!(
        !header.contains("Road Mix") && !header.contains("none"),
        "expected playlist pill to stay out of queue header:\n{out}"
    );
}

#[test]
fn power_view_bottom_status_bar_shows_playlist_pill_when_queue_is_a_playlist() {
    let mut app = make_power_queue_app(2);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".into()),
        name: "Road Mix".into(),
    };

    let (term, _layout) = render_power_view_to_terminal(&mut app, 100, 28);
    let out = buffer_to_string(&term);
    let last_line = out.lines().last().unwrap_or_default();

    assert!(
        last_line.contains("Road Mix"),
        "expected the playlist pill to appear in the Power View status bar:\n{last_line}"
    );
    let text_x = last_line
        .find("Road Mix")
        .expect("expected playlist name position") as u16;
    assert_eq!(
        term.backend().buffer()[(text_x, 27)].fg,
        palette::YELLOW,
        "expected playlist pill text to be yellow, not green:\n{last_line}"
    );
}

#[test]
fn short_power_queue_panel_drops_padding_before_rows() {
    let mut app = make_power_queue_app(20);

    let (term, layout) = render_power_view_to_terminal(&mut app, 100, 12);
    let buf = term.backend().buffer();
    let top_y = layout.queue_area.y - 1;
    let bottom_y = layout.queue_area.y + layout.queue_area.height;

    assert_eq!(buf[(layout.queue_area.x, top_y)].symbol(), "\u{2594}");
    assert_eq!(buf[(layout.queue_area.x, bottom_y)].symbol(), "\u{2581}");
    assert!(
        layout.queue_area.height >= 1,
        "expected at least one usable queue row on a short terminal, got {:?}",
        layout.queue_area
    );
}

#[test]
fn power_queue_panel_counts_wrapped_group_headers_before_adding_padding() {
    let mut app = make_power_movie_app();
    app.panel_focus = PanelFocus::Queue;
    let mut item = make_item("Track", "Audio");
    item.id = "boundary-track".into();
    item.album_id = "boundary-album".into();
    item.album = "Long Album Title".into();
    item.artist = "Very Long Artist".into();
    app.player_tab.set_items(vec![item], 0);

    let panel_area = Rect::new(0, 0, 20, 6);
    let backend = TestBackend::new(panel_area.width, panel_area.height);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        let queue_area = render_power_queue_panel_frame(f, panel_area, true);
        app.render_power_queue(f, queue_area, true, &mut layout);
    })
    .unwrap();
    let out = buffer_to_string(&term);

    assert_eq!(layout.queue_area.y, 1);
    assert_eq!(layout.queue_area.height, 4);
    assert!(
        layout.queue_row_map.contains(&Some(0)),
        "expected selected track row to be mapped as visible after wrapped header: {:?}",
        layout.queue_row_map
    );
    assert!(
        out.contains("1. Track"),
        "expected selected track to remain visible below the wrapped group header:\n{out}"
    );
}

#[test]
fn power_queue_panel_preserves_group_aware_scrolling() {
    let mut app = make_power_movie_app();
    app.panel_focus = PanelFocus::Queue;

    let mut items = Vec::new();
    for i in 0..4 {
        let mut item = make_item(&format!("A{i}"), "Audio");
        item.id = format!("a-{i}");
        item.album_id = "album-a".into();
        item.album = "Album A".into();
        item.artist = "Artist".into();
        items.push(item);
    }
    for i in 0..4 {
        let mut item = make_item(&format!("B{i}"), "Audio");
        item.id = format!("b-{i}");
        item.album_id = "album-b".into();
        item.album = "Album B".into();
        item.artist = "Artist".into();
        items.push(item);
    }
    app.player_tab.set_items(items, 4);
    app.queue_scroll = 9;

    let (_term, _layout) = render_power_view_to_terminal(&mut app, 100, 20);

    assert_eq!(app.queue_scroll, 9);
}

fn make_power_music_group_app() -> App {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.music_levels = vec!["group".into(), "album".into()];

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();

    // Six groups is enough to force horizontal scrolling in a narrow test terminal.
    let group_names = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"];
    let groups: Vec<MediaItem> = group_names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut it = make_item(n, "MusicArtist");
            it.id = format!("group-{i}");
            it.is_folder = true;
            it
        })
        .collect();

    let mut album = make_item("First Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = "Alpha".into();
    album.production_year = 2001;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![
            BrowseLevel {
                parent_id: "lib-music".into(),
                title: "Music".into(),
                items: groups,
                total_count: group_names.len(),
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            },
            BrowseLevel {
                parent_id: "group-0".into(),
                title: "Alpha".into(),
                items: vec![album],
                total_count: 1,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            },
        ],
        search: None,
        feed_home_video: None,

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

#[test]
fn selectable_artist_headers_are_typed_row_targets() {
    let mut app = make_power_music_group_app();
    // Headers for groups with only one child are not selectable, so give
    // Alpha a second album to keep it eligible as a typed row target.
    let mut alpha_album2 = make_item("Second Alpha Album", "MusicAlbum");
    alpha_album2.id = "album-1b".into();
    alpha_album2.artist = "Alpha".into();
    alpha_album2.is_folder = true;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(alpha_album2);
    let mut beta_album = make_item("Beta Album", "MusicAlbum");
    beta_album.id = "album-2".into();
    beta_album.artist = "Beta".into();
    beta_album.is_folder = true;
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(beta_album);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);

    assert!(
        out.contains("Alpha") && out.contains("Beta"),
        "expected both artist headers to render:\n{out}"
    );
    let header_row = layout
        .left_row_targets
        .iter()
        .position(|target| {
            matches!(
                target,
                Some(LibraryRowTarget::ArtistHeader(selection))
                    if selection.artist_label == "Alpha"
                        && selection.first_album_id == "album-1"
            )
        })
        .expect("expected the custom artist header to be a typed row target");
    assert_eq!(
        layout.left_row_map[header_row], None,
        "legacy row map must keep headers non-album rows"
    );
}

#[test]
fn grouped_album_rows_use_styled_suffix_and_single_group_spacers() {
    let mut app = make_power_music_group_app();
    let mut alpha_album = make_item("Second Alpha Album", "MusicAlbum");
    alpha_album.id = "album-1b".into();
    alpha_album.artist = "Alpha".into();
    alpha_album.production_year = 2002;
    let mut beta_album = make_item("Beta Album", "MusicAlbum");
    beta_album.id = "album-2".into();
    beta_album.artist = "Beta".into();
    beta_album.production_year = 2003;
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    level.items.extend([alpha_album, beta_album]);
    level.cursor = 2;

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal(&mut app, &mut layout);
    let out = buffer_to_string(&term);
    let lines: Vec<&str> = out.lines().collect();
    let alpha_y = lines
        .iter()
        .position(|line| line.contains("Alpha") && !line.contains("Album"))
        .expect("expected Alpha artist header");
    let beta_y = lines
        .iter()
        .position(|line| line.contains("Beta") && !line.contains("Album"))
        .expect("expected Beta artist header");
    let last_alpha_album_y = lines
        .iter()
        .rposition(|line| line.contains("Alpha Album"))
        .expect("expected the final Alpha album row");
    assert_eq!(
        beta_y,
        last_alpha_album_y + 4,
        "expected one spacer plus the selected group's frame before the next artist:\n{out}"
    );
    let last_selectable = layout
        .left_row_targets
        .iter()
        .rev()
        .find_map(Option::as_ref)
        .expect("expected a selectable album row");
    assert!(
        matches!(last_selectable, LibraryRowTarget::Album(_)),
        "expected no trailing artist spacer after the final album"
    );

    let album_y = lines
        .iter()
        .position(|line| line.contains("Second Alpha Album"))
        .expect("expected Alpha album row");
    let title_x = lines[album_y].find("Second Alpha Album").unwrap() as u16;
    let header_x = lines[alpha_y].find("Alpha").unwrap() as u16;
    assert_eq!(
        title_x, header_x,
        "album title should align with its header"
    );
    let bullet_x = lines[album_y].find('•').unwrap() as u16;
    let year_x = lines[album_y].find("2002").unwrap() as u16;
    let buffer = term.backend().buffer();
    assert_eq!(buffer[(title_x, album_y as u16)].fg, palette::WHITE);
    assert_eq!(buffer[(bullet_x, album_y as u16)].fg, palette::YELLOW);
    assert_eq!(buffer[(year_x, album_y as u16)].fg, palette::AQUA);

    let selected_album_y = lines
        .iter()
        .position(|line| line.contains("Beta Album"))
        .expect("expected selected Beta album row");
    let selected_title_x = lines[selected_album_y].find("Beta Album").unwrap() as u16;
    assert_eq!(
        buffer[(selected_title_x, selected_album_y as u16)].fg,
        palette::WHITE,
        "selected album titles should remain white"
    );
}

#[test]
fn artist_and_album_focus_share_one_selected_group_bounds() {
    let mut app = make_power_music_group_app();
    let mut second = make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    app.libs[0].nav_stack.last_mut().unwrap().items.push(second);
    let albums = app.libs[0].nav_stack.last().unwrap().items.clone();
    let header = crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Alpha".into(),
    };

    let album_plan =
        app.build_grouped_album_display_plan(&albums, 0, false, true, None, false, Some((120, 0)));
    let header_plan = app.build_grouped_album_display_plan(
        &albums,
        0,
        false,
        true,
        Some(&header),
        false,
        Some((120, 0)),
    );

    assert_eq!(
        album_plan.selected_block_bounds, header_plan.selected_block_bounds,
        "header and album focus should use the same artist-scoped frame"
    );
    assert_eq!(
        album_plan
            .rows
            .iter()
            .filter(|row| matches!(row, GroupedAlbumDisplayRow::Album(_)))
            .count(),
        2,
        "the selected group should emit the complete discography"
    );
}

#[test]
fn grouped_target_marker_and_inline_art_follow_album_or_artist_focus() {
    let mut album_app = make_power_music_group_app();
    let mut second = make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    album_app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second);
    album_app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    album_app.image_protocol_enabled = true;
    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut album_app, &mut layout);
    let selected_line = out
        .lines()
        .find(|line| line.contains("Second Album"))
        .expect("expected the focused album row");
    assert!(selected_line.contains('\u{258c}'));
    assert!(out.contains("First Album"));
    assert!(album_app.card_image_loading.contains("album-2:P"));
    assert!(!album_app.card_image_loading.contains("album-2:sq"));

    let mut header_app = make_power_music_group_app();
    let mut second = make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    header_app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second);
    header_app.image_protocol_enabled = true;
    header_app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Alpha".into(),
    });
    let mut header_layout = LayoutMain::default();
    let header_out = render_power_library_to_string(&mut header_app, &mut header_layout);
    let header_line = header_out
        .lines()
        .find(|line| line.contains("Alpha"))
        .expect("expected the focused artist row");
    assert!(header_line.contains('\u{258c}'));
    assert!(header_app.card_image_loading.contains("album-1:sq"));
}

#[test]
fn long_inline_track_focus_keeps_the_detail_table_inside_the_selected_block() {
    let mut app = make_power_music_group_app();
    app.libs[0].album_track_focus = Some(29);
    let tracks: Vec<_> = (0..30)
        .map(|i| {
            let mut track = make_item(&format!("Track {i}"), "Audio");
            track.id = format!("track-{i}");
            track.album = "First Album".into();
            track.artist = "Alpha".into();
            track.index_number = i + 1;
            track
        })
        .collect();
    app.album_tracks_cache.insert("album-1".into(), tracks);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);
    assert!(
        out.contains("Track 29"),
        "expected focused track in the block:\n{out}"
    );
    let title_line = out
        .lines()
        .find(|line| line.contains("First Album"))
        .expect("expected selected album row");
    assert!(title_line.contains('\u{258c}'));
    assert!(layout.cursor_screen_y.is_some());
}

#[test]
fn selectable_artist_header_renders_focused() {
    let mut app = make_power_music_group_app();
    app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
        first_album_id: "album-1".into(),
        artist_label: "Alpha".into(),
    });

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal(&mut app, &mut layout);
    let out = buffer_to_string(&term);
    let lines: Vec<&str> = out.lines().collect();
    let header_row = lines
        .iter()
        .position(|line| line.contains("Alpha"))
        .expect("expected Alpha header");
    let header = lines[header_row];
    let header_title_x = header.find("Alpha").expect("expected artist title") as u16;
    assert_eq!(
        term.backend().buffer()[(header_title_x, header_row as u16)].fg,
        palette::FOAM,
        "selected artist titles should use the project blue"
    );

    assert!(
        header.contains('\u{258c}'),
        "selected artist header should render the AQUA focus gutter:\n{out}"
    );
    assert!(
        !header.contains('\u{f037b}'),
        "selected artist header should no longer render the trailing focus icon \
         (the selection block now carries the focus signal):\n{out}"
    );

    // The header is wrapped in the artist-scoped block: a `▁` border row
    // (with a blank colored-bg padding row directly beneath it), the header,
    // an action-hint row, then the complete album region and a `▔` border.
    assert!(
        header_row >= 2 && lines[header_row - 2].contains('\u{2581}'),
        "expected a top border row two rows above the selected header:\n{out}"
    );
    let hint_row = header_row + 1;
    assert!(
        lines[hint_row].contains("^P: Play | ^A: Enqueue | ^S: Shuffle"),
        "expected the artist action-hint row directly below the header:\n{out}"
    );
    assert!(
        !lines[hint_row].contains("ENTER"),
        "artist action hint should not include the album's ENTER clause:\n{out}"
    );
    assert!(
        lines[hint_row + 1..]
            .iter()
            .any(|line| line.contains('\u{2594}')),
        "expected a bottom border row below the selected header block:\n{out}"
    );

    assert_eq!(
        layout.cursor_screen_y,
        Some(header_row as u16),
        "selected header should own the screen cursor row"
    );
}

#[test]
fn music_group_pills_render_on_row_below_title_marker() {
    let mut app = make_power_music_group_app();
    app.queue_column_width = 20;
    let width = 100u16;
    let height = 20u16;
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        app.render_main(
            f,
            Rect::new(0, 0, width, height),
            &mut layout,
            &mut LayoutPlayback::default(),
            &mut Rect::default(),
            &mut Rect::default(),
            0,
            false,
            &None,
        );
    })
    .unwrap();
    let out = buffer_to_string(&term);
    let row0 = out.lines().next().unwrap();
    let _row1 = out.lines().nth(1).unwrap();

    let row3 = out.lines().nth(3).unwrap();

    assert!(
        !row0.contains("Alpha") && !row0.contains("Beta"),
        "expected pills not on the first row:\n{out}"
    );
    assert!(
        row3.contains("Alpha") && row3.contains("Beta"),
        "expected group pills below the tab bar (no header row):\n{out}"
    );

    let _rchar_x = |line: &str, needle: &str| -> u16 {
        let byte_idx = line.rfind(needle).expect("needle not found");
        line[..byte_idx].chars().count() as u16
    };
    let char_x = |line: &str, needle: &str| -> u16 {
        let byte_idx = line.find(needle).expect("needle not found");
        line[..byte_idx].chars().count() as u16
    };

    let right_col_x = app.queue_column_width + POWER_VIEW_GAP;
    let buf = term.backend().buffer();
    assert!(
        row3.chars().take(right_col_x as usize).all(|c| c == ' '),
        "expected the pill row to be confined to the right library column:\n{out}"
    );

    let alpha_x = char_x(row3, "Alpha");
    assert!(
        alpha_x >= right_col_x,
        "expected pills confined to the right column"
    );
    assert_eq!(buf[(alpha_x, 3)].bg, palette::YELLOW);
    assert_eq!(
        buf[(alpha_x, 3)].fg,
        palette::PILL_DARK,
        "expected the selected group pill to use dark text"
    );
    let beta_x = char_x(row3, "Beta");
    assert_eq!(buf[(beta_x, 3)].bg, palette::LIBRARY_SIDE_BG);
    assert_eq!(
        buf[(beta_x, 3)].fg,
        palette::YELLOW,
        "expected a non-selected group pill to use yellow text"
    );

    let (gap_start, gap_end) = (alpha_x.min(beta_x), alpha_x.max(beta_x));
    let between: String = row3
        .chars()
        .skip(gap_start as usize)
        .take((gap_end - gap_start) as usize)
        .collect();
    assert!(
        !between.contains('\u{2501}'),
        "expected a blank gap between adjacent pills, not a dash rule:\n{between:?}"
    );

    assert!(!layout.selector_tabs.is_empty());
    for (rect, _) in &layout.selector_tabs {
        assert_eq!(rect.y, 3, "expected selector hitboxes on the pills row");
        assert!(
            rect.x >= right_col_x,
            "expected selector hitboxes confined to the right column"
        );
    }

    // Row 4 is a blank spacer between the pill row and the album list.
    let spacer_row = out.lines().nth(4).unwrap();
    assert!(
        spacer_row.trim().is_empty(),
        "expected a blank spacer row between the pills and the album list:\n{out}"
    );
    let album_row = out.lines().nth(7).unwrap();
    assert!(
        album_row.contains("Alpha") || album_row.contains("First Album"),
        "expected album list content to start below the pill/spacer rows:\n{out}"
    );
}

#[test]
fn music_group_pills_scroll_within_reserved_space_when_overflowing() {
    let mut app = make_power_music_group_app();
    app.queue_column_width = 20;
    let width = 40u16;
    let height = 20u16;
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        app.render_main(
            f,
            Rect::new(0, 0, width, height),
            &mut layout,
            &mut LayoutPlayback::default(),
            &mut Rect::default(),
            &mut Rect::default(),
            0,
            false,
            &None,
        );
    })
    .unwrap();
    let out = buffer_to_string(&term);
    let _row0 = out.lines().next().unwrap();

    let row3 = out.lines().nth(3).unwrap();
    let _row4 = out.lines().nth(4).unwrap();

    assert!(
        row3.contains('\u{203a}'),
        "expected a right scroll indicator on the pills row (no header gap):\n{out}"
    );

    let rchar_x = |line: &str, needle: &str| -> u16 {
        let byte_idx = line.rfind(needle).expect("needle not found");
        line[..byte_idx].chars().count() as u16
    };

    let right_indicator_x = rchar_x(row3, "\u{203a}");
    assert!(
        right_indicator_x < width,
        "expected the right scroll indicator to stay inside the pill row:\n{out}"
    );

    let right_col_x = (app.queue_column_width + POWER_VIEW_GAP) as usize;
    assert!(
        row3.chars().take(right_col_x).all(|c| c == ' '),
        "expected the pill row to be confined to the right library column:\n{out}"
    );

    assert!(!layout.selector_tabs.is_empty());
    for (rect, _) in &layout.selector_tabs {
        assert_eq!(rect.y, 3, "expected pill hitboxes on the pills row");
        assert!(
            rect.x as usize >= right_col_x,
            "expected pill hitboxes confined to the right column"
        );
        assert!(
            rect.x + rect.width <= width,
            "expected pill hitboxes confined to the visible pill row"
        );
    }
}

// ── inline album detail at the album-folder-listing level (#145, task 2) ──

#[test]
fn album_folder_listing_renders_list_and_inline_detail_together() {
    let mut app = make_power_music_group_app();
    // Sitting at the album-folder-listing level already (no drilldown push).
    assert_eq!(app.libs[0].nav_stack.len(), 2);

    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second_album);

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    // In the music-group (pill selector) view, inline tracks only render
    // once track-selection mode has been entered (Enter pressed).
    app.libs[0].album_track_focus = Some(0);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);
    let lines: Vec<&str> = out.lines().collect();

    assert!(
        out.contains("Alpha"),
        "expected the album list (grouped by artist) to still render:\n{out}"
    );
    assert!(
        out.contains("Opening Track"),
        "expected the selected album's cached tracks to render inline, \
         without any drilldown:\n{out}"
    );

    // Selection now reads via a colored MEDIA_SELECTED_BG block framed by
    // ▁/▔ unicode borders (movie-tab colored-block style), not the legacy
    // `─` rule + `▌` gutter.
    let title_y = lines
        .iter()
        .position(|l| l.contains("First Album"))
        .expect("expected selected album row");
    assert!(
        lines[title_y - 4].contains("\u{2581}"),
        "expected the artist block top border four rows above the first album:\n{out}"
    );
    assert!(
        lines[title_y - 3].trim().is_empty(),
        "expected the colored top-padding row above the artist header to be blank:\n{out}"
    );
    assert_eq!(
        lines.iter().filter(|line| line.trim() == "Alpha").count(),
        1,
        "plain album framing must not duplicate the artist name:\n{out}"
    );

    let track_y = lines
        .iter()
        .position(|l| l.contains("Opening Track"))
        .expect("expected inline track row");
    assert!(
        track_y > title_y,
        "expected the track row to render below the selected album title:\n{out}"
    );

    let second_album_y = lines
        .iter()
        .position(|l| l.contains("Second Album"))
        .expect("expected the following album row");
    assert!(
        second_album_y < track_y,
        "expected sibling albums to render before the inline track detail:\n{out}"
    );

    let title_row_idx = layout
        .left_row_map
        .iter()
        .position(|r| *r == Some(0))
        .expect("expected the selected album (index 0) in the row map");
    let second_row_idx = layout
        .left_row_map
        .iter()
        .position(|r| *r == Some(1))
        .expect("expected the following album (index 1) in the row map");
    assert!(
        second_row_idx > title_row_idx,
        "expected the following album's row-map entry after the selected album's"
    );
    assert!(
        layout.left_row_map[title_row_idx + 1..second_row_idx]
            .iter()
            .all(Option::is_none),
        "expected every row between the two albums (borders, padding, track detail) to be non-selectable:\n{:?}",
        layout.left_row_map
    );
    assert_eq!(
        app.libs[0].nav_stack.len(),
        2,
        "rendering the inline preview must not push a nav_stack level"
    );
}

#[test]
fn flat_album_folder_listing_renders_inline_detail_under_selected_album() {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.music_levels = vec!["album".into()];

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();

    let mut album = make_item("First Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = "Alpha".into();
    album.is_folder = true;
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    second_album.is_folder = true;

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-music".into(),
            title: "Music".into(),
            items: vec![album, second_album],
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);
    let lines: Vec<&str> = out.lines().collect();

    // Selection now reads via a colored MEDIA_SELECTED_BG block framed by
    // ▁/▔ unicode borders (movie-tab colored-block style), not the legacy
    // `─` rule + `▌` gutter. Structure per block:
    //   [border ▁] [colored padding] [album title] [tracks...] [colored padding] [border ▔]
    let title_y = lines
        .iter()
        .position(|l| l.contains("First Album"))
        .expect("expected selected album title row");
    assert!(
        lines[title_y - 2].contains("\u{2581}"),
        "expected a top border two rows above the title (border, then padding):\n{out}"
    );
    assert!(
        lines[title_y - 1].trim().is_empty(),
        "expected the colored top-padding row directly above the title to be blank:\n{out}"
    );
    assert_eq!(
        lines.iter().filter(|line| line.trim() == "Alpha").count(),
        1,
        "plain album framing must not duplicate the artist name:\n{out}"
    );

    let track_y = lines
        .iter()
        .position(|l| l.contains("Opening Track"))
        .expect("expected inline track row");
    assert!(
        track_y > title_y,
        "expected the track row to render below the selected album title:\n{out}"
    );

    let second_album_y = lines
        .iter()
        .position(|l| l.contains("Second Album"))
        .expect("expected the following album row");
    assert!(
        lines[second_album_y - 1].contains("\u{2594}"),
        "expected a bottom border directly above the following album row:\n{out}"
    );
    assert!(
        second_album_y > track_y,
        "expected the following album to render after the inline track detail:\n{out}"
    );

    // Row-map: only the Album() rows (title + following album) map to a
    // selectable index; every border/padding/track-detail row is `None`.
    let title_row_idx = layout
        .left_row_map
        .iter()
        .position(|r| *r == Some(0))
        .expect("expected the selected album (index 0) in the row map");
    let second_row_idx = layout
        .left_row_map
        .iter()
        .position(|r| *r == Some(1))
        .expect("expected the following album (index 1) in the row map");
    assert!(
        second_row_idx > title_row_idx,
        "expected the following album's row-map entry after the selected album's"
    );
    assert!(
        layout.left_row_map[title_row_idx + 1..second_row_idx]
            .iter()
            .all(Option::is_none),
        "expected every row between the two albums (borders, padding, track detail) to be non-selectable:\n{:?}",
        layout.left_row_map
    );
    assert!(
        layout
            .left_row_targets
            .iter()
            .all(|target| !matches!(target, Some(LibraryRowTarget::ArtistHeader(_)))),
        "flat/non-custom grouped album headers must remain non-selectable"
    );
}

#[test]
fn inline_album_track_selection_block_hides_its_own_scrollbar() {
    let mut app = make_app_stub();
    let mut tracks = Vec::new();
    for i in 0..20 {
        let mut track = make_item(&format!("Track {i}"), "Audio");
        track.id = format!("track-{i}");
        track.album = "Selected Album".into();
        track.index_number = i + 1;
        tracks.push(track);
    }

    let backend = TestBackend::new(30, 8);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        app.render_power_album_detail(
            f,
            Rect::new(0, 0, 30, 8),
            &tracks,
            12,
            true,
            true,
            true,
            false,
            0,
            &mut layout,
        );
    })
    .unwrap();
    let out = buffer_to_string(&term);

    assert!(
        !out.contains('\u{2590}'),
        "inline track-selection block must not draw its own scrollbar:\n{out}"
    );
}

#[test]
fn album_folder_listing_fetches_and_shows_loading_on_cache_miss() {
    let mut app = make_power_music_group_app();
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second_album);
    assert!(!app.album_tracks_cache.contains_key("album-1"));
    assert!(!app.album_tracks_loading.contains("album-1"));

    // In the music-group (pill selector) view, inline tracks (and the
    // fetch that populates them) only happen once track-selection mode
    // has been entered (Enter pressed).
    app.libs[0].album_track_focus = Some(0);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);
    let lines: Vec<&str> = out.lines().collect();

    assert!(
        app.album_tracks_loading.contains("album-1"),
        "expected a cache miss to trigger fetch_album_tracks for the \
         selected album:\n{out}"
    );
    assert!(
        out.to_lowercase().contains("loading"),
        "expected a loading indicator in the detail pane while the \
         fetch is in flight:\n{out}"
    );
    // Selection now reads via a colored MEDIA_SELECTED_BG block framed by
    // ▁/▔ unicode borders (movie-tab colored-block style), not the legacy
    // `─` rule + `▌` gutter.
    let title_y = lines
        .iter()
        .position(|l| l.contains("First Album"))
        .expect("expected selected album row");
    assert!(
        lines[title_y - 4].contains("\u{2581}"),
        "expected the artist block top border four rows above the first album:\n{out}"
    );
    assert!(
        lines[title_y - 3].trim().is_empty(),
        "expected the colored top-padding row above the artist header to be blank:\n{out}"
    );
    assert_eq!(
        lines.iter().filter(|line| line.trim() == "Alpha").count(),
        1,
        "plain album framing must not duplicate the artist name:\n{out}"
    );

    let loading_y = lines
        .iter()
        .position(|l| l.to_lowercase().contains("loading"))
        .expect("expected an inline loading row");
    assert!(
        loading_y > title_y,
        "expected the loading row to render below the selected album title:\n{out}"
    );

    let second_album_y = lines
        .iter()
        .position(|l| l.contains("Second Album"))
        .expect("expected the following album row");
    assert!(
        second_album_y < loading_y,
        "expected sibling albums to render before the inline loading row:\n{out}"
    );

    let title_row_idx = layout
        .left_row_map
        .iter()
        .position(|r| *r == Some(0))
        .expect("expected the selected album (index 0) in the row map");
    let second_row_idx = layout
        .left_row_map
        .iter()
        .position(|r| *r == Some(1))
        .expect("expected the following album (index 1) in the row map");
    assert!(
        second_row_idx > title_row_idx,
        "expected the following album's row-map entry after the selected album's"
    );
    assert!(
        layout.left_row_map[title_row_idx + 1..second_row_idx]
            .iter()
            .all(Option::is_none),
        "expected every row between the two albums (borders, padding, loading row) to be non-selectable:\n{:?}",
        layout.left_row_map
    );
}

#[test]
fn album_folder_inline_detail_is_hidden_until_track_selection_mode() {
    let mut app = make_power_music_group_app();

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal(&mut app, &mut layout);
    let out = buffer_to_string(&term);
    let lines: Vec<&str> = out.lines().collect();
    let buf = term.backend().buffer();

    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("First Album"))
            .count(),
        1,
        "expected no duplicate inline album title row:\n{out}"
    );

    assert!(
        !out.contains("Opening Track"),
        "expected inline tracks to stay hidden until track-selection mode is entered \
         (Enter pressed):\n{out}"
    );

    let hint_y = lines
        .iter()
        .position(|line| line.contains("^P: Play"))
        .expect("expected inline action hint row");
    assert!(
        // The full hint text is wider than this fixture's terminal, so
        // it's truncated with an ellipsis -- just check for the
        // still-visible prefix.
        lines[hint_y].contains("ENTER: Show"),
        "expected the collapsed hint row to prompt Enter to show tracks:\n{out}"
    );
    let hint_x = lines[hint_y]
        .find("^P: Play")
        .expect("expected hint x position");
    let title_y = lines
        .iter()
        .position(|line| line.contains("First Album"))
        .expect("expected selected album title row");
    let title_x = lines[title_y]
        .find("First Album")
        .expect("expected selected album title position");
    assert_eq!(
        hint_x,
        lines[title_y][..title_x].chars().count(),
        "expected collapsed hint content to align with the selected album title:\n{out}"
    );
    assert_eq!(
        buf[(hint_x as u16, hint_y as u16)].fg,
        palette::SOFT_WHITE,
        "expected inline action hints to render soft white:\n{out}"
    );
}

#[test]
fn selected_music_group_album_shows_right_aligned_art_before_track_mode() {
    let mut app = make_power_music_group_app();
    app.image_protocol_enabled = true;

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal(&mut app, &mut layout);
    let out = buffer_to_string(&term);
    let art_rect = layout
        .inline_image_rect
        .expect("expected selected album art rect before track mode");

    assert!(
        !out.contains("Opening Track"),
        "tracks should stay hidden until track-selection mode:\n{out}"
    );
    let lines: Vec<&str> = out.lines().collect();
    let header_y = lines
        .iter()
        .position(|line| line.trim() == "Alpha")
        .expect("expected the artist header row");
    assert_eq!(
        art_rect.y, header_y as u16,
        "album artwork should start on the selected block's artist row"
    );
    assert_eq!(
        art_rect.x + art_rect.width,
        58,
        "album art should have two columns of right padding"
    );
    assert_eq!((art_rect.width, art_rect.height), (24, 12));
    assert!(app.card_image_loading.contains("album-1:P"));
    assert!(!app.card_image_loading.contains("track-1:P"));
    assert_eq!(
        term.backend().buffer()[(art_rect.x, art_rect.y)].bg,
        palette::OVERLAY,
        "loading album art should reserve a right-aligned placeholder:\n{out}"
    );
}

#[test]
fn selected_album_block_wraps_text_around_art_without_moving_art() {
    let mut app = make_power_music_group_app();
    app.image_protocol_enabled = true;
    app.libs[0].album_track_focus = Some(0);
    let album = &mut app.libs[0].nav_stack.last_mut().unwrap().items[0];
    album.name = "A Very Long Album Title That Wraps Before Artwork".into();
    album.artist = "Fallback Artist With A Very Long Name That Wraps Clearly".into();
    let mut track = make_item(
        "A Very Long Track Name That Continues Below The Artwork Width",
        "Audio",
    );
    track.id = "track-1".into();
    track.album = album.name.clone();
    track.artist = album.artist.clone();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    let mut layout = LayoutMain::default();
    let backend = TestBackend::new(50, 35);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_power_library(f, Rect::new(0, 0, 50, 35), true, &mut layout);
    })
    .unwrap();
    let out = buffer_to_string(&term);
    let lines: Vec<&str> = out.lines().collect();
    let art_rect = layout
        .inline_image_rect
        .expect("expected selected album artwork");
    let title_y = lines
        .iter()
        .position(|line| line.contains("A Very Long"))
        .unwrap_or_else(|| panic!("expected wrapped album title:\n{out}"));
    let header_y = lines
        .iter()
        .position(|line| line.contains("Fallback Artist"))
        .unwrap_or_else(|| panic!("expected artist header row:\n{out}"));
    assert_eq!(art_rect.y, header_y as u16);
    assert!(
        lines.iter().any(|line| line.contains("^P: Play"))
            && lines.iter().any(|line| line.contains("Shuffle")),
        "expected wrapped action hint rows:\n{out}"
    );
    assert!(
        lines.iter().any(|line| line.contains("That Continue"))
            && lines.iter().any(|line| line.trim() == "Artwork Width"),
        "expected wrapped inline track rows:\n{out}"
    );
    for line in &lines[title_y..] {
        if line.contains("A Very Long Album")
            || line.contains("^P: Play")
            || line.contains("Shuffle")
            || line.contains("A Very Long Track")
            || line.contains("Artwork Width")
        {
            let last_text_x = line
                .chars()
                .enumerate()
                .filter(|(_, ch)| !ch.is_whitespace())
                .map(|(x, _)| x as u16)
                .max()
                .unwrap();
            assert!(
                last_text_x < art_rect.x,
                "selected-block text must not draw beneath artwork:\n{out}"
            );
        }
    }
}

#[test]
fn selected_music_group_album_keeps_right_aligned_art_in_track_mode() {
    let mut app = make_power_music_group_app();
    app.image_protocol_enabled = true;
    app.libs[0].album_track_focus = Some(0);

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.player_tab.set_items(vec![track.clone()], 0);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.paused = false;
    }
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal(&mut app, &mut layout);
    let out = buffer_to_string(&term);
    let art_rect = layout
        .inline_image_rect
        .expect("expected selected album art rect in track mode");

    assert!(
        out.contains("Opening Track"),
        "expected inline track row:\n{out}"
    );
    let lines: Vec<&str> = out.lines().collect();
    let playing_line = lines
        .iter()
        .find(|line| line.contains("Opening Track"))
        .copied()
        .expect("expected active music track row");
    let icon = super::play_icon(app.use_nerd_fonts);
    assert!(
        playing_line.contains(&format!("1. {icon} Opening Track")),
        "expected the active track icon and following space after its number:\n{out}"
    );
    let track_y = lines
        .iter()
        .position(|line| line.contains("Opening Track"))
        .expect("expected inline track row");
    let hint_y = lines[..track_y]
        .iter()
        .rposition(|line| line.contains("^P: Play"))
        .expect("expected track-mode action hint row");
    assert!(
        lines[hint_y..track_y]
            .iter()
            .any(|line| line.contains("BACK: Exit")),
        "expected track-mode hint row to show the exit hint:\n{out}"
    );
    assert!(
        track_y > hint_y,
        "expected the track list below the track-mode hint:\n{out}"
    );
    let hint_x = lines[hint_y]
        .find("^P: Play")
        .expect("expected track-mode hint x position");
    assert_eq!(
        hint_x, 1,
        "track-mode detail hint keeps its existing indent"
    );
    assert!(
        lines[track_y].starts_with("  \u{258c}1."),
        "track list should be indented 2 columns from the album block title:\n{out}"
    );
    let icon_byte_x = playing_line
        .find(icon)
        .expect("expected active music track icon");
    let icon_x = playing_line[..icon_byte_x].chars().count() as u16;
    let title_byte_x = playing_line
        .find("Opening Track")
        .expect("expected active music track title");
    let active_title_x = playing_line[..title_byte_x].chars().count() as u16;
    let buffer = term.backend().buffer();
    assert_eq!(
        buffer[(icon_x, track_y as u16)].fg,
        palette::AQUA,
        "expected active icon to be AQUA at x={icon_x}:\n{out}"
    );
    assert_eq!(buffer[(active_title_x, track_y as u16)].fg, palette::YELLOW);
    assert_eq!(
        term.backend().buffer()[(hint_x as u16, hint_y as u16)].fg,
        palette::SOFT_WHITE,
        "expected track-mode action hints to render soft white:\n{out}"
    );
    assert_eq!(
        art_rect.x + art_rect.width,
        58,
        "album art should have two columns of right padding"
    );
    assert_eq!((art_rect.width, art_rect.height), (24, 12));
    assert!(app.card_image_loading.contains("album-1:P"));
    assert!(!app.card_image_loading.contains("track-1:P"));
    assert_eq!(
        term.backend().buffer()[(art_rect.x, art_rect.y)].bg,
        palette::OVERLAY,
        "loading album art should reserve a right-aligned placeholder:\n{out}"
    );
}

#[test]
fn album_folder_inline_detail_keeps_title_gutter_when_library_pane_unfocused() {
    // Selection now reads via a colored block + white title text, not the
    // legacy `▌` marker -- confirm that block dims (rather than
    // disappearing) and the title stays white when the pane loses focus.
    let mut app = make_power_music_group_app();

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    track.album = "First Album".into();
    track.artist = "Alpha".into();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal_focused(&mut app, &mut layout, false);
    let out = buffer_to_string(&term);
    let title_y = out
        .lines()
        .position(|line| line.contains("First Album"))
        .expect("expected selected album title row");
    let title_line = out.lines().nth(title_y).unwrap();
    let title_x = title_line
        .find("First Album")
        .expect("expected title text position") as u16;

    let buf = term.backend().buffer();
    assert_eq!(
        buf[(title_x, title_y as u16)].bg,
        palette::PLAYBACK_PANEL_BG,
        "selected album title row should keep a colored block background (dimmed) while unfocused:\n{out}"
    );
    assert_eq!(
        buf[(title_x, title_y as u16)].fg,
        palette::WHITE,
        "selected album title should keep its white text while unfocused:\n{out}"
    );
}

#[test]
fn album_folder_listing_preserves_inline_track_focus_cursor() {
    let mut app = make_power_music_group_app();
    app.libs[0].album_track_focus = Some(1);

    let mut first = make_item("Opening Track", "Audio");
    first.id = "track-1".into();
    first.album = "First Album".into();
    first.artist = "Alpha".into();
    first.index_number = 1;

    let mut second = make_item("Focused Track", "Audio");
    second.id = "track-2".into();
    second.album = "First Album".into();
    second.artist = "Alpha".into();
    second.index_number = 2;

    app.album_tracks_cache
        .insert("album-1".into(), vec![first, second]);

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);
    let focused_line = out
        .lines()
        .find(|line| line.contains("Focused Track"))
        .expect("expected focused track to render inline");
    let focused_y = out
        .lines()
        .position(|line| line.contains("Focused Track"))
        .expect("expected focused track row");
    let lines: Vec<&str> = out.lines().collect();
    let hint_y = lines[..focused_y]
        .iter()
        .rposition(|line| line.contains("BACK: Exit"))
        .expect("expected track-mode action hint row");
    assert!(
        lines[hint_y].contains("BACK: Exit"),
        "expected track-mode hint row to show the exit hint:\n{out}"
    );
    assert!(
        lines[hint_y + 1].trim().is_empty(),
        "expected a blank row between the track-mode hint and tracks:\n{out}"
    );
    assert_eq!(
        focused_y,
        hint_y + 3,
        "expected second track after hint, blank separator, and first track:\n{out}"
    );

    assert!(
        // The AQUA `▌` cursor marker now has 2-column indent in track-selection mode.
        focused_line.starts_with("  \u{258c}2. Focused Track"),
        "expected focused track row to show the AQUA cursor marker with 2-column indent in track-selection mode:\n{out}"
    );
    assert_eq!(
        layout.cursor_screen_y,
        Some(focused_y as u16),
        "expected layout cursor to follow the focused inline track row"
    );
}

#[test]
fn album_folder_track_focus_cursor_renders_when_library_pane_unfocused() {
    let mut app = make_power_music_group_app();
    app.libs[0].album_track_focus = Some(1);

    let mut first = make_item("Opening Track", "Audio");
    first.id = "track-1".into();
    first.album = "First Album".into();
    first.artist = "Alpha".into();
    first.index_number = 1;

    let mut second = make_item("Focused Track", "Audio");
    second.id = "track-2".into();
    second.album = "First Album".into();
    second.artist = "Alpha".into();
    second.index_number = 2;

    app.album_tracks_cache
        .insert("album-1".into(), vec![first, second]);

    let mut layout = LayoutMain::default();
    let term = render_power_library_to_terminal_focused(&mut app, &mut layout, false);
    let out = buffer_to_string(&term);
    let focused_line = out
        .lines()
        .find(|line| line.contains("Focused Track"))
        .expect("expected focused track to render inline");

    assert!(
        // The AQUA `▌` cursor marker now has 2-column indent in track-selection mode.
        focused_line.starts_with("  \u{258c}2. Focused Track"),
        "expected track-selection row to show the AQUA cursor marker with 2-column indent while pane is unfocused:\n{out}"
    );
}

#[test]
fn selected_album_item_follows_raw_cursor_not_display_order() {
    let mut app = make_power_music_group_app();

    // A second album whose artist sorts before "Alpha" -- if the cursor
    // were (mis)interpreted against the artist-grouped display order
    // instead of the raw `items` array, moving the cursor to 1 would
    // resolve to the wrong album here.
    let mut second_album = make_item("Zero Day", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Aaardvark".into();

    {
        let lvl = app.libs[0].nav_stack.last_mut().unwrap();
        lvl.items.push(second_album);
        lvl.cursor = 1;
    }

    let selected = app
        .selected_album_item(0)
        .expect("expected a selected album at cursor 1");
    assert_eq!(
        selected.id, "album-2",
        "expected the raw items[cursor] entry, not a sorted/display-order lookup"
    );

    // In the music-group (pill selector) view, the inline-detail fetch
    // (and thus this test's target assertion) only happens once
    // track-selection mode has been entered.
    app.libs[0].album_track_focus = Some(0);

    let mut layout = LayoutMain::default();
    let _ = render_power_library_to_string(&mut app, &mut layout);
    assert!(
        app.album_tracks_loading.contains("album-2"),
        "expected the fetch triggered by rendering to target the cursor-selected \
         album (album-2), not album-1"
    );
    assert!(
        !app.album_tracks_loading.contains("album-1"),
        "album-1 is no longer selected, so it should not be (re)fetched"
    );
}

// ── #145 task 5: regression coverage for non-music Power View surfaces ──
// `is_viewing_album_folders` gates on `collection_type == "music"`, so
// this is provably unreachable for series/home-video libraries; the
// tests below additionally prove the *render* path
// (`render_power_library`) still picks the original single-pane
// series/home-video renderer and never touches the new album-tracks
// cache/track-focus machinery added in tasks 1-4.

fn make_power_home_video_app() -> App {
    let mut app = make_app_stub();
    app.library_tab = 1;

    let mut library = make_item("Home Videos", "CollectionFolder");
    library.id = "lib-homevideos".into();
    library.is_folder = true;
    library.collection_type = "homevideos".into();

    let mut first = make_item("Birthday Clip", "Video");
    first.id = "video-1".into();
    let mut second = make_item("Vacation Clip", "Video");
    second.id = "video-2".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-homevideos".into(),
            title: "Home Videos".into(),
            items: vec![first, second],
            total_count: 2,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,

        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app
}

#[test]
fn home_video_library_is_never_album_folders_and_renders_via_original_list_path() {
    let mut app = make_power_home_video_app();
    let lib_idx = 0;

    assert!(
        !app.is_viewing_album_folders(lib_idx),
        "a homevideos library must never satisfy is_viewing_album_folders"
    );
    assert!(app.is_home_video_view(lib_idx));
    assert!(app.libs[lib_idx].album_track_focus.is_none());

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);

    assert!(
        out.contains("Birthday Clip"),
        "expected the original single-pane home-video list renderer to fire \
         unchanged:\n{out}"
    );
    assert!(
        app.album_tracks_cache.is_empty(),
        "home-video rendering must never touch the album-tracks cache added by #145"
    );
    assert!(
        app.libs[lib_idx].album_track_focus.is_none(),
        "home-video rendering must never set track-selection mode"
    );
}

#[test]
fn letter_filter_buckets_match_emby_name_range_bounds() {
    // Verified empirically against a live Emby server (2026-07-22) that
    // NameStartsWithOrGreater/NameLessThan filter on SortName -- these
    // bounds must stay in lockstep with `letter_bucket`'s range labels.
    let ac = LetterFilter::for_index(0).unwrap();
    assert_eq!(ac.label, "A\u{2013}C");
    assert_eq!(ac.name_ge, Some("A"));
    assert_eq!(ac.name_lt, Some("D"));

    let vz = LetterFilter::for_index(7).unwrap();
    assert_eq!(vz.label, "V\u{2013}Z");
    assert_eq!(vz.name_ge, Some("V"));
    assert_eq!(vz.name_lt, None, "V–Z has no upper bound");

    let hash = LetterFilter::for_index(8).unwrap();
    assert_eq!(hash.label, "#");
    assert_eq!(hash.name_ge, None, "# has no lower bound");
    assert_eq!(hash.name_lt, Some("A"));

    assert!(LetterFilter::for_index(9).is_none());
    assert_eq!(LetterFilter::count(), 9);
    assert_eq!(LetterFilter::labels().len(), 9);
}

#[test]
fn letter_filter_default_is_the_first_bucket() {
    assert_eq!(
        LetterFilter::default_filter(),
        LetterFilter::for_index(0).unwrap()
    );
}

fn make_power_large_movie_library_app(library_total: usize) -> App {
    let mut app = make_app_stub();
    app.library_tab = 1;

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: Vec::new(),
            total_count: 0,
            cursor: 0,
            scroll: 0,
            item_types: Some("Movie".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        }],
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: Some(library_total),
    });

    app
}

#[test]
fn letter_pills_show_only_when_library_total_exceeds_threshold() {
    let mut small = make_power_large_movie_library_app(LIBRARY_PILL_THRESHOLD);
    assert!(
        !small.should_show_letter_pills(0),
        "exactly the threshold must not qualify"
    );

    let mut large = make_power_large_movie_library_app(LIBRARY_PILL_THRESHOLD + 1);
    assert!(large.should_show_letter_pills(0));

    // `render_power_library_to_string` calls `render_power_library`
    // directly, which is *below* the pill-row layout carve-out (that
    // lives in `render_main`, mirroring the music-group pills
    // row) -- go through the full view so the carve-out fires.
    let backend = TestBackend::new(60, 20);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        large.render_main(
            f,
            Rect::new(0, 0, 60, 20),
            &mut layout,
            &mut crate::app::layout::LayoutPlayback::default(),
            &mut Rect::default(),
            &mut Rect::default(),
            0,
            false,
            &None,
        );
    })
    .unwrap();
    let out = buffer_to_string(&term);
    assert!(
        out.contains("A\u{2013}C"),
        "expected the default A–C pill to render:\n{out}"
    );
    assert!(
        !layout.selector_tabs.is_empty(),
        "expected pill hitboxes to be recorded for click dispatch"
    );

    // Rendering the small (non-qualifying) library must not show pills.
    let backend2 = TestBackend::new(60, 20);
    let mut term2 = Terminal::new(backend2).unwrap();
    let mut layout2 = LayoutMain::default();
    term2
        .draw(|f| {
            small.render_main(
                f,
                Rect::new(0, 0, 60, 20),
                &mut layout2,
                &mut crate::app::layout::LayoutPlayback::default(),
                &mut Rect::default(),
                &mut Rect::default(),
                0,
                false,
                &None,
            );
        })
        .unwrap();
    let out2 = buffer_to_string(&term2);
    assert!(!out2.contains("A\u{2013}C"));
}
