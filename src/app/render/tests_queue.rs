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
    let bottom_y = layout.queue_area.y + layout.queue_area.height;
    let x = layout.queue_area.x;

    assert_eq!(buf[(x, bottom_y)].symbol(), "\u{2581}");
    assert_eq!(buf[(x, bottom_y)].fg, palette::SEEK_TRACK);
    assert_eq!(buf[(x, layout.queue_area.y)].bg, palette::MEDIA_SELECTED_BG);
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
    let bottom_y = layout.queue_area.y + layout.queue_area.height;

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
    let bottom_y = layout.queue_area.y + layout.queue_area.height;

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
    // A group header only renders for a run of 3+ same-album items, so use
    // three items here (rather than one) to keep exercising the wrapped-header
    // line-counting behavior this test targets.
    let mut items = Vec::new();
    for i in 0..3 {
        let mut item = make_item("Track", "Audio");
        item.id = format!("boundary-track-{i}");
        item.album_id = "boundary-album".into();
        item.album = "Long Album Title".into();
        item.artist = "Very Long Artist".into();
        items.push(item);
    }
    app.player_tab.set_items(items, 0);

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

    assert_eq!(layout.queue_area.y, 0);
    assert_eq!(layout.queue_area.height, 5);
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
