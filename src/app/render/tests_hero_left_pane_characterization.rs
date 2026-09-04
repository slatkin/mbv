//! Characterization tests (task 1.1, standardize-hero-on-left-pane): pin the
//! *current* Wide left-pane output of all seven hero-on-left destinations
//! before any paint/primitive change lands. These intentionally capture
//! today's drifted behaviour (the Home clamp, ABS Podcasts' missing fill, ABS
//! Books' foreground-only `.style(Color)` bug, Feeds' conditional fill) as-is
//! -- they are a baseline to diff phases 2/3 against, not a statement of
//! correct behaviour. Must land in its own commit before any hero-left paint
//! or primitive change (ledger migration flow).

use super::test_helpers::{buffer_to_string, make_audiobookshelf_book_app, make_music_group_app};
use crate::app::components::browser::BrowserContent;
use crate::app::components::{
    AudiobookshelfBookComponent, AudiobookshelfPodcastComponent, BrowserComponent, BrowserKind,
    FeedsComponent, HomeComponent, MusicWorkspaceComponent, TvWorkspaceComponent,
};
use crate::app::palette;
use crate::app::render::arrangements::hero_left::{PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::library::wide_library_panes;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::TvWideRenderCtx;
use crate::app::tests::make_item;
use crate::app::TWO_COLUMN_THRESHOLD;
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::{FeedEntry, QueueItem};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::Component;

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

/// Movies/home-videos/Emby-podcasts/feed-group browser: `BrowserComponent`
/// drives `render_wide_movies` for `BrowserKind::Movies`/`HomeVideos`, or for
/// any kind carrying `narrow_extras.feed_items`. One representative kind
/// (`Movies`) characterizes the shared code path all four destinations run.
#[test]
fn movies_family_wide_left_pane_unconditional_fill_double_horizontal_inset() {
    let mut component = BrowserComponent::new_for_kind(BrowserKind::Movies);
    let mut item = make_item("Focused Movie", "Movie");
    item.overview = "A short overview.".into();
    component.set_content(BrowserContent::from_items(vec![item]), true);
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));
    let buffer = terminal.backend().buffer();

    let panes = wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y).expect("wide fits");
    let left_panel = panes.left_panel;

    // Fill is unconditional and full-extent today (not the broken class).
    assert_eq!(
        buffer[(left_panel.x, left_panel.y)].bg,
        palette::SURFACE_RESTING
    );
    assert_eq!(
        buffer[(left_panel.x, left_panel.bottom() - 1)].bg,
        palette::SURFACE_RESTING
    );
    // The known double-inset defect (D-F): `browser/paint.rs` insets the
    // hero content twice, an effective `(4, 1)` rather than the shared
    // `(PANE_PAD_X, PANE_PAD_Y)` -- capture that today's overview text does
    // not start at `left_panel.x + PANE_PAD_X`.
    let single_inset_x = left_panel.x + PANE_PAD_X;
    let row_text = (single_inset_x..left_panel.right())
        .map(|x| buffer[(x, left_panel.y + PANE_PAD_Y)].symbol())
        .collect::<String>();
    assert!(
        !row_text.trim_start().starts_with("Focused Movie"),
        "characterizes the pre-fix double inset; row={row_text:?}"
    );
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
        true,
        false,
    ));
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));
    let buffer = terminal.backend().buffer();

    let panes = wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y).expect("wide fits");
    let left_panel = panes.left_panel;

    assert_eq!(
        buffer[(left_panel.x, left_panel.y)].bg,
        palette::resolve_surface_focus(false)
    );
    assert_eq!(
        buffer[(left_panel.x, left_panel.bottom() - 1)].bg,
        palette::resolve_surface_focus(false)
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

    let panes = wide_library_panes(area, 0, PANE_PAD_Y).expect("wide fits");
    let left_panel = panes.left_panel;

    assert_eq!(
        buffer[(left_panel.x, left_panel.y)].bg,
        palette::resolve_surface_focus(false)
    );
    assert_eq!(
        buffer[(left_panel.x, left_panel.bottom() - 1)].bg,
        palette::resolve_surface_focus(false)
    );
}

