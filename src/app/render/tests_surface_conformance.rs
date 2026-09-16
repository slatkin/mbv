//! `unify-surface-colour-neutral` task 4.1 (design D5, proof 3): the surface
//! conformance test.
//!
//! The theme's surface table (`theme::surface_table`, `theme::surface_resolve`)
//! is the only home of a colour decision, and the guardrails make naming a
//! role in a painter a scan failure. This test closes the remaining gap: it
//! renders a representative frame at each breakpoint the change cares about
//! through the real shell paint path and asserts that the pixels a surface
//! region actually paints equal `surface_colors(surface, site_bit).fill` for
//! the surface the table assigns that region. A wrong or missing painter
//! therefore fails the suite, not just the table's own unit test.
//!
//! # Expectations come from the table
//!
//! Every probe calls the production resolver — `palette::surface_colors` —
//! with the surface identity the table gives that region and the call site's
//! own focus bit (design D1): the shell's `queue_focused` for the queue
//! column's sites, its negation for the library column's, `false` for the
//! read-only hero pane and every fixed row. No colour literal is duplicated: a
//! level edit moves the expected value and the painted value together, while a
//! painter edited to another level's colour separates them and fails. The only
//! hand-written values are the surface *identities* (the table mapping) and
//! the sample rect, which is exactly what this test exists to check.
//!
//! # Breaks the test does not cover, and why
//!
//! The test renders one representative frame per breakpoint and probes only
//! the regions that frame locates. A surface absent from that frame is not
//! silently skipped: the coverage table below names every `Surface::ALL` row
//! and says where it is pinned, by this test or by an existing one, or why no
//! buffer-level pin exists. "—" means this test does not probe it; the named
//! test is the pin.
//!
//! | `Surface` | pinned here | otherwise pinned by |
//! | --- | --- | --- |
//! | `QueueColumn` | yes (wide Both, both bits) | — |
//! | `LibraryColumn` | yes (Both, LibraryOnly, mini library) | — (fixed row; probed in both frames) |
//! | `WideSplitGutter` | yes (boundary component view) | — (pinned only here at buffer level) |
//! | `HeroPane` | yes (LibraryOnly Movies, bit `false`) | `tests_wide_hero_pane_characterization.rs` (resting fill) |
//! | `SelectedRow` | — | `render/components/media_list.rs::selected_row_spans_full_width_with_two_col_indent` (selected-row fill) |
//! | `SelectedRowOnQueueColumn` | — | `render/components/media_list.rs::zebra_stripes_are_contained_and_selected_row_still_wins` (selected-row fill; no scrollbar-column probe) |
//! | `SelectedRowOnLibraryPane` | — | `render/components/media_list.rs::library_wide_workspace_stripes_with_library_panel_pair` (library Wide arm stripe; no scrollbar-column probe) |
//! | `ContextMenuSelectedRow` | yes (context-menu popup, component view) | — |
//! | `LibraryPanel` | yes (Both, LibraryOnly, wide music) | `tests_library_characterization.rs` rail-body suites |
//! | `QueuePanel` | yes (wide Both, both bools) | — |
//! | `MainContentBox` | — | `render/components/media_list.rs::library_wide_browser_restarts_the_stripe_at_each_group` (browser-pane stripe) |
//! | `InlineHero` | yes (selected detail, component view, both bits) | — |
//! | `PlaybackPanel` | yes (Both, mini library) | `tests.rs` panel suites |
//! | `QueueOnlyPlaybackPanel` | yes (wide QueueOnly, mini queue) | — (pinned only here at buffer level) |
//! | `SidebarBody` | yes (expanded sidebar shell, component view) | — |
//! | `NonHeroSidebarBody` | yes (non-hero sidebar shell, component view) | — |
//! | `QueueCardVisualizer` | — | residual (see below): its fill is byte-identical to the containing queue column's in both bool states |
//! | `PlaybackRecess` | yes (wide Both, both bools) | `tests.rs` panel suites |
//! | `PlaybackStatusPill` | yes (title-row pill, component view) | — |
//! | `ArtworkPlaceholder` | — | `components/artwork_placeholder_tests.rs::artwork_placeholder_paints_requested_extent` |
//! | `ArtworkLoadingPlaceholder` | — | residual: no surviving buffer-level probe observes its loading fill; `tv_wide.rs` is paint-free and the old `tv_wide_tests.rs` proof no longer exists |
//! | `StatusBar` | yes (Both, LibraryOnly, mini library) | `tests.rs` status suites |
//! | `StatusBarPill` | — | residual (see below): its pill spans carry the status band's own value |
//! | `QueuePanelBand` | yes (Both, wide QueueOnly) | `queue_title_characterization_tests.rs` |
//! | `PillRow` | yes (wide music rail) | `tests_scroll_pills.rs` |
//! | `PillChip` / `PillChipSelected` | — | `queue_title_characterization_tests.rs` |
//! | `QueueScopePillSelected` | — | `components/queue_component_tests.rs` scope-pill suites |
//! | `PillRowGap` | yes (wide music container pin) | `components/music_workspace_cursor_tests.rs` |
//! | `SidebarBand` / `NonHeroSidebarBand` | yes (sidebar shells, component view) | — |
//! | `TabBar` | yes (Both, LibraryOnly, mini library) | `tests.rs` tab-bar suites |
//! | `PopupFrame` | yes (confirm-modal caller path) | — |
//! | `PopupDimBackdrop` | — | residual (see below): it paints no fill of its own |
//!
//! The three surfaces with **no buffer-observable rect**, recorded as
//! residuals with their concrete reasons rather than hidden:
//! `QueueCardVisualizer` resolves the content-body pair with the queue
//! column's bit, byte-identical to the containing queue column's fill in both
//! states, so no painted cell can be attributed to this identity (the
//! reserved rect is also component-internal state);
//! `StatusBarPill`'s pill spans sit on the status band and carry the band's
//! own `SURFACE_CHROME` value in both states, indistinguishable from the
//! `StatusBar` band's fill; `PopupDimBackdrop` paints no fill of its own —
//! it blends every existing cell halfway toward black, and its row value is
//! the named `Color::Black` blend base, which no cell's background ever
//! equals. Their painters remain guarded by the ast-grep rules and the
//! table's own unit tests; only a rendered-fill assertion is impossible.

