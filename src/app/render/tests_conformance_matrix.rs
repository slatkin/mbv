use super::test_helpers::{
    buffer_to_string, make_movie_app, make_music_group_app, render_view_to_terminal,
};
use super::*;
use crate::app::layout::LayoutMain;
use crate::app::render::audiobookshelf_book_tests::make_audiobookshelf_book_app;
use crate::app::tests::make_item;
use crate::app::{PanelFocus, SeriesDetail, TabSelection};
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::{FeedEntry, QueueItem};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::collections::HashMap;

fn render_library(app: &mut App, width: u16, height: u16) -> (Terminal<TestBackend>, LayoutMain) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = LayoutMain::default();
    terminal
        .draw(|frame| {
            app.render_library(frame, Rect::new(0, 0, width, height), true, &mut layout);
        })
        .unwrap();
    (terminal, layout)
}

fn movie_with_pills() -> App {
    let mut app = make_movie_app();
    let items = &mut app.libs[0].nav_stack[0].items;
    items.extend((0..55).map(|index| {
        let letter = (b'A' + (index % 26) as u8) as char;
        let mut item = make_item(&format!("{letter} Movie {index:02}"), "Movie");
        item.id = format!("movie-{index}");
        item
    }));
    let item_count = items.len();
    app.libs[0].nav_stack[0].total_count = item_count;
    app.libs[0].library_total = Some(item_count);
    app
}

fn series_app() -> App {
    let mut app = movie_with_pills();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
        item.is_folder = true;
    }
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = make_item("Pilot", "Episode");
    episode.id = "episode-1".into();
    let mut episodes = HashMap::new();
    episodes.insert("season-1".into(), vec![episode]);
    app.series_detail_cache.insert(
        "movie-focused".into(),
        SeriesDetail {
            seasons: vec![season],
            episodes,
        },
    );
    app
}

fn podcast_app_with_bottom_selection() -> App {
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    let state = &mut app.audiobookshelf_browse[0];
    let template = state.shows[0].clone();
    state.shows.extend((0..4).map(|index| {
        let mut show = template.clone();
        show.library_item_id = format!("show-{index}");
        show.title = format!("Show {index}");
        show
    }));
    state.select(state.shows.len() - 1);
    app
}

fn feed_app() -> App {
    let mut app = crate::app::tests::make_app_stub();
    app.tab = TabSelection::Feeds;
    app.feed_tab.subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    app.feed_tab.entries = vec![vec![FeedEntry {
        guid: "entry-1".into(),
        title: "Entry One".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    }]];
    app.feed_tab.rebuild_all_entries();
    app.mini_view_focus = PanelFocus::Library;
    app
}

fn mixed_home_app() -> App {
    let mut app = crate::app::tests::make_app_stub();
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    app.home.latest = vec![
        (
            "Books".into(),
            crate::app::types_playback::HomeLatestSource::Audiobookshelf("books".into()),
            vec![QueueItem::AudiobookshelfBook(
                mbv_core::playback_queue::AudiobookshelfBookQueueItem {
                    library_item_id: "book-1".into(),
                    title: "Home Book".into(),
                    author: Some("Author".into()),
                    duration_ticks: None,
                    position_ticks: 0,
                    played: false,
                    is_finished: false,
                    cover_path: None,
                },
            )],
            0,
        ),
        (
            "Feeds".into(),
            crate::app::types_playback::HomeLatestSource::Feeds,
            vec![QueueItem::Feed(FeedEntry {
                guid: "home-feed".into(),
                title: "Home Feed".into(),
                enclosure_url: None,
                link: None,
                mime_type: None,
                duration_ticks: None,
                pub_date_secs: None,
                feed_kind: Some(FeedKind::Audio),
                feed_id: None,
                position_ticks: 0,
                played: false,
            })],
            0,
        ),
    ];
    app.home.section = 1;
    app.home.home_cursor = 0;
    app
}

