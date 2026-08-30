#![allow(dead_code, unused_imports)]

use super::screens::album_plan::GroupedAlbumDisplayRow;
use super::*;
use crate::app::components::{BrowserComponent, MusicWorkspaceComponent, TvWorkspaceComponent};
use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
use crate::app::shell::Model;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::types_audiobookshelf_browse::{
    build_surname_buckets, AudiobookshelfBookBrowseState,
};
use crate::app::{App, PanelFocus};
use crate::app::{BrowseLevel, LibraryTab, QueueScope, RemoteSlotState, TabSelection};
use crate::config::Config;
use mbv_core::api::EmbyClient;
use mbv_core::api::EmbyItem;
use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfChapter, AudiobookshelfLibrary};
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
    let mut model = crate::app::shell::Model::new(std::mem::replace(app, make_app_stub()));
    model.sync_mounted_surfaces();
    term.draw(|f| {
        model
            .app
            .render_library(f, Rect::new(0, 0, 60, 20), focused, layout, None);
        model.render_emby_browser_component(f);
        model.render_music_workspace_component(f);
    })
    .unwrap();
    *app = model.app;
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
    let mut model = crate::app::shell::Model::new(std::mem::replace(app, make_app_stub()));
    model.sync_mounted_surfaces();
    term.draw(|f| {
        model
            .app
            .render_library(f, Rect::new(0, 0, width, height), true, layout, None);
        model.render_emby_browser_component(f);
        model.render_music_workspace_component(f);
    })
    .unwrap();
    *app = model.app;
    buffer_to_string(&term)
}

/// Build a `Model` at an explicit terminal size with the library pane focused.
/// Characterization tests whose surface is now painted by a mounted component
/// (`BrowserComponent` / `MusicWorkspaceComponent` / `TvWorkspaceComponent`)
/// instead of the legacy `render_library` arm start here, then draw with
/// `draw_mounted_frame` and read geometry via `mounted_*_layout`.
pub fn mounted_model_at(mut app: App, width: u16, height: u16) -> Model {
    app.terminal_width = width;
    app.terminal_height = height;
    app.mini_view_focus = PanelFocus::Library;
    Model::new(app)
}

/// Draw one full frame through `Model::draw_frame` (the live shell paint path)
/// after re-syncing mounted surfaces, and return the painted buffer text.
pub fn draw_mounted_frame(model: &mut Model, width: u16, height: u16) -> String {
    model.sync_mounted_surfaces();
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    buffer_to_string(&term)
}

/// The mounted Emby `BrowserComponent`'s own painted geometry (task 3.8: the
/// legacy `render_library` `EmbyLibrary` arm no longer publishes it).
pub fn mounted_browser_layout(model: &Model) -> &LayoutMain {
    let id = model
        .emby_browser_id
        .as_ref()
        .expect("emby browser component mounted");
    model
        .application
        .get_component(id)
        .expect("emby browser mounted")
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .expect("BrowserComponent")
        .test_layout()
}

/// The scroll offset the mounted Emby `BrowserComponent` settled on this frame
/// (task 3.8: the browser owns the persisted flow offset the legacy renderer
/// used to write back into the `BrowseLevel`).
pub fn mounted_browser_scroll(model: &Model) -> usize {
    let id = model
        .emby_browser_id
        .as_ref()
        .expect("emby browser component mounted");
    model
        .application
        .get_component(id)
        .expect("emby browser mounted")
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .expect("BrowserComponent")
        .scroll()
}

/// The mounted `MusicWorkspaceComponent`'s own painted geometry.
pub fn mounted_music_layout(model: &Model) -> &LayoutMain {
    let id = model
        .music_workspace_id
        .as_ref()
        .expect("music workspace component mounted");
    model
        .application
        .get_component(id)
        .expect("music workspace mounted")
        .as_any()
        .downcast_ref::<MusicWorkspaceComponent>()
        .expect("MusicWorkspaceComponent")
        .layout()
}

/// The mounted `TvWorkspaceComponent`'s own painted geometry.
pub fn mounted_tv_layout(model: &Model) -> &LayoutMain {
    let id = model
        .tv_workspace_id
        .as_ref()
        .expect("tv workspace component mounted");
    model
        .application
        .get_component(id)
        .expect("tv workspace mounted")
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .expect("TvWorkspaceComponent")
        .test_layout()
}