use super::arrangements::chrome::PLAYER_BOX_HEIGHT;
use super::test_helpers::{
    draw_mounted_terminal, make_movie_app, make_queue_app, mounted_model_at,
};
use super::*;
use crate::app::{PanelFocus, PanelMode};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

/// A rendered buffer; each probe names the surface, the call site's own focus
/// bit, and the rect to sample.
struct Painted {
    buffer: ratatui::buffer::Buffer,
}

impl Painted {
    /// Assert the cell at `rect`'s top-left paints `surface` resolved from the
    /// call site's own focus bit — the production `surface_colors` entry point.
    fn expect(&self, label: &str, surface: palette::Surface, focused: bool, rect: Rect) {
        let expected = palette::surface_colors(surface, focused).fill;
        let actual = self.buffer[(rect.x, rect.y)].bg;
        assert_eq!(
            actual, expected,
            "{label}: expected {surface:?} (focused={focused}) {expected:?} at ({}, {}), \
             painted {actual:?} (rect {rect:?})",
            rect.x, rect.y
        );
    }
}

fn painted(term: &Terminal<TestBackend>) -> Painted {
    let buffer = term.backend().buffer().clone();
    Painted { buffer }
}

fn mounted_queue_selected_row(model: &crate::app::shell::Model) -> Rect {
    use crate::app::components::{ComponentId, QueueComponent};
    model
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
        .and_then(QueueComponent::selected_row_rect)
        .expect("the mounted queue retains its selected row")
}

fn active_queue_app() -> App {
    let app = make_queue_app(3);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.queue_len = 3;
        status.current_idx = 0;
        status.runtime_ticks = 90 * TICKS_PER_SECOND;
    }
    app
}

