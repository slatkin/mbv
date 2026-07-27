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
            true,
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
        lines[title_y - 5].contains("\u{2581}"),
        "expected the artist block top border five rows above the first album:\n{out}"
    );
    assert!(
        lines[title_y - 4].trim().is_empty(),
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
        second_album_y > loading_y,
        "expected the inline loading row to render before sibling albums:\n{out}"
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
    assert_eq!((art_rect.width, art_rect.height), (30, 15));
    assert!(app.card_image_loading.contains("album-1:P"));
    assert!(!app.card_image_loading.contains("track-1:P"));
    assert_eq!(
        term.backend().buffer()[(art_rect.x, art_rect.y)].bg,
        palette::OVERLAY,
        "loading album art should reserve a right-aligned placeholder:\n{out}"
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