pub fn render_view_to_terminal(
    app: &mut App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, LayoutMain) {
    // Mirror App::render(), which syncs terminal_width from the drawn Rect
    // before render_main runs -- without this, effective_panel_mode()/
    // effective_panel_focus() see whatever width the app was constructed
    // with instead of the width this call is actually rendering at. Only
    // terminal_width is touched here (the historical helper contract): the
    // terminal-normalization side effects of `compute_frame_layout` (image
    // cache clears, mini-view focus, queue-column clamping, terminal_height)
    // would change card reservation geometry for tests that render a view
    // at a different height than the stub default.
    app.terminal_width = width;
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        // Root/chrome geometry comes from the same authoritative paint-free
        // computation the live seam uses (task 2.1a).
        let chrome = app.compute_chrome_geometry(Rect::new(0, 0, width, height));
        layout.panel_area = chrome.panel_area;
        layout.panel_content_area = chrome.panel_content_area;
        app.render_main(
            f,
            Rect::new(0, 0, width, height),
            &chrome,
            &mut layout,
            &mut LayoutPlayback::default(),
            0,
            false,
            &None,
            None,
        );
    })
    .unwrap();
    (term, layout)
}

pub fn render_app_to_terminal(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| app.compose_base_frame(f, None)).unwrap();
    term
}

/// Render the Home destination exactly as the live shell does (task 5.3d,
/// Home legacy underpaint removal): draw the legacy `App::render` base frame
/// — which for Home now only reserves `home_area` — then paint the mounted
/// `HomeComponent` through the real `Model::render_home_component` shell
/// path (which sizes the component by `home_area` and paints the cover image
/// it returned). Returns the model, so tests can read the component's own
/// painted geometry and App state, together with the terminal. This is the
/// Home characterization path once the legacy underpaint is gone.
///
/// Home content is Model-owned (task 5.3d), so a test that needs seeded
/// Continue Watching rows/pills uses `render_home_shell_with` and seeds
/// `model.home_content` before the push.
pub fn render_home_shell(
    app: App,
    width: u16,
    height: u16,
) -> (crate::app::shell::Model, Terminal<TestBackend>) {
    render_home_shell_with(app, width, height, |_| {})
}

/// `render_home_shell` with a content-seeding callback: the test seeds
/// Model-owned `home_content` (task 5.3d) right after `Model::new` and
/// before `push_home_content` projects it into the mounted `HomeComponent`.
pub fn render_home_shell_with(
    mut app: App,
    width: u16,
    height: u16,
    seed: impl FnOnce(&mut crate::app::shell::Model),
) -> (crate::app::shell::Model, Terminal<TestBackend>) {
    app.terminal_width = width;
    app.terminal_height = height;
    let mut model = crate::app::shell::Model::new(app);
    seed(&mut model);
    model.push_home_content();
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| {
        model.app.compose_base_frame(f, None);
        model.render_home_component(f);
    })
    .unwrap();
    (model, term)
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

/// Book surface app for conformance tests (moved here from the deleted
/// `tests_audiobookshelf_books.rs` legacy-renderer suite, task 5.3d.13). Three
/// books span three surname buckets (Adams -> A-C, Mason -> J-L, Zephyr ->
/// V-Z), so the A-C bucket is selected by default and only "Alpha Tales" is in
/// range.
pub(super) fn make_audiobookshelf_book_app() -> App {
    let mut app = make_app_stub();
    let library = AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    };
    let mut state = AudiobookshelfBookBrowseState::new(library.clone());
    state.append_page_books(
        0,
        3,
        vec![
            AudiobookshelfBook {
                library_item_id: "book-a".into(),
                title: "Alpha Tales".into(),
                author_display: Some("Adams".into()),
                author_sort_key: "Adams".into(),
                cover_path: None,
                duration_seconds: 0.0,
                narrator: None,
                published_year: None,
                genres: Vec::new(),
                description: None,
                series_name: None,
                chapters: Vec::new(),
                audio_files: Vec::new(),
            },
            AudiobookshelfBook {
                library_item_id: "book-m".into(),
                title: "Middle Ground".into(),
                author_display: Some("Mason".into()),
                author_sort_key: "Mason".into(),
                cover_path: None,
                duration_seconds: 0.0,
                narrator: None,
                published_year: None,
                genres: Vec::new(),
                description: None,
                series_name: None,
                chapters: Vec::new(),
                audio_files: Vec::new(),
            },
            AudiobookshelfBook {
                library_item_id: "book-z".into(),
                title: "Zenith Story".into(),
                author_display: Some("Zephyr".into()),
                author_sort_key: "Zephyr".into(),
                cover_path: None,
                duration_seconds: 0.0,
                narrator: None,
                published_year: None,
                genres: Vec::new(),
                description: None,
                series_name: None,
                chapters: Vec::new(),
                audio_files: Vec::new(),
            },
        ],
    );
    state.detail_cache.insert(
        "book-a".into(),
        (
            vec![AudiobookshelfChapter {
                id: 0,
                start: 0.0,
                end: 60.0,
                title: "Chapter One".into(),
            }],
            Vec::new(),
        ),
    );
    app.audiobookshelf_libraries.push(library);
    app.audiobookshelf_book_browse.push(state);
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app
}