fn assert_one_pill_row_and_spacer(
    surface: &str,
    terminal: &Terminal<TestBackend>,
    layout: &LayoutMain,
) {
    let first = layout
        .selector_tabs
        .first()
        .unwrap_or_else(|| panic!("{surface} should publish pill targets"))
        .0;
    assert!(
        layout
            .selector_tabs
            .iter()
            .all(|(rect, _)| rect.y == first.y && rect.height == 1),
        "pill targets must share one row: {:?}",
        layout.selector_tabs
    );

    let buffer = terminal.backend().buffer();
    let painted_rows = (0..buffer.area().height)
        .filter(|y| {
            layout.selector_tabs.iter().all(|(rect, _)| {
                buffer[(rect.x, *y)].symbol() == "◢"
                    && buffer[(rect.right() - 1, *y)].symbol() == "◤"
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        painted_rows,
        vec![first.y],
        "{surface} should paint exactly one pill bar"
    );

    let last = layout.selector_tabs.last().unwrap().0;
    assert!(first.bottom() < buffer.area().height);
    let spacer_bg = buffer[(first.x, first.y + 1)].style().bg;
    for x in first.x..last.right() {
        assert_eq!(
            buffer[(x, first.y + 1)].style().bg,
            spacer_bg,
            "pill spacer spilled at x={x}"
        );
    }
}

#[test]
fn matrix_cannot_fit_preserves_an_ordinary_selected_row() {
    let cases = [
        ("Movies", make_movie_app(), "Focused Movie"),
        ("TV", series_app(), "Focused Movie"),
        ("Music", make_music_group_app(), "First Album"),
        (
            "Podcasts",
            crate::app::tests_podcast::audiobookshelf_app(),
            "Show A",
        ),
        ("Books", make_audiobookshelf_book_app(), "Alpha Tales"),
    ];

    for (surface, mut app, title) in cases {
        let (terminal, layout) = render_library(&mut app, 60, 4);
        let output = buffer_to_string(&terminal);
        assert_eq!(
            layout.hero_area,
            Rect::default(),
            "{surface} must suppress a hero that cannot fit"
        );
        assert!(
            output.contains(title),
            "{surface} must retain the ordinary selected row:\n{output}"
        );
        assert_ne!(layout.selected_item_rect, Some(layout.hero_area));
    }
}

#[test]
fn matrix_bottom_selected_heroes_swallow_their_source_rows() {
    let mut movies = make_movie_app();
    let items = &mut movies.libs[0].nav_stack[0].items;
    items.extend((0..6).map(|index| make_item(&format!("Movie {index}"), "Movie")));
    let selected = items.len() - 1;
    items[selected].overview = "The selected movie overview.".into();
    movies.libs[0].nav_stack[0].cursor = selected;
    movies.libs[0].nav_stack[0].scroll = 1;
    let mut music = make_music_group_app();
    music.libs[0].nav_stack[1].cursor = 0;
    let cases = [
        ("Movies", movies, "Movie 5"),
        ("Music", music, "First Album"),
        ("Podcasts", podcast_app_with_bottom_selection(), "Show 3"),
        (
            "Books",
            {
                let mut app = make_audiobookshelf_book_app();
                let state = &mut app.audiobookshelf_book_browse[0];
                let mut companion = state.books[2].clone();
                companion.library_item_id = "book-z2".into();
                companion.title = "Zenith Companion".into();
                state.append_page_books(1, 4, vec![companion]);
                app.select_audiobookshelf_book_bucket(2);
                let state = &mut app.audiobookshelf_book_browse[0];
                state.select(state.books.len() - 1);
                app
            },
            "Zenith Companion",
        ),
    ];

    for (surface, mut app, title) in cases {
        let (terminal, layout) = render_library(&mut app, 70, 30);
        let output = buffer_to_string(&terminal);
        assert!(
            layout.hero_area.height > 0,
            "{surface} bottom selection should admit a hero:\n{output}"
        );
        assert_eq!(layout.selected_item_rect, Some(layout.hero_area));
        assert_eq!(
            output.matches(title).count(),
            1,
            "{surface} source row was not swallowed:\n{output}"
        );
        assert!(
            layout.hero_area.y > layout.left_area.y,
            "{surface} bottom hero should grow upward from a lower source row"
        );
    }
}

#[test]
fn matrix_all_surfaces_paint_one_pill_bar_with_one_parent_spacer() {
    let cases = [
        ("Movies", movie_with_pills(), 60),
        ("TV", series_app(), 60),
        ("Music", make_music_group_app(), 100),
        (
            "Podcasts",
            crate::app::tests_podcast::audiobookshelf_app(),
            60,
        ),
        ("Books", make_audiobookshelf_book_app(), 60),
    ];

    for (surface, mut app, width) in cases {
        let (terminal, layout) = render_library(&mut app, width, 30);
        assert_one_pill_row_and_spacer(surface, &terminal, &layout);
        assert!(
            !buffer_to_string(&terminal).is_empty(),
            "{surface} did not paint a buffer"
        );
    }

    for (surface, mut app) in [("Home", mixed_home_app()), ("Feeds", feed_app())] {
        let (terminal, layout) = render_view_to_terminal(&mut app, 60, 30);
        assert_one_pill_row_and_spacer(surface, &terminal, &layout);
        assert!(
            !buffer_to_string(&terminal).is_empty(),
            "{surface} did not paint a buffer"
        );
    }
}

#[test]
fn matrix_mini_presentations_do_not_admit_a_full_hero() {
    let cases = vec![
        ("Movies", make_movie_app()),
        ("TV", series_app()),
        ("Music", make_music_group_app()),
        ("Podcasts", crate::app::tests_podcast::audiobookshelf_app()),
        ("Books", make_audiobookshelf_book_app()),
        ("Home", mixed_home_app()),
        ("Feeds", feed_app()),
    ];
    for (surface, mut app) in cases {
        app.terminal_width = 60;
        app.terminal_height = 20;
        let _terminal = super::test_helpers::render_app_to_terminal(&mut app, 60, 8);
        assert_eq!(
            app.layout.main.hero_area.height, 0,
            "{surface} mini presentation should not admit a hero: {:?}",
            app.layout.main.hero_area
        );
    }
}