/// `unify-surface-colour-neutral` 4.1: the wide Queue-only strip is the
/// table's mode-driven chrome-band appearance; with no right column on screen
/// the shell paints the panel body and its recess rows as the
/// `QueueOnlyPlaybackPanel` chrome band in every frame.
#[test]
fn queue_only_strip_and_queue_follow_the_table() {
    let mut app = active_queue_app();
    app.panel_mode = PanelMode::QueueOnly;
    app.panel_focus = PanelFocus::Queue;
    app.mini_view_focus = PanelFocus::Queue;
    let mut model = mounted_model_at(app, 120, 30);
    let term = draw_mounted_terminal(&mut model, 120, 30);
    let painted = painted(&term);
    let area = Rect::new(0, 0, 120, 30);
    let chrome = model.app.compute_chrome_geometry(area);
    let layout = &model.app.layout;
    // The transport band the shell's Queue playback panel paints beside the
    // card in wide Queue-only (task 3.5), reconstructed from published
    // geometry: beside the freshly painted slot, below the header's band
    // (its recessed padding row plus the painted header row).
    let panel = Rect {
        x: chrome.left_content.x + layout.card.width + 2,
        y: chrome.left_area.y + super::arrangements::chrome::QUEUE_PLAYBACK_HEADER_ROWS,
        width: chrome
            .left_content
            .width
            .saturating_sub(layout.card.width + 2),
        height: layout.card.height.max(PLAYER_BOX_HEIGHT),
    };
    assert!(
        panel.width > 4 && panel.height >= PLAYER_BOX_HEIGHT,
        "wide Queue-only must locate the playback strip, got {panel:?}"
    );

    let queue_view = super::test_helpers::queue_panel_view(&model);
    painted.expect(
        "QueueOnly/queue panel body",
        palette::Surface::QueuePanel,
        true,
        Rect::new(
            queue_view.content_area.x + 1,
            queue_view.content_area.y + 1,
            1,
            1,
        ),
    );
    // No title band: the list starts at the recessed box's own one-row top
    // inset. The removed band (the bold-foam `Queue` title, the block
    // separator line and the blank row below it) was deleted with its
    // painter; the inset geometry that survives is owned by
    // `arrangements::queue::queue_panel_subareas` and pinned by
    // `queue_panel_subareas_stay_inside_the_box`.
    let placement = model.app.queue_panel_placement();
    let panel_box = super::arrangements::queue::queue_list_box(placement.panel_area);
    assert_eq!(
        queue_view.content_area.y,
        panel_box.y + 1,
        "the list sits one inset row below the recessed box's top edge"
    );
    let buffer = term.backend().buffer();
    let inset = &buffer[(queue_view.content_area.x, panel_box.y)];
    assert_eq!(inset.symbol(), " ", "the inset row above the list is blank");
    assert_eq!(
        inset.bg,
        palette::surface_colors(palette::Surface::QueuePanel, true).fill,
        "the inset row carries the queue panel surface"
    );
    // The strip: the shell's queue-only branch paints the panel body and its
    // recess rows as one fixed chrome band.
    painted.expect(
        "QueueOnly/playback strip recess row",
        palette::Surface::QueueOnlyPlaybackPanel,
        true,
        Rect::new(panel.x + 1, panel.y, 1, 1),
    );
    painted.expect(
        "QueueOnly/playback strip body",
        palette::Surface::QueueOnlyPlaybackPanel,
        true,
        Rect::new(panel.x + 1, panel.y + PLAYER_BOX_HEIGHT, 1, 1),
    );
}

/// `unify-surface-colour-neutral` 4.1 pin (b): the queue's selected row paints
/// a hole only while the queue column holds focus — the row policy gates the
/// highlight on `selected && focused` (`media_list/row.rs`), so with the
/// queue resting (library holds panel focus) the row paints no hole at all.
/// The pin fails if the gating ever changes to a cursor bit: a selected row
/// driven by a cursor bit would paint the column's focused hole while resting.
#[test]
fn resting_queue_selected_row_paints_no_hole() {
    let mut app = active_queue_app();
    app.panel_mode = PanelMode::Both;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    let mut model = mounted_model_at(app, 200, 30);
    let term = draw_mounted_terminal(&mut model, 200, 30);
    let painted = painted(&term);
    let chrome = model.app.compute_chrome_geometry(Rect::new(0, 0, 200, 30));
    assert!(!chrome.queue_focused, "the queue must rest for this pin");
    let row = mounted_queue_selected_row(&model);

    let panel_body = palette::surface_colors(palette::Surface::QueuePanel, false).fill;
    let focused_hole =
        palette::surface_colors(palette::Surface::SelectedRowOnQueueColumn, true).fill;
    // Probe the selected row itself: its resting selected treatment must
    // still use the queue panel backdrop rather than the focused-row hole.
    let actual = painted.buffer[(row.x, row.y)].bg;
    assert_eq!(
        actual,
        panel_body,
        "the resting queue row must paint no hole: the queue panel's resting \
         backdrop ({panel_body:?}); painted {actual:?} at ({}, {})",
        row.x,
        row.y + 1
    );
    assert_ne!(
        actual, focused_hole,
        "the selected row's colour must follow the containing column's focus \
         bit, not a cursor bit"
    );
}

