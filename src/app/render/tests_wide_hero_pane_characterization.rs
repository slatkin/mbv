//! Characterization tests (task 1.1, standardize-Wide-hero-pane): pin the
//! *current* Wide browser-pane output of all seven Wide hero destinations
//! before any paint/primitive change lands. These intentionally capture
//! today's drifted behaviour (the Home clamp, ABS Podcasts' missing fill, ABS
//! Books' foreground-only `.style(Color)` bug, Feeds' conditional fill) as-is
//! -- they are a baseline to diff phases 2/3 against, not a statement of
//! correct behaviour. Must land in its own commit before any Wide hero paint
//! or primitive change (ledger migration flow).

use super::test_helpers::buffer_to_string;
use crate::app::components::feeds_content::{FeedsContent, FeedsOwnerPush};
use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::tv_content::TvContent;
use crate::app::components::LibraryKind;
use crate::app::palette;
use crate::app::render::arrangements::library::wide_library_panes;
use crate::app::render::arrangements::wide_hero::{PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::TvWideRenderCtx;
use crate::app::tests::make_item;
use crate::app::TWO_COLUMN_THRESHOLD;
use mbv_core::config::{FeedKind, FeedSubscription, ServiceKind};
use mbv_core::playback_queue::FeedEntry;
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

/// TV already routes through `wide_library_panes(area, PANE_PAD_X,
/// PANE_PAD_Y)` and `resolve_surface_focus` -- the one destination the
/// standardization leaves visually unchanged (task 3.2). Task 8.4 hosts the
/// owner in the mounted panel.
#[test]
fn tv_wide_left_pane_unconditional_fill_shared_inset() {
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![make_item("Focused Series", "Series")], 0, 0),
        None,
        None,
        0,
        None,
        false,
    ));
    let key = LibraryKey::Service {
        service: ServiceKind::Emby,
        library_id: "lib".into(),
        kind: LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key.clone(), Box::new(owner));
    panel.set_active(Some(key));
    let area = wide_area();
    let terminal = direct_terminal(|f| Component::view(&mut panel, f, area));
    let buffer = terminal.backend().buffer();

    let panes = wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y, None).expect("wide fits");
    let hero_panel = panes.hero_panel;

    assert_eq!(
        buffer[(hero_panel.x, hero_panel.y)].bg,
        palette::resolve_surface_focus(false)
    );
    assert_eq!(
        buffer[(hero_panel.x, hero_panel.bottom() - 1)].bg,
        palette::resolve_surface_focus(false)
    );
}

fn render_feeds_panel(entries: Vec<FeedEntry>) -> (Terminal<TestBackend>, Rect) {
    let subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let grouped = vec![entries];
    let all_entries = grouped[0].clone();
    let mut owner = FeedsContent::new();
    owner.set_content(FeedsOwnerPush {
        subscriptions,
        entries: grouped,
        all_entries,
        loading: false,
    });
    let mut panel = LibraryPanel::new();
    panel.insert_owner(LibraryKey::Feeds, Box::new(owner));
    panel.set_active(Some(LibraryKey::Feeds));
    tuirealm::component::Component::attr(
        &mut panel,
        tuirealm::props::Attribute::Focus,
        tuirealm::props::AttrValue::Flag(true),
    );
    let area = wide_area();
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    terminal
        .draw(|f| Component::view(&mut panel, f, area))
        .unwrap();
    let hero = panel
        .test_wide_geometry()
        .expect("the panel painted a Wide skeleton")
        .hero_area;
    (terminal, hero)
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
/// starting point). Painted through the mounted `LibraryPanel`'s embedded
/// `FeedsContent` owner (task 7.3).
#[test]
fn feeds_wide_left_pane_fills_when_an_entry_is_selected() {
    let (terminal, hero) = render_feeds_panel(vec![feed_entry("entry-1", "Entry One")]);
    assert!(hero.width > 0 && hero.height > 0);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(hero.x, hero.y)].bg, palette::SURFACE_RESTING);
    assert_eq!(
        buffer[(hero.x, hero.bottom() - 1)].bg,
        palette::SURFACE_RESTING
    );
}

/// Feeds (task 7.2): the wide right hero pane fill is unconditional (D1) --
/// with no selectable entry the panel still fills `SURFACE_RESTING` (D3:
/// read-only, never focus-green) around the empty-slot placeholder. Feeds
/// paints through the shared panel skeleton now, so `hero_area` is published
/// whether or not a hero exists.
#[test]
fn feeds_wide_left_pane_fills_unconditionally_with_no_selection() {
    let (terminal, hero) = render_feeds_panel(vec![]);
    assert!(hero.width > 0 && hero.height > 0, "hero={hero:?}");
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(hero.x, hero.y)].bg, palette::SURFACE_RESTING);
    assert_eq!(
        buffer[(hero.x, hero.bottom() - 1)].bg,
        palette::SURFACE_RESTING
    );
    assert!(buffer_to_string(&terminal).contains("Press r to load feeds"));
}

/// Sanity: the fixture width used throughout this module clears the shared
/// two-column breakpoint, so every characterization above exercises the Wide
/// Wide hero presentation rather than falling back to narrow.
#[test]
fn fixture_width_is_wide() {
    const { assert!(WIDTH >= TWO_COLUMN_THRESHOLD) };
    let _ = buffer_to_string; // keep the shared helper import exercised
}
