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
        out.contains("Openi") && out.contains("Track"),
        "expected inline track row (may be wrapped):\n{out}"
    );
    let lines: Vec<&str> = out.lines().collect();
    let playing_line = lines
        .iter()
        .find(|line| line.contains("Openi"))
        .copied()
        .expect("expected active music track row");
    let icon = super::play_icon(app.use_nerd_fonts);
    assert!(
        playing_line.contains(&format!("1. {icon} Openi")),
        "expected the active track icon and following space after its number:\n{out}"
    );
    let track_y = lines
        .iter()
        .position(|line| line.contains("Openi"))
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
        hint_x, 2,
        "track-mode detail hint has 2-column indent in grouped block"
    );
    assert!(
        lines[track_y].starts_with("        \u{258c}1."),
        "track list should be indented with block padding:\n{out}"
    );
    let icon_byte_x = playing_line
        .find(icon)
        .expect("expected active music track icon");
    let icon_x = playing_line[..icon_byte_x].chars().count() as u16;
    let title_byte_x = playing_line
        .find("Openi")
        .expect("expected active music track title");
    let active_title_x = playing_line[..title_byte_x].chars().count() as u16;
    let buffer = term.backend().buffer();
    assert_eq!(
        buffer[(icon_x, track_y as u16)].fg,
        palette::AQUA,
        "expected active icon to be AQUA at x={icon_x}:\n{out}"
    );
    assert_eq!(
        buffer[(active_title_x, track_y as u16)].fg,
        palette::SOFT_WHITE
    );
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
    assert_eq!((art_rect.width, art_rect.height), (30, 15));
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
    let hint_y = lines
        .iter()
        .position(|line| line.contains("BACK: Exit"))
        .expect("expected track-mode action hint row");
    assert!(
        lines[hint_y].contains("BACK: Exit"),
        "expected track-mode hint row to show the exit hint:\n{out}"
    );
    let album_y = lines
        .iter()
        .position(|line| line.contains("First Album"))
        .expect("expected album title row");
    assert!(album_y > hint_y, "expected album title after hint:\n{out}");
    let first_track_y = lines
        .iter()
        .position(|line| line.contains("Opening Track"))
        .expect("expected first track row");
    assert!(
        first_track_y > album_y,
        "expected tracks after album title:\n{out}"
    );
    assert_eq!(
        focused_y,
        first_track_y + 1,
        "expected second track after first track:\n{out}"
    );

    assert!(
        // The AQUA `▌` cursor marker now has 2-column indent in track-selection mode,
        // plus the 6-column block indentation.
        focused_line.starts_with("        \u{258c}2. Focused Track"),
        "expected focused track row to show the AQUA cursor marker with proper indent in track-selection mode:\n{out}"
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
        // The AQUA `▌` cursor marker now has 2-column indent in track-selection mode,
        // plus the 6-column block indentation.
        focused_line.starts_with("        \u{258c}2. Focused Track"),
        "expected track-selection row to show the AQUA cursor marker with proper indent while pane is unfocused:\n{out}"
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