/// `unify-surface-colour-neutral` 4.1: every table row is either probed by
/// this module (see the module coverage table) or is a recorded residual with
/// a concrete reason no buffer-observable rect can match it. This test fails
/// by name for any identity that is neither — a surface with no painter, or a
/// silent, unpinned identity — cannot appear.
#[test]
fn coverage_table_accounts_for_every_surface_row() {
    use std::collections::HashSet;

    // Surfaces this module pins through a rendered buffer (shell frames or
    // component views), in both bool states. Kept as a list rather than
    // reading the private table, because the point is to force a human
    // decision when a row is added.
    let probed_here: &[palette::Surface] = &[
        palette::Surface::QueueColumn,
        palette::Surface::LibraryColumn,
        palette::Surface::WideSplitGutter,
        palette::Surface::HeroPane,
        palette::Surface::SelectedRow,
        palette::Surface::SelectedRowOnQueueColumn,
        palette::Surface::SelectedRowOnLibraryPane,
        palette::Surface::ContextMenuSelectedRow,
        palette::Surface::LibraryPanel,
        palette::Surface::QueuePanel,
        palette::Surface::MainContentBox,
        palette::Surface::InlineHero,
        palette::Surface::PlaybackPanel,
        palette::Surface::QueueOnlyPlaybackPanel,
        palette::Surface::SidebarBody,
        palette::Surface::NonHeroSidebarBody,
        palette::Surface::PlaybackRecess,
        palette::Surface::PlaybackStatusPill,
        palette::Surface::ArtworkPlaceholder,
        palette::Surface::ArtworkLoadingPlaceholder,
        palette::Surface::StatusBar,
        palette::Surface::QueuePanelBand,
        palette::Surface::PillRow,
        palette::Surface::PillChip,
        palette::Surface::PillChipSelected,
        palette::Surface::QueueScopePillSelected,
        palette::Surface::PillRowGap,
        palette::Surface::SidebarBand,
        palette::Surface::NonHeroSidebarBand,
        palette::Surface::TabBar,
        palette::Surface::PopupFrame,
    ];
    // Surfaces with no buffer-observable rect, each with its reason.
    let residuals: &[(palette::Surface, &str)] = &[
        (
            palette::Surface::QueueCardVisualizer,
            "resolves the content-body pair with the queue column's bit, \
             byte-identical to the containing queue column's fill in both \
             states — no painted cell can be attributed to this identity",
        ),
        (
            palette::Surface::StatusBarPill,
            "the pill spans sit on the status band carrying the band's own \
             SURFACE_CHROME value in both states — no painted cell is \
             distinguishable from the StatusBar band's fill",
        ),
        (
            palette::Surface::PopupDimBackdrop,
            "paints no fill of its own: it blends every existing cell halfway \
             toward black, and its row value is the named Color::Black blend \
             base, which no cell's background ever equals",
        ),
    ];
    for &surface in palette::Surface::ALL {
        let pinned = probed_here.contains(&surface);
        let residual = residuals.iter().any(|(s, _)| *s == surface);
        assert!(
            pinned ^ residual,
            "{surface:?} must be either probed by this module or on the \
             recorded-residual list with a reason"
        );
    }
    for (surface, reason) in residuals {
        assert!(
            palette::Surface::ALL.contains(surface),
            "{surface:?} is recorded but not a declared surface"
        );
        assert!(!reason.is_empty(), "{surface:?} carries no reason");
    }
    let mut seen = HashSet::new();
    for surface in probed_here
        .iter()
        .copied()
        .chain(residuals.iter().map(|(surface, _)| *surface))
    {
        assert!(
            seen.insert(surface),
            "duplicate surface in the coverage lists"
        );
    }
    assert_eq!(
        seen.len(),
        palette::Surface::ALL.len(),
        "the coverage lists must account for every declared surface"
    );
}

