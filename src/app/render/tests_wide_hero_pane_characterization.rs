//! Characterization tests (task 1.1, standardize-Wide-hero-pane): pin the
//! *current* Wide browser-pane output of all seven Wide hero destinations
//! before any paint/primitive change lands. These intentionally capture
//! today's drifted behaviour (the Home clamp, ABS Podcasts' missing fill, ABS
//! Books' foreground-only `.style(Color)` bug, Feeds' conditional fill) as-is
//! -- they are a baseline to diff phases 2/3 against, not a statement of
//! correct behaviour. Must land in its own commit before any Wide hero paint
//! or primitive change (ledger migration flow).

use super::test_helpers::{
    buffer_to_string, make_audiobookshelf_book_app, make_music_group_app,
    region_has_selection_marker,
};
use crate::app::components::{
    AudiobookshelfBookComponent, AudiobookshelfPodcastComponent, FeedsComponent, HomeComponent,
    MusicWorkspaceComponent, TvWorkspaceComponent,
};
use crate::app::palette;
use crate::app::render::arrangements::library::wide_library_panes;
use crate::app::render::arrangements::wide_hero::{PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::TvWideRenderCtx;
use crate::app::tests::make_item;
use crate::app::TWO_COLUMN_THRESHOLD;
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::{FeedEntry, QueueItem};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};

fn surface_fill(surface: palette::Surface, focused: bool) -> ratatui::style::Color {
    palette::surface_colors_for_column_focus(surface, focused).fill
}
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

const WIDTH: u16 = 100;
const HEIGHT: u16 = 30;

fn wide_area() -> Rect {
    Rect::new(0, 0, WIDTH, HEIGHT)
}

fn direct_terminal(mut draw: impl FnMut(&mut ratatui::Frame)) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    terminal.draw(|f| draw(f)).unwrap();
    terminal
}

/// TV already routes through `wide_library_panes(area, PANE_PAD_X,
/// PANE_PAD_Y)` and `resolve_surface_focus` -- the one destination the
/// standardization leaves visually unchanged (task 3.2).
#[test]
fn tv_wide_left_pane_unconditional_fill_shared_inset() {
    let mut component = TvWorkspaceComponent::new();
    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![make_item("Focused Series", "Series")], 0, 0),
        None,
        None,
        0,
        None,
        false,
    ));
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));
    let buffer = terminal.backend().buffer();

    let panes = wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y, None).expect("wide fits");
    let hero_panel = panes.hero_panel;

    assert_eq!(
        buffer[(hero_panel.x, hero_panel.y)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );
    assert_eq!(
        buffer[(hero_panel.x, hero_panel.bottom() - 1)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );
}

/// Music routes through `wide_library_panes(area, 0, PANE_PAD_Y)` --
/// no horizontal inset on the panel/pane split itself (task 3.3).
#[test]
fn music_wide_left_pane_unconditional_fill_no_horizontal_pad() {
    let app = make_music_group_app();
    let lib_idx = app.tab.emby_library_index().unwrap();
    let context = app.wide_music_render_ctx(lib_idx, None);
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context);
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));
    let buffer = terminal.backend().buffer();

    let panes = wide_library_panes(area, 0, PANE_PAD_Y, None).expect("wide fits");
    let hero_panel = panes.hero_panel;

    assert_eq!(
        buffer[(hero_panel.x, hero_panel.y)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );
    assert_eq!(
        buffer[(hero_panel.x, hero_panel.bottom() - 1)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );
}

/// Home's non-Emby Latest selection fills the complete hero pane. The
/// Audiobookshelf cover and metadata are top-anchored within that pane.
#[test]
fn home_wide_non_emby_latest_fills_the_full_hero_area() {
    let source = crate::app::types_playback::HomeLatestSource::Audiobookshelf("books".into());
    let latest = vec![(
        "Books".into(),
        source.clone(),
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
    )];
    let mut component = HomeComponent::new();
    component.set_content(Vec::new(), latest, false);
    component.set_focused(true);
    assert!(component.restore_section(&source), "Books pill must exist");
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));

    let hero = component.hero_area().expect("wide non-Emby hero pane");
    let buffer = terminal.backend().buffer();

    assert_eq!(
        buffer[(hero.x, hero.y)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );
    assert_eq!(
        buffer[(hero.x, hero.bottom() - 1)].bg,
        surface_fill(palette::Surface::HeroPane, false),
        "the full reported hero area must be filled"
    );
}

