use super::test_helpers::{
    buffer_to_string, make_audiobookshelf_book_app, make_movie_app, make_music_group_app,
    render_home_shell_with,
};
use super::*;
use crate::app::components::audiobookshelf_book::AudiobookshelfBookComponent;
use crate::app::components::feeds_content::{FeedsContent, FeedsOwnerPush};
use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::{
    AudiobookshelfPodcastComponent, BrowserComponent, ComponentId, MusicWorkspaceComponent,
};
use crate::app::layout::LayoutMain;
use crate::app::tests::make_item;
use crate::app::{PanelFocus, SeriesDetail, TabSelection};
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::{FeedEntry, QueueItem};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::collections::HashMap;
use tuirealm::component::Component;

fn render_library(app: &mut App, width: u16, height: u16) -> (Terminal<TestBackend>, LayoutMain) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = LayoutMain::default();
    terminal
        .draw(|frame| {
            app.render_library(frame, Rect::new(0, 0, width, height), &mut layout, None);
        })
        .unwrap();
    (terminal, layout)
}

/// Render an Emby browse surface (Movies / TV / grouped Music) through the real
/// `Model::draw_frame` shell path — the mounted `BrowserComponent` /
/// `MusicWorkspaceComponent` is the sole painter after task 3.8 — and surface
/// the active component's own painted geometry as a `LayoutMain` so the shared
/// conformance assertions still hold (mirrors `render_music_component`).
fn render_browse_component(
    mut app: App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, LayoutMain) {
    app.terminal_width = width;
    app.terminal_height = height;
    app.mini_view_focus = PanelFocus::Library;
    let seed_cursor = app.libs[0]
        .nav_stack
        .last()
        .map_or(0, |level| level.resting().cursor());
    let mut model = crate::app::shell::Model::new(app);
    model.sync_mounted_surfaces();
    // Generic/Movies/HomeVideos (task 6.1) route through the embedded
    // `BrowserContent` owner inside the mounted `LibraryPanel` instead of a
    // mounted `BrowserComponent`; seed its cursor there when that owner is
    // the active one.
    if let Some((_, key, _)) = model.active_migrated_browser_owner() {
        if let Some(owner) = model
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(|panel| panel.owner_mut(&key))
            .and_then(|owner| {
                owner
                    .as_any_mut()
                    .downcast_mut::<crate::app::components::browser_content::BrowserContent>()
            })
        {
            owner.set_cursor_for_test(seed_cursor);
        }
    } else if let Some(id) = model.emby_browser_id.clone() {
        if let Some(browser) = model
            .application
            .get_component_mut(&id)
            .and_then(|component| component.as_any_mut().downcast_mut::<BrowserComponent>())
        {
            browser.set_cursor_for_test(seed_cursor);
        }
    }
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    let list_area = model.app.layout.main.left_area;
    let layout = if model.active_library_owner_migrated() {
        panel_browse_layout(&model)
    } else {
        let painted = if model.music_workspace_id.is_some() {
            super::test_helpers::mounted_music_layout(&model)
        } else if model.tv_workspace_id.is_some() {
            super::test_helpers::mounted_tv_layout(&model)
        } else {
            super::test_helpers::mounted_browser_layout(&model)
        };
        LayoutMain {
            left_area: if painted.left_area.width > 0 {
                painted.left_area
            } else {
                list_area
            },
            hero_area: painted.hero_area,
            selected_item_rect: painted.selected_item_rect,
            selector_tabs: painted.selector_tabs.clone(),
            ..Default::default()
        }
    };
    (terminal, layout)
}

/// The migrated `BrowserContent` owner's painted geometry, surfaced as a
/// `LayoutMain` so the shared conformance assertions still hold (mirrors
/// `render_browse_component`'s old-path shape).
fn panel_browse_layout(model: &crate::app::shell::Model) -> LayoutMain {
    let panel = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("Library panel mounted");
    let selector_tabs = panel.test_selector_hits().regions().to_vec();
    if let Some(wide) = panel.test_wide_geometry() {
        LayoutMain {
            left_area: wide.list_area,
            hero_area: wide.hero_area,
            selected_item_rect: wide.selected,
            selector_tabs,
            ..Default::default()
        }
    } else {
        let narrow = panel
            .test_narrow_geometry()
            .expect("the panel painted a Wide or Narrow skeleton");
        LayoutMain {
            left_area: narrow.list_area,
            hero_area: narrow.inline_hero.unwrap_or_default(),
            selected_item_rect: narrow.selected,
            selector_tabs,
            ..Default::default()
        }
    }
}