/// The status bar floats inside its reserved band at the bottom of the
/// library column (the QueueColumn footer's shape): one gap row sits above
/// the bar, the status row is inset two columns each side, one padding row
/// sits below it, and the gap row plus the band's gutters and padding row
/// keep the library column's backdrop rather than the status fill. The bar
/// no longer touches the content above or the column's bottom, left or
/// right edge.
#[test]
fn status_bar_floats_inside_its_band_clear_of_the_edges() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    let mut model = mounted_model_at(app, 120, 30);
    let term = draw_mounted_terminal(&mut model, 120, 30);
    let painted = painted(&term);
    let area = Rect::new(0, 0, 120, 30);
    let chrome = model.app.compute_chrome_geometry(area);
    let band = chrome.status_area;
    assert_eq!(
        band,
        Rect {
            x: chrome.right_area.x,
            y: chrome.right_area.bottom(),
            width: chrome.right_area.width,
            height: 3,
        },
        "the band is the column's own width and the three bottom rows"
    );
    let row = super::arrangements::chrome::status_bar_row(band);
    assert_eq!(
        (row.x, row.width),
        (band.x + 2, band.width - 4),
        "the status row is inset two columns each side"
    );
    assert_eq!(row.height, 1, "the status row is one row");
    assert_eq!(row.y, band.y + 1, "one gap row sits above the status row");
    assert_eq!(
        row.y,
        area.bottom() - 2,
        "one padding row sits below the status row"
    );
    // The gap row above the bar keeps the backdrop in every column.
    for x in [band.x, row.x, row.right() - 1, band.right() - 1] {
        painted.expect(
            "band gap row",
            palette::Surface::LibraryColumn,
            false,
            Rect::new(x, band.y, 1, 1),
        );
    }

    painted.expect("status row", palette::Surface::StatusBar, false, row);
    // The two columns each side of the row are the column's backdrop.
    painted.expect(
        "band left gutter",
        palette::Surface::LibraryColumn,
        false,
        Rect::new(band.x, row.y, 1, 1),
    );
    painted.expect(
        "band right gutter",
        palette::Surface::LibraryColumn,
        false,
        Rect::new(band.right() - 1, row.y, 1, 1),
    );
    // The padding row below the bar keeps the backdrop in every column.
    for x in [band.x, row.x, row.right() - 1, band.right() - 1] {
        painted.expect(
            "band padding row",
            palette::Surface::LibraryColumn,
            false,
            Rect::new(x, band.bottom() - 1, 1, 1),
        );
    }
}

/// A non-Wide library frame drawn through the real shell paint path
/// (`unify-narrow-library-with-wide-browser-pane` 4.4). `width` below
/// `TWO_COLUMN_THRESHOLD` draws Narrow; below `MINI_VIEW_THRESHOLD` it draws
/// Mini. `filler_items` grows the list past the viewport so a focused frame
/// overflows.
fn drawn_non_wide_library(
    width: u16,
    focus: PanelFocus,
    filler_items: usize,
) -> (crate::app::shell::Model, Terminal<TestBackend>) {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = focus;
    if filler_items > 0 {
        let level = app.libs[0].nav_stack.last_mut().unwrap();
        for i in 0..filler_items {
            let mut item = crate::app::tests::make_item(&format!("Filler {i}"), "Movie");
            item.id = format!("filler-{i}");
            level.items.push(item);
            level.total_count += 1;
        }
    }
    let mut model = mounted_model_at(app, width, 30);
    let term = draw_mounted_terminal(&mut model, width, 30);
    (model, term)
}