fn feed_component_with_entries(entries: Vec<FeedEntry>) -> FeedsComponent {
    let subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let grouped = vec![entries];
    let all_entries = grouped[0].clone();
    let mut component = FeedsComponent::new();
    component.set_content(&subscriptions, &grouped, &all_entries, false);
    component.set_focused(true);
    component
}

fn feed_entry(guid: &str, title: &str) -> FeedEntry {
    FeedEntry {
        guid: guid.into(),
        title: title.into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

/// Feeds with a selected entry: the fill and detail both paint (task 2.3's
/// starting point).
#[test]
fn feeds_wide_left_pane_fills_when_an_entry_is_selected() {
    let mut component = feed_component_with_entries(vec![feed_entry("entry-1", "Entry One")]);
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));
    let hero = component.layout().hero_area;
    assert!(hero.width > 0 && hero.height > 0);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(hero.x, hero.y)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );
    assert_eq!(
        buffer[(hero.x, hero.bottom() - 1)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );
}

/// Feeds (task 2.3): the wide right hero pane fill is unconditional (D1) -- with
/// entries present but nothing selected, the pane still fills
/// `SURFACE_RESTING` (D3: read-only, never focus-green), with no hero
/// content painted. `render_feeds_content` is called directly with
/// `selected_entry: None` since the component's own cursor always resolves
/// to an entry once entries exist.
#[test]
fn feeds_wide_left_pane_fills_unconditionally_with_no_selection() {
    use crate::app::components::media_list::WideMediaList;
    use crate::app::layout::LayoutMain;
    use crate::app::render::{render_feeds_content, FeedsPresentation, FeedsRenderModel};
    use crate::app::types_feed_tab::WatchedFilter;

    let subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![feed_entry("entry-1", "Entry One")];
    let mut layout = LayoutMain::default();
    let mut canonical_list: WideMediaList<String> = WideMediaList::new();
    let area = wide_area();
    let terminal = direct_terminal(|f| {
        render_feeds_content(
            f,
            area,
            false,
            &mut layout,
            FeedsRenderModel {
                subscriptions: &subscriptions,
                visible_entries: &entries,
                watched_filter: WatchedFilter::All,
                selected_group: 0,
                loading: false,
                selected_entry: None,
                images_enabled: true,
            },
            FeedsPresentation::Wide(&mut canonical_list),
            None,
        );
    });
    let hero = layout.hero_area;
    assert!(hero.width > 0 && hero.height > 0, "hero={hero:?}");
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(hero.x, hero.y)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );
    assert_eq!(
        buffer[(hero.x, hero.bottom() - 1)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );

    let mut focused_layout = LayoutMain::default();
    let mut focused_list: WideMediaList<String> = WideMediaList::new();
    let focused_terminal = direct_terminal(|f| {
        render_feeds_content(
            f,
            area,
            true,
            &mut focused_layout,
            FeedsRenderModel {
                subscriptions: &subscriptions,
                visible_entries: &entries,
                watched_filter: WatchedFilter::All,
                selected_group: 0,
                loading: false,
                selected_entry: None,
                images_enabled: true,
            },
            FeedsPresentation::Wide(&mut focused_list),
            None,
        );
    });
    let focused_hero = focused_layout.hero_area;
    let focused_buffer = focused_terminal.backend().buffer();
    assert_eq!(
        focused_buffer[(focused_hero.x, focused_hero.y)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );
    assert_eq!(
        focused_buffer[(focused_hero.x, focused_hero.bottom() - 1)].bg,
        surface_fill(palette::Surface::HeroPane, false)
    );
}

/// Feeds with no entries to select: `feeds.rs:170-184` returns before the
/// Wide hero pane is ever reached (a placeholder message paints instead),
/// so no `hero_area` is published at all -- the broken empty-selection state
/// task 2.3 fixes (D1: an unconditional pane fill even with nothing
/// selected).
#[test]
fn feeds_wide_left_pane_unfilled_with_no_selected_entry() {
    let mut component = feed_component_with_entries(vec![]);
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));
    let hero = component.layout().hero_area;
    assert_eq!(
        hero,
        Rect::default(),
        "characterizes the pre-fix state: no hero pane is published with no entries"
    );
    let output = buffer_to_string(&terminal);
    assert!(
        output.contains("Press r to load feeds"),
        "output={output:?}"
    );
}

