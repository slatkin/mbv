use super::test_helpers::{
    buffer_to_string, make_movie_app, make_music_group_app, render_home_shell_with,
};
use super::*;
use crate::app::components::feeds_content::{FeedsContent, FeedsOwnerPush};
use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::ComponentId;
use crate::app::layout::PaintedRowGeometry;
use crate::app::tests::make_item;
use crate::app::{PanelFocus, SeriesDetail, TabSelection};
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::{FeedEntry, QueueItem};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::collections::HashMap;
use tuirealm::component::Component;

fn render_reserved_library_area(
    app: &mut App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, Rect) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = Rect::default();
    terminal
        .draw(|frame| {
            app.reserve_library_area(frame, Rect::new(0, 0, width, height), &mut layout, None);
        })
        .unwrap();
    (terminal, layout)
}

/// Render an Emby browse surface (Movies / TV / grouped Music) through the real
/// `Model::draw_frame` shell path — the embedded Emby/TV content owner /
/// `MusicWorkspaceComponent` is the sole painter after task 3.8 — and surface
/// the active component's own painted geometry as a `PaintedRowGeometry` so
/// the shared conformance assertions still hold (mirrors
/// `render_music_component`).
fn render_browse_component(
    mut app: App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, PaintedRowGeometry) {
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
    // `EmbyLibraryContent` owner inside the mounted `LibraryPanel` instead of a
    // mounted Emby library owner; seed its cursor there when that owner is
    // the active one.
    if let Some((_, key, _)) = model.active_emby_library_owner() {
        if let Some(owner) = model
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(|panel| panel.owner_mut(&key))
            .and_then(|owner| {
                owner
                    .as_any_mut()
                    .downcast_mut::<crate::app::components::emby_library_content::EmbyLibraryContent>()
            })
        {
            owner.set_cursor_for_test(seed_cursor);
        }
    }
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    let layout = panel_browse_layout(&model);
    (terminal, layout)
}

/// The migrated `EmbyLibraryContent` owner's painted geometry, surfaced as a
/// `PaintedRowGeometry` so the shared conformance assertions still hold
/// (mirrors `render_browse_component`'s old-path shape).
fn panel_browse_layout(model: &crate::app::shell::Model) -> PaintedRowGeometry {
    let panel = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("Library panel mounted");
    let selector_tabs = panel.test_selector_hits().regions().to_vec();
    if let Some(wide) = panel.test_wide_geometry() {
        PaintedRowGeometry {
            left_area: wide.list_area,
            selected_item_rect: wide.selected,
            selector_tabs,
        }
    } else {
        let narrow = panel
            .test_narrow_geometry()
            .expect("the panel painted a Wide or Narrow skeleton");
        PaintedRowGeometry {
            left_area: narrow.list_area,
            selected_item_rect: narrow.selected,
            selector_tabs,
        }
    }
}

/// Render grouped Music through its LibraryPanel owner and shared skeleton.
fn render_music_component(
    app: App,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, PaintedRowGeometry) {
    let mut model = crate::app::shell::Model::new(app);
    model.sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    let layout = panel_browse_layout(&model);
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
/// `LibraryPanel` (task 7.3), surfaced as a `PaintedRowGeometry` so the
/// shared conformance assertions still hold.
fn render_feeds_panel(
    owner: FeedsContent,
    width: u16,
    height: u16,
) -> (Terminal<TestBackend>, PaintedRowGeometry) {
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
        PaintedRowGeometry {
            left_area: wide.list_area,
            selected_item_rect: wide.selected,
            selector_tabs,
        }
    } else {
        let narrow = panel
            .test_narrow_geometry()
            .expect("the panel painted a Wide or Narrow skeleton");
        PaintedRowGeometry {
            left_area: narrow.list_area,
            selected_item_rect: narrow.selected,
            selector_tabs,
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
    layout: &PaintedRowGeometry,
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
    ];

    for (surface, app, width) in cases {
        let (terminal, layout) = if surface == "Music" {
            render_music_component(app, width, 30)
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

/// `remove-migrated-surface-underpaint` 3.7 (D4): the registered Feeds owner
/// inside the mounted `LibraryPanel` owns the Feeds picture (task 7.3). The
/// Feeds arm of `render_library` (`src/app/render/components/widgets.rs`)
/// reserves nothing and never delegates to `render_list`, so the legacy base
/// frame paints no feed entry, selector pill, or filter pill.
#[test]
fn feeds_legacy_base_frame_paints_no_entries() {
    for (width, height) in [(60, 20), (140, 30)] {
        let mut app = feed_app();
        let (terminal, _layout) = render_reserved_library_area(&mut app, width, height);
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
            .draw(|frame| app.compose_root_frame(frame))
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
