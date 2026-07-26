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