/// Render the Book surface through its mounted `AudiobookshelfBookComponent`
/// (task 5.3d.13, render ownership) instead of the legacy `render_library`, and
/// surface the component's geometry as a `LayoutMain` so the shared conformance
/// assertions still hold. The component paints the hero, rows, and pills; the
/// legacy `AppLayout` fields are reconstructed from `AudiobookshelfBookGeometry`.
fn render_book_component(
    app: &App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, LayoutMain) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let area = Rect::new(0, 0, width, height);
    let mut component = AudiobookshelfBookComponent::new();
    if let Some(state) = app.audiobookshelf_book_browse.first() {
        component.set_content(state, app.images_enabled());
        component.set_focused(true);
    }
    terminal.draw(|frame| component.view(frame, area)).unwrap();
    let geometry = component.geometry();
    let layout = LayoutMain {
        left_area: area,
        hero_area: geometry.hero_area.unwrap_or_default(),
        selected_item_rect: geometry.selected_item_rect,
        selector_tabs: geometry.selector_tabs.clone(),
        ..Default::default()
    };
    (terminal, layout)
}

fn render_podcast_component(
    app: &App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, LayoutMain) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let area = Rect::new(0, 0, width, height);
    let mut component = AudiobookshelfPodcastComponent::new();
    if let Some(state) = app.audiobookshelf_browse.first() {
        component.set_content(state, app.images_enabled());
        component.set_focused(true);
    }
    terminal.draw(|frame| component.view(frame, area)).unwrap();
    let geometry = component.geometry();
    let layout = LayoutMain {
        left_area: area,
        hero_area: geometry.hero_area,
        inline_hero_area: geometry.inline_hero_area,
        selected_item_rect: geometry.selected_item_rect,
        selector_tabs: geometry.selector_tabs.clone(),
        ..Default::default()
    };
    (terminal, layout)
}

/// Render the wide grouped Music workspace through its mounted
/// `MusicWorkspaceComponent` (the sole wide-music painter, #613) instead of
/// the legacy `render_library`, surfacing the component's own painted pill
/// geometry as a `LayoutMain` so the shared conformance assertions still hold.
fn render_music_component(
    app: &App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, LayoutMain) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let area = Rect::new(0, 0, width, height);
    let lib_idx = app.tab.emby_library_index().unwrap();
    let context = app.wide_music_render_ctx(lib_idx, None);
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context);
    terminal.draw(|frame| component.view(frame, area)).unwrap();
    let painted = component.layout();
    let layout = LayoutMain {
        left_area: area,
        hero_area: painted.hero_area,
        selected_item_rect: painted.selected_item_rect,
        selector_tabs: painted.selector_tabs.clone(),
        ..Default::default()
    };
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

