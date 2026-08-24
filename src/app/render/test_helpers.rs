#![allow(dead_code, unused_imports)]

use super::screens::album_plan::GroupedAlbumDisplayRow;
use super::*;
use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, QueueScope, RemoteSlotState, TabSelection};
use crate::config::Config;
use mbv_core::api::EmbyClient;
use mbv_core::api::EmbyItem;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;

pub fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
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

pub fn render_sidebar_scrollbar_column(total: usize, visible: u16, scroll: usize) -> String {
    let backend = TestBackend::new(1, visible);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        super::components::chrome::render_sidebar_scrollbar(
            f,
            Rect::new(0, 0, 0, visible),
            total,
            scroll,
        );
    })
    .unwrap();
    buffer_to_string(&term)
}

pub fn render_scrollbar_column(height: u16, max_offset: usize, offset: usize) -> String {
    let backend = TestBackend::new(1, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_right_scrollbar(
            f,
            Rect::new(0, 0, 1, height),
            max_offset,
            offset,
            palette::TEXT_METADATA,
        );
    })
    .unwrap();
    buffer_to_string(&term)
}

pub fn render_scrollbar_column_with_viewport(
    height: u16,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
) -> String {
    let backend = TestBackend::new(1, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        render_right_scrollbar_with_viewport(
            f,
            Rect::new(0, 0, 1, height),
            content_length,
            viewport_content_length,
            offset,
            palette::TEXT_METADATA,
        );
    })
    .unwrap();
    buffer_to_string(&term)
}

pub fn render_pill_bar_hitboxes(
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
            },
        );
    })
    .unwrap();
    tabs
}

pub fn assert_surface_pills(
    terminal: &Terminal<TestBackend>,
    layout: &LayoutMain,
    panel: Rect,
    expected_pill_rows: usize,
    spacer_bg: Color,
    expected_ids: &[usize],
    expected_labels: &[&str],
    selected_id: usize,
) {
    assert_eq!(
        layout
            .selector_tabs
            .iter()
            .map(|(_, id)| *id)
            .collect::<Vec<_>>(),
        expected_ids,
        "surface pill targets"
    );
    let first = layout
        .selector_tabs
        .first()
        .expect("surface should publish pill targets")
        .0;
    assert!(
        layout
            .selector_tabs
            .iter()
            .all(|(rect, _)| rect.y == first.y && rect.height == 1),
        "pill hitboxes must occupy one shared row: {:?}",
        layout.selector_tabs
    );
    let buffer = terminal.backend().buffer();
    let painted_rows = (panel.y..panel.bottom())
        .filter(|y| (panel.x..panel.right()).any(|x| matches!(buffer[(x, *y)].symbol(), "◢" | "◤")))
        .collect::<Vec<_>>();
    assert_eq!(
        painted_rows.len(),
        expected_pill_rows,
        "painted pill rows in designated panel: panel={panel:?} targets={:?}",
        layout.selector_tabs
    );
    assert!(
        painted_rows.contains(&first.y),
        "target row is not a painted pill row: targets={:?} rows={painted_rows:?}",
        layout.selector_tabs
    );
    let row_text = (0..buffer.area().width)
        .map(|x| buffer[(x, first.y)].symbol())
        .collect::<String>();
    for label in expected_labels {
        assert!(
            row_text.contains(label),
            "pill row missing {label:?}: {row_text:?}"
        );
    }
    assert_eq!(
        buffer[(first.x, first.y)].style().bg,
        Some(palette::PILL_ROW_BG),
        "pill row background"
    );
    for pill_y in &painted_rows {
        assert!(
            *pill_y + 1 < panel.bottom(),
            "reserved spacer must fit in panel"
        );
        for x in panel.x..panel.right() {
            assert_eq!(
                buffer[(x, *pill_y + 1)].style().bg,
                Some(spacer_bg),
                "reserved spacer background at x={x}, y={}",
                *pill_y + 1
            );
        }
    }
    let painted_spans = (first.x..panel.right())
        .filter(|x| buffer[(*x, first.y)].symbol() == "◢")
        .filter_map(|start| {
            (start + 1..panel.right())
                .find(|x| buffer[(*x, first.y)].symbol() == "◤")
                .map(|end| Rect::new(start, first.y, end - start + 1, 1))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        painted_spans,
        layout
            .selector_tabs
            .iter()
            .map(|(rect, _)| *rect)
            .collect::<Vec<_>>(),
        "pill hitboxes must match painted horizontal spans"
    );
    for rect in layout.selector_tabs.iter().map(|(rect, _)| *rect) {
        assert!(
            panel.contains((rect.x, rect.y).into())
                && panel.contains((rect.right() - 1, rect.bottom() - 1).into()),
            "pill target outside designated panel: {rect:?} panel={panel:?}"
        );
    }
    let selected = layout
        .selector_tabs
        .iter()
        .find(|(_, id)| *id == selected_id)
        .expect("selected pill id should have a hitbox")
        .0;
    assert_eq!(
        buffer[(selected.x + 1, selected.y)].style().bg,
        Some(palette::PILL_SELECTED_BG),
        "selected pill appearance"
    );
}

pub fn render_library_to_terminal(app: &mut App, layout: &mut LayoutMain) -> Terminal<TestBackend> {
    render_library_to_terminal_focused(app, layout, true)
}

pub fn render_library_to_terminal_focused(
    app: &mut App,
    layout: &mut LayoutMain,
    focused: bool,
) -> Terminal<TestBackend> {
    let backend = TestBackend::new(60, 20);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_library(f, Rect::new(0, 0, 60, 20), focused, layout);
    })
    .unwrap();
    term
}