/// Home's `HeroData::Generic` clamp (`home.rs:237-242`): `hero_area()`
/// reports the full unclamped pane (captured before the clamp mutates
/// `hero_panel.height` in place), but the fill only paints the clamped
/// subset -- rows below the clamp are left unpainted. Non-Emby Latest
/// selection (an Audiobookshelf book) exercises the `Generic` arm.
#[test]
fn home_wide_non_emby_latest_clamps_the_fill_below_reported_hero_area() {
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

    assert_eq!(buffer[(hero.x, hero.y)].bg, palette::SURFACE_RESTING);
    assert_ne!(
        buffer[(hero.x, hero.bottom() - 1)].bg,
        palette::SURFACE_RESTING,
        "characterizes the pre-fix clamp: the reported hero_area's bottom \
         row is not painted"
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
    component.set_content(&subscriptions, &grouped, &all_entries, false, true);
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
    assert_eq!(buffer[(hero.x, hero.y)].bg, palette::SURFACE_RESTING);
    assert_eq!(
        buffer[(hero.x, hero.bottom() - 1)].bg,
        palette::SURFACE_RESTING
    );
}

/// Feeds with no entries to select: `feeds.rs:170-184` returns before the
/// hero-on-left pane is ever reached (a placeholder message paints instead),
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
/// the wide left pane is filled via `hero_on_left_pane`, focus-green
/// (`LeftPaneFocus::Workspace`) only when a chapter is selected while
/// focused.
#[test]
fn abs_books_wide_left_pane_fills_via_shared_primitive() {
    let app = make_audiobookshelf_book_app();
    let mut component = AudiobookshelfBookComponent::new();
    if let Some(state) = app.audiobookshelf_book_browse.first() {
        component.set_content(state, true, app.images_enabled());
    }
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));
    let geometry = component.geometry();
    assert!(geometry.wide);
    let panes = wide_library_panes(area, 0, PANE_PAD_Y).expect("wide fits");
    let left_panel = panes.left_panel;
    let buffer = terminal.backend().buffer();
    // No chapter is selected in this fixture, so the workspace is not held:
    // the pane stays resting, not focus-green.
    assert_eq!(
        buffer[(left_panel.x, left_panel.y)].bg,
        palette::SURFACE_RESTING
    );
    assert_eq!(
        buffer[(left_panel.x, left_panel.bottom() - 1)].bg,
        palette::SURFACE_RESTING
    );
}

/// ABS Podcasts (task 2.1): the wide left pane fills via `hero_on_left_pane`.
/// D8's gain: this surface goes focus-green when the episode workspace holds
/// focus (mirroring TV), not a bare `focused`.
#[test]
fn abs_podcasts_wide_left_pane_fills_via_shared_primitive() {
    let app = crate::app::tests_podcast::audiobookshelf_app();
    let mut component = AudiobookshelfPodcastComponent::new();
    if let Some(state) = app.audiobookshelf_browse.first() {
        component.set_content(state, true, app.images_enabled());
    }
    let area = wide_area();
    let terminal = direct_terminal(|f| component.view(f, area));
    let geometry = component.geometry();
    let hero = geometry.hero_area;
    assert!(hero.width > 0 && hero.height > 0, "hero={hero:?}");
    let buffer = terminal.backend().buffer();
    // No episode is selected in this fixture: the show list holds focus, so
    // the pane stays resting even though the surface is focused overall
    // (D8/D3: never a bare `focused`).
    assert_eq!(buffer[(hero.x, hero.y)].bg, palette::SURFACE_RESTING);
    assert_ne!(
        buffer[(hero.x, hero.y)].bg,
        palette::resolve_surface_focus(true)
    );
}

/// Sanity: the fixture width used throughout this module clears the shared
/// two-column breakpoint, so every characterization above exercises the Wide
/// hero-on-left presentation rather than falling back to narrow.
#[test]
fn fixture_width_is_wide() {
    const { assert!(WIDTH >= TWO_COLUMN_THRESHOLD) };
    let _ = buffer_to_string; // keep the shared helper import exercised
}