/// ABS Books (task 2.2): the `.style(Color)` foreground-only bug is fixed --
/// the wide right hero pane is filled via `wide_hero_hero_pane`, focus-green
/// (`LeftPaneFocus::Workspace`) whenever the library column holds focus.
#[test]
fn abs_books_wide_left_pane_fills_via_shared_primitive() {
    let app = make_audiobookshelf_book_app();
    let mut component = AudiobookshelfBookComponent::new();
    if let Some(state) = app.audiobookshelf_book_browse.first() {
        component.set_content(state, app.images_enabled());
        component.set_focused(true);
    }
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));
    let geometry = component.geometry();
    assert!(geometry.wide);
    let panes = wide_library_panes(area, 0, PANE_PAD_Y, None).expect("wide fits");
    let hero_panel = panes.hero_panel;
    let buffer = terminal.backend().buffer();
    // The library column holds focus, so the pane is focus-green from the
    // start. Before `unify-surface-colour` 3.2 this was `SURFACE_RESTING`
    // while no chapter was selected: the per-screen `chapter_focused` bit no
    // longer chooses the pane fill.
    assert_eq!(
        buffer[(hero_panel.x, hero_panel.y)].bg,
        surface_fill(palette::Surface::HeroPane, true)
    );
    assert_eq!(
        buffer[(hero_panel.x, hero_panel.bottom() - 1)].bg,
        surface_fill(palette::Surface::HeroPane, true)
    );

    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Left,
        modifiers: KeyModifiers::NONE,
    }));
    let focused_terminal = direct_terminal(|f| component.view(f, area));
    let focused_buffer = focused_terminal.backend().buffer();
    // Moving the cursor into the chapter pane changes no fill.
    assert_eq!(
        focused_buffer[(hero_panel.x, hero_panel.y)].bg,
        surface_fill(palette::Surface::HeroPane, true)
    );
}

/// ABS Podcasts (task 2.1): the wide right hero pane fills via
/// `wide_hero_hero_pane`.
/// D8's gain: this surface goes focus-green whenever the library column holds
/// focus, not only when the episode workspace holds the cursor.
#[test]
fn abs_podcasts_wide_left_pane_fills_via_shared_primitive() {
    let app = crate::app::tests_podcast::audiobookshelf_app();
    let mut component = AudiobookshelfPodcastComponent::new();
    if let Some(state) = app.audiobookshelf_browse.first() {
        component.set_content(state, app.images_enabled());
        component.set_focused(true);
    }
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));
    let geometry = component.geometry();
    let hero = geometry.hero_area;
    assert!(hero.width > 0 && hero.height > 0, "hero={hero:?}");
    let buffer = terminal.backend().buffer();
    // The library column holds focus, so the pane is focus-green from the
    // start. Before `unify-surface-colour` 3.2 this was `SURFACE_RESTING`
    // while the show list held the cursor: the per-screen `episode_focused`
    // bit no longer chooses the pane fill.
    assert_eq!(
        buffer[(hero.x, hero.y)].bg,
        surface_fill(palette::Surface::HeroPane, true)
    );

    component.enter_episode_focus();
    let focused_terminal = direct_terminal(|f| component.view(f, area));
    let focused_buffer = focused_terminal.backend().buffer();
    // Moving the cursor into the episode pane changes no fill.
    assert_eq!(
        focused_buffer[(hero.x, hero.y)].bg,
        surface_fill(palette::Surface::HeroPane, true)
    );
}