pub fn render_library_to_string(app: &mut App, layout: &mut LayoutMain) -> String {
    let term = render_library_to_terminal(app, layout);
    buffer_to_string(&term)
}

/// Like `render_library_to_string` but at an explicit terminal size, for
/// tests that need more rows than the default 60x20 (e.g. music-group views
/// whose hero panel reserves most of a short terminal).
pub fn render_library_to_string_sized(
    app: &mut App,
    layout: &mut LayoutMain,
    width: u16,
    height: u16,
) -> String {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        app.render_library(f, Rect::new(0, 0, width, height), true, layout);
    })
    .unwrap();
    buffer_to_string(&term)
}

pub fn render_view_to_terminal(
    app: &mut App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, LayoutMain) {
    // Mirror App::render(), which syncs terminal_width from the drawn Rect
    // before render_main runs -- without this, effective_panel_mode()/
    // effective_panel_focus() see whatever width the app was constructed
    // with instead of the width this call is actually rendering at.
    app.terminal_width = width;
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
            0,
            false,
            &None,
        );
    })
    .unwrap();
    (term, layout)
}

pub fn render_app_to_terminal(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.render(f)).unwrap();
    term
}

pub fn render_view(app: &mut App, width: u16, height: u16) -> LayoutMain {
    render_view_to_terminal(app, width, height).1
}

pub fn make_movie_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

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
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    app
}

pub fn make_queue_app(item_count: usize) -> App {
    let mut app = make_movie_app();
    app.panel_focus = PanelFocus::Queue;
    app.player_tab.set_items(
        (0..item_count)
            .map(|i| make_item(&format!("Queue Item {i}"), "Movie"))
            .collect(),
        0,
    );
    app
}

pub fn make_remote_queue_app() -> App {
    let local_items = vec![make_item("Local Queue Item", "Movie")];
    let remote_items = vec![make_item("Remote Queue Item", "Movie")];
    let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
    let mut app = App::new_remote(
        EmbyClient::new(Config::default()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Queue;
    app.queue_scope = QueueScope::Remote;
    app.player_tab.set_items(local_items, 0);
    app
}

pub fn make_music_group_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.music_levels = vec!["group".into(), "album".into()];

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();

    let group_names = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"];
    let groups: Vec<EmbyItem> = group_names
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
                music_grouping: None,
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
                music_grouping: None,
            },
        ],
        ..LibraryTab::new(library)
    });

    app
}

/// Builds on `make_music_group_app` by adding a second sibling album
/// ("Second Album", also by "Alpha") to the same nav level. Shared by the
/// cache-miss/loading and cache-hit/rendered inline-detail tests, which
/// both need a following album to assert framing around the selected one.
pub fn make_music_group_app_with_second_album() -> App {
    let mut app = make_music_group_app();
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .items
        .push(second_album);
    app
}

/// Shared row-map assertions for the inline album-detail tests: the
/// selected album's inline detail (loading indicator or rendered tracks)
/// must sit between the selected album title and the following sibling
/// album, and every row in between must be non-selectable.
pub fn assert_inline_detail_frames_between_albums(
    lines: &[&str],
    layout: &LayoutMain,
    title_y: usize,
    detail_y: usize,
) {
    assert!(
        lines[title_y - 4].trim().is_empty(),
        "expected the colored top-padding row above the artist header to be blank:\n{}",
        lines.join("\n")
    );
    assert_eq!(
        lines.iter().filter(|line| line.trim() == "Alpha").count(),
        1,
        "plain album framing must not duplicate the artist name:\n{}",
        lines.join("\n")
    );
    assert!(
        detail_y > title_y,
        "expected the inline detail row to render below the selected album title:\n{}",
        lines.join("\n")
    );

    let second_album_y = lines
        .iter()
        .position(|l| l.contains("Second Album"))
        .expect("expected the following album row");
    assert!(
        second_album_y > detail_y,
        "expected the inline detail to render before sibling albums:\n{}",
        lines.join("\n")
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
        "expected every row between the two albums (borders, padding, detail) to be non-selectable:\n{:?}",
        layout.left_row_map
    );
}

pub fn make_home_video_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Home Videos", "CollectionFolder");
    library.id = "lib-homevideos".into();
    library.is_folder = true;
    library.collection_type = "homevideos".into();

    let mut first = make_item("Birthday Clip", "Video");
    first.id = "video-1".into();
    let mut second = make_item("Vacation Clip", "Video");
    second.id = "video-2".into();

    app.libs.push(LibraryTab {
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
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    app
}

pub fn make_large_movie_library_app(library_total: usize) -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    app.libs.push(LibraryTab {
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
            music_grouping: None,
        }],
        library_total: Some(library_total),
        ..LibraryTab::new(library)
    });

    app
}