fn feed_owner() -> FeedsContent {
    let subscriptions = [FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![vec![FeedEntry {
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
    let all_entries = entries[0].clone();
    let mut owner = FeedsContent::new();
    owner.set_content(FeedsOwnerPush {
        subscriptions: subscriptions.to_vec(),
        entries,
        all_entries,
        loading: false,
    });
    owner
}

/// The embedded `FeedsContent` owner's painted geometry through the mounted
/// `LibraryPanel` (task 7.3), surfaced as a `LayoutMain` so the shared
/// conformance assertions still hold.
fn render_feeds_panel(
    owner: FeedsContent,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, LayoutMain) {
    let mut panel = LibraryPanel::new();
    panel.insert_owner(LibraryKey::Feeds, Box::new(owner));
    panel.set_active(Some(LibraryKey::Feeds));
    tuirealm::component::Component::attr(
        &mut panel,
        tuirealm::props::Attribute::Focus,
        tuirealm::props::AttrValue::Flag(true),
    );
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, width, height)))
        .unwrap();
    let selector_tabs = panel.test_selector_hits().regions().to_vec();
    let layout = if let Some(wide) = panel.test_wide_geometry() {
        LayoutMain {
            left_area: wide.list_area,
            hero_area: wide.hero_area,
            selected_item_rect: wide.selected,
            selector_tabs,
            ..Default::default()
        }
    } else {
        let narrow = panel
            .test_narrow_geometry()
            .expect("the panel painted a Wide or Narrow skeleton");
        LayoutMain {
            left_area: narrow.list_area,
            hero_area: narrow.inline_hero.unwrap_or_default(),
            selected_item_rect: narrow.selected,
            selector_tabs,
            ..Default::default()
        }
    };
    (terminal, layout)
}

fn mixed_home_app() -> App {
    let mut app = crate::app::tests::make_app_stub();
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    // Select the Books pill (section 1) through the real pending-source
    // boundary so `render_home_shell_with`'s `push_home_content` restores it
    // (task 5.3d, numeric Home section deletion). The pill data itself is
    // Model-owned `home_content.latest` (task 5.3d), seeded by
    // `mixed_home_latest()` at the render call.
    app
}

/// The mixed Books+Feeds pill data the Home-characterization seeds into
/// Model-owned `home_content.latest` (task 5.3d).
fn mixed_home_latest() -> Vec<(
    String,
    crate::app::types_playback::HomeLatestSource,
    Vec<QueueItem>,
)> {
    vec![
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
        ),
    ]
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
        ("TV", series_app(), "Movie"),
        ("Music", make_music_group_app(), "First Album"),
        (
            "Podcasts",
            crate::app::tests_podcast::audiobookshelf_app(),
            "Show A",
        ),
        ("Books", make_audiobookshelf_book_app(), "Alpha Tales"),
    ];

    for (surface, app, title) in cases {
        // Books/Podcasts drive their component directly with the full rect;
        // Movies/TV/Music route through `Model::draw_frame`, which reserves
        // chrome, so give them a slightly taller terminal whose *content* area
        // is still far shorter than any hero needs.
        let (terminal, layout) = if surface == "Books" {
            render_book_component(&app, 60, 4)
        } else if surface == "Podcasts" {
            render_podcast_component(&app, 60, 4)
        } else {
            render_browse_component(app, 60, 12)
        };
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
    let mut music = make_music_group_app();
    music.libs[0].nav_stack[1].set_resting_cursor(0);
    let cases = [
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

    for (surface, app, title) in cases {
        let (terminal, layout) = if surface == "Books" {
            render_book_component(&app, 70, 30)
        } else if surface == "Podcasts" {
            render_podcast_component(&app, 70, 30)
        } else {
            render_browse_component(app, 70, 30)
        };
        let output = buffer_to_string(&terminal);
        assert!(
            layout.hero_area.height > 0,
            "{surface} bottom selection should admit a hero:\n{output}"
        );
        let selected_rect = layout
            .selected_item_rect
            .expect("selected item keeps a parent-owned target");
        assert_eq!(selected_rect.x, layout.hero_area.x);
        assert_eq!(selected_rect.y, layout.hero_area.y);
        assert_eq!(selected_rect.width, layout.hero_area.width);
        assert!(selected_rect.height > 0);
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

/// Task 5.3d + 5.11, Home as the first panel owner: the Home leg of the
/// pill-bar conformance reads the mounted `LibraryPanel`'s own retained
/// `SkeletonHits.selector` (the single painter) — the same painted-truth
/// contract the deleted `HomeComponent::pill_targets` served. The assertions
/// mirror `assert_one_pill_row_and_spacer` exactly for that surface: targets
/// share one row, exactly one pill bar is painted, and the pill spacer stays
/// consistent below it.
fn assert_home_one_pill_row_and_spacer(
    model: &crate::app::shell::Model,
    terminal: &Terminal<TestBackend>,
) {
    let targets = model
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        .expect("Library panel type")
        .test_selector_hits()
        .regions()
        .to_vec();
    let first = targets
        .first()
        .unwrap_or_else(|| panic!("Home should publish pill targets"))
        .0;
    assert!(
        targets
            .iter()
            .all(|(rect, _)| rect.y == first.y && rect.height == 1),
        "pill targets must share one row: {:?}",
        targets
    );

    let buffer = terminal.backend().buffer();
    let painted_rows = (0..buffer.area().height)
        .filter(|y| {
            targets.iter().all(|(rect, _)| {
                buffer[(rect.x, *y)].symbol() == "◢"
                    && buffer[(rect.right() - 1, *y)].symbol() == "◤"
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        painted_rows,
        vec![first.y],
        "Home should paint exactly one pill bar"
    );

    let last = targets.last().unwrap().0;
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

    for (surface, app, width) in cases {
        let (terminal, layout) = if surface == "Books" {
            render_book_component(&app, width, 30)
        } else if surface == "Podcasts" {
            render_podcast_component(&app, width, 30)
        } else if surface == "Music" {
            render_music_component(&app, width, 30)
        } else {
            render_browse_component(app, width, 30)
        };
        assert_one_pill_row_and_spacer(surface, &terminal, &layout);
        assert!(
            !buffer_to_string(&terminal).is_empty(),
            "{surface} did not paint a buffer"
        );
    }

    // Home (task 5.3d, legacy underpaint removal) renders through the
    // mounted component; assert its pill bar from the component's own painted
    // targets. The pill data is Model-owned `home_content.latest` (5.3d).
    let (model, terminal) = render_home_shell_with(mixed_home_app(), 60, 30, |m| {
        m.home_section_pending =
            Some(crate::app::types_playback::HomeLatestSource::Audiobookshelf("books".into()));
        m.home_content.latest = mixed_home_latest();
    });
    assert_home_one_pill_row_and_spacer(&model, &terminal);
    assert!(
        !buffer_to_string(&terminal).is_empty(),
        "Home did not paint a buffer"
    );

    let (terminal, layout) = render_feeds_panel(feed_owner(), 60, 30);
    assert_one_pill_row_and_spacer("Feeds", &terminal, &layout);
    assert!(
        !buffer_to_string(&terminal).is_empty(),
        "Feeds did not paint a buffer"
    );
}

#[test]
fn matrix_mini_presentations_do_not_admit_a_full_hero() {
    let cases = vec![
        ("Movies", make_movie_app()),
        ("TV", series_app()),
        ("Music", make_music_group_app()),
        ("Podcasts", crate::app::tests_podcast::audiobookshelf_app()),
        ("Books", make_audiobookshelf_book_app()),
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

    // Home (task 5.3d + 5.11) is painted by the mounted `LibraryPanel`, so
    // its mini-view hero is asserted from the panel's own geometry: the tiny
    // viewport admits no inline hero. The pill data is Model-owned
    // `home_content.latest` (5.3d).
    let (model, _terminal) = render_home_shell_with(mixed_home_app(), 60, 8, |m| {
        m.home_section_pending =
            Some(crate::app::types_playback::HomeLatestSource::Audiobookshelf("books".into()));
        m.home_content.latest = mixed_home_latest();
    });
    let admitted = model
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        .expect("Library panel type")
        .test_narrow_geometry()
        .and_then(|geo| geo.inline_hero);
    assert_eq!(
        admitted, None,
        "Home mini presentation should not admit a hero"
    );
}

/// P1 regression for 5.3d.13 Unit A: the legacy base frame must populate
/// `layout.main.audiobookshelf_book_area` for the Book tab, or the shell's
/// `render_audiobookshelf_book_component` early-returns on the zero Rect and
/// the live Book surface stays blank (the component overlay reads that rect).
/// Exercises the real base-frame path (`App::render_library` ->
/// `render_audiobookshelf_library`), not a direct component render.
#[test]
fn render_library_sets_book_area_before_component_overlay() {
    let mut app = make_audiobookshelf_book_app();
    let (_, layout) = render_library(&mut app, 60, 20);
    assert_eq!(
        layout.audiobookshelf_book_area,
        Rect::new(0, 0, 60, 20),
        "base frame must populate audiobookshelf_book_area before the component overlay paints"
    );
}

/// `remove-migrated-surface-underpaint` 3.5 (D4): the mounted
/// `AudiobookshelfBookComponent` owns the Book picture at every breakpoint.
/// `render_audiobookshelf_library`
/// (`src/app/render/components/widgets.rs:599`) sets
/// `audiobookshelf_book_area` and returns without painting a book row, hero,
/// or pill. Mirrors the Home precedent
/// `legacy_base_frame_does_not_paint_home_content_before_the_component`.
#[test]
fn abs_book_legacy_base_frame_publishes_geometry_but_paints_no_books() {
    for (width, height) in [(60, 20), (120, 40)] {
        let mut app = make_audiobookshelf_book_app();
        let (terminal, layout) = render_library(&mut app, width, height);
        assert_eq!(
            layout.audiobookshelf_book_area,
            Rect::new(0, 0, width, height),
            "book geometry hand-off must stay reserved at {width}x{height}"
        );
        let output = buffer_to_string(&terminal);
        assert!(
            !output.contains("Alpha Tales") && !output.contains("◢"),
            "legacy base frame must not paint the Book surface at {width}x{height}: {output:?}"
        );
    }
}

/// `remove-migrated-surface-underpaint` 3.6 (D4): the mounted
/// `AudiobookshelfPodcastComponent` owns the Podcast picture. The podcast
/// case of `render_audiobookshelf_library`
/// (`src/app/render/components/widgets.rs:605`) only assigns
/// `audiobookshelf_podcast_area`; nothing else runs in the function, so no
/// show row, hero, or pill is painted.
#[test]
fn abs_podcast_legacy_base_frame_publishes_geometry_but_paints_no_shows() {
    for (width, height) in [(60, 20), (120, 40)] {
        let mut app = crate::app::tests_podcast::audiobookshelf_app();
        let (terminal, layout) = render_library(&mut app, width, height);
        assert_eq!(
            layout.audiobookshelf_podcast_area,
            Rect::new(0, 0, width, height),
            "podcast geometry hand-off must stay reserved at {width}x{height}"
        );
        let output = buffer_to_string(&terminal);
        assert!(
            !output.contains("Show A") && !output.contains("◢"),
            "legacy base frame must not paint the Podcast surface at {width}x{height}: {output:?}"
        );
    }
}

/// `remove-migrated-surface-underpaint` 3.7 (D4): the registered Feeds owner
/// inside the mounted `LibraryPanel` owns the Feeds picture (task 7.3). The
/// Feeds arm of `render_library` (`src/app/render/components/widgets.rs`)
/// reserves nothing and never delegates to `render_list`, so the legacy base
/// frame paints no feed entry, selector pill, or filter pill.
#[test]
fn feeds_legacy_base_frame_paints_no_entries() {
    for (width, height) in [(60, 20), (140, 30)] {
        let mut app = feed_app();
        let (terminal, _layout) = render_library(&mut app, width, height);
        let output = buffer_to_string(&terminal);
        assert!(
            !output.contains("Entry One") && !output.contains("Test Feed") && !output.contains("◢"),
            "legacy base frame must not paint the Feeds surface at {width}x{height}: {output:?}"
        );
    }
}

/// `remove-migrated-surface-underpaint` 3.7 (D4): the mounted `QueueComponent`
/// owns the queue slot rows. The queue legacy base frame only publishes
/// geometry and never paints the slot rows.
#[test]
fn queue_legacy_base_frame_reserves_geometry_but_paints_no_slot_rows() {
    for (width, height) in [(60, 20), (140, 30)] {
        let mut app = crate::app::tests::make_app_stub();
        app.player_tab.set_queue_items(
            crate::app::tests::make_items(2)
                .into_iter()
                .map(|item| QueueItem::Emby(Box::new(item)))
                .collect(),
            0,
        );
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| app.compose_base_frame(frame, None))
            .unwrap();
        assert!(
            app.queue_panel_placement().panel_area.width > 0,
            "the queue placement must reserve panel rows at {width}x{height}"
        );
        let output = buffer_to_string(&terminal);
        assert!(
            !output.contains("Item 0") && !output.contains("Item 1"),
            "QueueComponent must be the sole slot-row painter at {width}x{height}: {output:?}"
        );
    }
}
