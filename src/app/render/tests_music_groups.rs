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
        palette::FOAM,
        "selected album titles should be foam"
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