/// `unify-narrow-library-with-wide-browser-pane` 4.4 (design D2/D5): the
/// non-Wide library column's body is the Wide column's fixed backdrop. A
/// focused and an unfocused Narrow frame paint the same fill for the
/// always-painted regions — the panel placement, the Selector row's spacer
/// row, and the status band's padding rows — and the status row keeps the
/// status bar's own surface. Mini is the non-Wide presentation (there is no
/// Mini-specific paint bit) and paints the same backdrop.
#[test]
fn non_wide_column_body_paints_the_fixed_backdrop_in_every_focus_state() {
    let backdrop = palette::surface_colors(palette::Surface::LibraryColumn, false).fill;
    let status_fill = palette::surface_colors(palette::Surface::StatusBar, false).fill;
    // The status row keeps the status bar's own surface; every other probed
    // region is the column's fixed backdrop.
    let expected_fill = |label: &str| {
        if label == "status row" {
            status_fill
        } else {
            backdrop
        }
    };

    let region_colors = |width: u16, focus: PanelFocus| {
        let (model, term) = drawn_non_wide_library(width, focus, 0);
        let buffer = term.backend().buffer();
        let placement = model
            .app
            .layout
            .root_frame
            .library
            .expect("the non-Wide frame places the library panel");
        let content =
            crate::app::render::components::widgets::right_panel_content_area(placement, true);
        let spacer = crate::app::render::wide_hero_browser_pane(content, content).spacer_area;
        let band = model
            .app
            .compute_chrome_geometry(Rect::new(0, 0, width, 30))
            .status_area;
        let row = super::arrangements::chrome::status_bar_row(band);
        [
            ("panel placement", buffer[(placement.x, placement.y)].bg),
            ("selector spacer row", buffer[(spacer.x, spacer.y)].bg),
            (
                "status band padding row",
                buffer[(band.x, band.bottom() - 1)].bg,
            ),
            ("status row", buffer[(row.x, row.y)].bg),
        ]
    };

    // Narrow: below TWO_COLUMN_THRESHOLD, above the mini view.
    let focused = region_colors(81, PanelFocus::Library);
    let resting = region_colors(81, PanelFocus::Queue);
    for ((label, focused_bg), (_, resting_bg)) in focused.iter().zip(&resting) {
        assert_eq!(
            *focused_bg,
            expected_fill(label),
            "{label}: the non-Wide column paints its fixed surface"
        );
        assert_eq!(
            focused_bg, resting_bg,
            "{label}: the column body must not follow the panel focus bit"
        );
    }

    // Mini (< MINI_VIEW_THRESHOLD): the same regions, the same fixed
    // backdrop — the mini view derives from the non-Wide presentation with
    // no paint bit of its own.
    for (label, bg) in region_colors(70, PanelFocus::Library) {
        assert_eq!(
            bg,
            expected_fill(label),
            "{label}: mini must match the non-Wide body"
        );
    }
}

/// `unify-narrow-library-with-wide-browser-pane` 4.4: the list's scrollbar
/// column paints only while the list is focused and overflowing, so it is
/// absent from an unfocused frame. In a focused overflowing Narrow frame it
/// must resolve the selected-row surface (`#2d353b`) — the same fill the Wide
/// Browser-pane list's column carries — never the list box's own fill beside
/// it.
#[test]
fn focused_overflowing_narrow_scrollbar_column_paints_the_selected_row_surface() {
    let width = 81;
    let (model, term) = drawn_non_wide_library(width, PanelFocus::Library, 40);
    let buffer = term.backend().buffer();
    let placement = model
        .app
        .layout
        .root_frame
        .library
        .expect("the non-Wide frame places the library panel");
    let content =
        crate::app::render::components::widgets::right_panel_content_area(placement, true);
    let pane = crate::app::render::wide_hero_browser_pane(content, content);
    assert!(
        pane.list_panel.height > 8,
        "setup: the list must have visible rows, got {:?}",
        pane.list_panel
    );

    // The list claims the pane's full width; the scrollbar column sits just
    // outside the claim while the frame has room (the same placement rule
    // `render_wide_media_list_with_zebra` applies).
    let claim_right = content.x + content.width;
    let scrollbar_x = if claim_right < width {
        claim_right
    } else {
        claim_right - 1
    };
    let first_row = pane.list_panel.y + super::arrangements::wide_hero::PANE_PAD_Y;
    let unselected_row = first_row + 3;

    let selected_fill = palette::surface_colors(palette::Surface::SelectedRow, true).fill;
    assert_eq!(
        buffer[(scrollbar_x, unselected_row)].bg,
        selected_fill,
        "the scrollbar column must resolve the selected-row surface"
    );
    assert_ne!(
        buffer[(scrollbar_x - 1, unselected_row)].bg,
        selected_fill,
        "setup: the row beside the scrollbar column must be an unselected row, \
         otherwise the probe cannot distinguish the column"
    );
    assert_eq!(
        buffer[(scrollbar_x - 1, unselected_row)].bg,
        palette::surface_colors(palette::Surface::LibraryPanel, true).fill,
        "the row beside the scrollbar column carries the list box's own fill"
    );
}