/// `unify-surface-colour` 3.2: the wide ABS Books rail and hero pane both
/// follow the library column's focus, not the chapter cursor. Moving the
/// cursor from the book rail into the chapter pane changes no fill.
///
/// 3.4: the chapter pane's selected-row highlight is asserted too, in both
/// cursor positions. It is observable only through the selection marker,
/// because the chapter list's `ListBackdrop` selected-row surface equals the
/// chapter content box's `SURFACE_BACKDROP`.
#[test]
fn abs_books_panel_fills_follow_the_library_column_not_the_chapter_cursor() {
    use crate::app::render::arrangements::wide_hero::wide_hero_browser_pane;
    use crate::app::render::components::list_rows::{selection_marker, MarkerEdge};

    // The marker the media-list painter writes on a focused selected row,
    // taken from the production primitive rather than hard-coded.
    let marker = selection_marker(true, MarkerEdge::Left);
    let marker_glyph = marker.content.to_string();
    let marker_fg = marker.style.fg.expect("selected-row marker paints");

    let app = make_audiobookshelf_book_app();
    let mut component = AudiobookshelfBookComponent::new();
    if let Some(state) = app.audiobookshelf_book_browse.first() {
        component.set_content(state, app.images_enabled());
        component.set_focused(true);
    }
    let area = wide_area();
    let panes = wide_library_panes(area, 0, PANE_PAD_Y, None).expect("wide fits");
    let list_panel = wide_hero_browser_pane(panes.browser_panel, panes.browser_area).list_panel;
    let hero_panel = panes.hero_panel;

    let terminal = direct_terminal(|f| component.view(f, area));
    let buffer = terminal.backend().buffer();
    let rail_fill = buffer[(list_panel.x, list_panel.y)].bg;
    let hero_fill = buffer[(hero_panel.x, hero_panel.y)].bg;
    // The selected book row sits at the rail's first content row; while the
    // rail holds the cursor it carries the punch-through surface.
    let rail_selected = (list_panel.x + PANE_PAD_X, list_panel.y + PANE_PAD_Y);
    assert_eq!(
        rail_fill,
        surface_fill(palette::Surface::LibraryPanel, true),
        "book rail body follows the library column's focus"
    );
    assert_eq!(
        hero_fill,
        surface_fill(palette::Surface::HeroPane, true),
        "hero pane follows the library column's focus"
    );
    assert_ne!(
        buffer[rail_selected].bg, rail_fill,
        "book rail selected-row highlight while the rail holds the cursor"
    );
    assert!(
        !region_has_selection_marker(buffer, hero_panel, &marker_glyph),
        "no chapter selected row is marked while the book rail holds the cursor"
    );

    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Left,
        modifiers: KeyModifiers::NONE,
    }));
    let terminal = direct_terminal(|f| component.view(f, area));
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(list_panel.x, list_panel.y)].bg,
        rail_fill,
        "book rail body fill is unchanged when the cursor moves into the chapter pane"
    );
    assert_eq!(
        buffer[(hero_panel.x, hero_panel.y)].bg,
        hero_fill,
        "hero pane fill is unchanged when the cursor moves into the chapter pane"
    );
    assert_eq!(
        buffer[rail_selected].bg, rail_fill,
        "book rail selected-row highlight is gone while the chapter pane holds the cursor"
    );
    // The chapter pane now marks its selected chapter, and the book rail's
    // mark is the only one gone: exactly one sub-panel highlights at a time.
    let chapter = component
        .chapter_content_rect_for_test()
        .expect("the chapter pane paints its rows");
    assert_eq!(
        buffer[(chapter.x, chapter.y)].symbol(),
        marker_glyph.as_str(),
        "chapter pane marks its selected row while it holds the cursor"
    );
    assert_eq!(buffer[(chapter.x, chapter.y)].fg, marker_fg);
    assert!(
        region_has_selection_marker(buffer, hero_panel, &marker_glyph),
        "the chapter selected row is marked inside the hero pane"
    );

    // Queue column holds panel focus: both panels rest.
    component.set_focused(false);
    let terminal = direct_terminal(|f| component.view(f, area));
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(list_panel.x, list_panel.y)].bg,
        surface_fill(palette::Surface::LibraryPanel, false),
        "book rail body rests when the queue column holds focus"
    );
    assert_eq!(
        buffer[(hero_panel.x, hero_panel.y)].bg,
        surface_fill(palette::Surface::HeroPane, false),
        "hero pane rests when the queue column holds focus"
    );
}

