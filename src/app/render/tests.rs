#![allow(dead_code, unused_imports)]

use super::album_plan::GroupedAlbumDisplayRow;
use super::*;
use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibSearch, LibraryTab, QueueScope, RemoteSlotState};
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