/// `unify-surface-colour` 3.2: the wide ABS Podcasts rail and hero pane both
/// follow the library column's focus, not the episode cursor. Moving the
/// cursor from the show rail into the episode pane changes no fill.
#[test]
fn abs_podcasts_panel_fills_follow_the_library_column_not_the_episode_cursor() {
    use crate::app::render::arrangements::wide_hero::{
        wide_hero_browser_pane, wide_hero_presentation,
    };

    let app = crate::app::tests_podcast::audiobookshelf_app();
    let mut component = AudiobookshelfPodcastComponent::new();
    if let Some(state) = app.audiobookshelf_browse.first() {
        component.set_content(state, app.images_enabled());
        component.set_focused(true);
    }
    let area = wide_area();
    let panes = wide_hero_presentation(area, None).expect("wide fits");
    let list_panel = wide_hero_browser_pane(panes.browser, panes.browser).list_panel;
    let hero_panel = panes.hero;

    let terminal = direct_terminal(|f| component.view(f, area));
    let buffer = terminal.backend().buffer();
    let rail_fill = buffer[(list_panel.x, list_panel.y)].bg;
    let hero_fill = buffer[(hero_panel.x, hero_panel.y)].bg;
    assert_eq!(
        rail_fill,
        surface_fill(palette::Surface::LibraryPanel, true),
        "show rail body follows the library column's focus"
    );
    assert_eq!(
        hero_fill,
        surface_fill(palette::Surface::HeroPane, true),
        "hero pane follows the library column's focus"
    );

    component.enter_episode_focus();
    let terminal = direct_terminal(|f| component.view(f, area));
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(list_panel.x, list_panel.y)].bg,
        rail_fill,
        "show rail body fill is unchanged when the cursor moves into the episode pane"
    );
    assert_eq!(
        buffer[(hero_panel.x, hero_panel.y)].bg,
        hero_fill,
        "hero pane fill is unchanged when the cursor moves into the episode pane"
    );

    // Queue column holds panel focus: both panels rest.
    component.set_focused(false);
    let terminal = direct_terminal(|f| component.view(f, area));
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(list_panel.x, list_panel.y)].bg,
        surface_fill(palette::Surface::LibraryPanel, false),
        "show rail body rests when the queue column holds focus"
    );
    assert_eq!(
        buffer[(hero_panel.x, hero_panel.y)].bg,
        surface_fill(palette::Surface::HeroPane, false),
        "hero pane rests when the queue column holds focus"
    );
}

#[test]
fn abs_book_wide_hero_keeps_text_with_images_on_or_off() {
    let app = make_audiobookshelf_book_app();
    for images_enabled in [true, false] {
        let mut component = AudiobookshelfBookComponent::new();
        component.set_content(
            app.audiobookshelf_book_browse.first().expect("book state"),
            images_enabled,
        );
        component.set_focused(true);
        let terminal = direct_terminal(|f| component.view(f, wide_area()));
        assert!(buffer_to_string(&terminal).contains("Alpha Tales"));
    }
}

#[test]
fn abs_podcast_wide_hero_keeps_text_with_images_on_or_off() {
    let app = crate::app::tests_podcast::audiobookshelf_app();
    for images_enabled in [true, false] {
        let mut component = AudiobookshelfPodcastComponent::new();
        component.set_content(
            app.audiobookshelf_browse.first().expect("podcast state"),
            images_enabled,
        );
        component.set_focused(true);
        let terminal = direct_terminal(|f| component.view(f, wide_area()));
        assert!(buffer_to_string(&terminal).contains("Show A"));
    }
}

/// Sanity: the fixture width used throughout this module clears the shared
/// two-column breakpoint, so every characterization above exercises the Wide
/// Wide hero presentation rather than falling back to narrow.
#[test]
fn fixture_width_is_wide() {
    const { assert!(WIDTH >= TWO_COLUMN_THRESHOLD) };
    let _ = buffer_to_string; // keep the shared helper import exercised
}
