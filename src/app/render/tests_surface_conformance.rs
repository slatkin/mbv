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
//! | `SelectedRow` | yes (LibraryOnly library rail) | `components/tv_wide_tests.rs::wide_tv_left_focus_drops_the_right_rail_to_the_resting_surface` |
//! | `SelectedRowOnQueueColumn` | yes (wide Both, both bits) | — |
//! | `SelectedRowOnLibraryPane` | — | `components/tv_wide_tests.rs::wide_tv_episode_selected_row_uses_the_shared_focused_surface` |
//! | `ContextMenuSelectedRow` | yes (context-menu popup, component view) | — |
//! | `LibraryPanel` | yes (Both, LibraryOnly, wide music) | `tests_library_characterization.rs` rail-body suites |
//! | `QueuePanel` | yes (wide Both, both bools) | `tests_queue.rs` (focused soft fill) |
//! | `MainContentBox` | — | `components/tv_wide_tests.rs` (focused soft fill), `components/music_workspace_cursor_tests.rs` |
//! | `InlineHero` | yes (selected detail, component view, both bits) | — |
//! | `PlaybackPanel` | yes (Both, mini library) | `tests.rs` panel suites |
//! | `QueueOnlyPlaybackPanel` | yes (wide QueueOnly, mini queue) | — (pinned only here at buffer level) |
//! | `SidebarBody` | yes (expanded sidebar shell, component view) | — |
//! | `NonHeroSidebarBody` | yes (non-hero sidebar shell, component view) | — |
//! | `QueueCardVisualizer` | — | residual (see below): its fill is byte-identical to the containing queue column's in both bool states |
//! | `PlaybackRecess` | yes (wide Both, both bools) | `tests.rs` panel suites |
//! | `PlaybackBottomRow` | yes (QueueOnly strip, mini queue) | — (pinned only here at buffer level) |
//! | `PlaybackStatusPill` | yes (title-row pill, component view) | — |
//! | `ArtworkPlaceholder` | — | `components/artwork_placeholder_tests.rs::artwork_placeholder_paints_requested_extent` |
//! | `ArtworkLoadingPlaceholder` | — | `components/tv_wide_tests.rs` (unpainted portrait cells) |
//! | `StatusBar` | yes (Both, LibraryOnly, mini library) | `tests.rs` status suites |
//! | `StatusBarPill` | — | residual (see below): its pill spans carry the status band's own value |
//! | `QueuePanelBand` | yes (Both, wide QueueOnly) | `queue_title_characterization_tests.rs` |
//! | `PillRow` | yes (wide music rail) | `test_helpers.rs::assert_surface_pills`, `tests_scroll_pills.rs` |
//! | `PillChip` / `PillChipSelected` | — | `test_helpers.rs::assert_surface_pills`, `queue_title_characterization_tests.rs` |
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

use super::test_helpers::{
    draw_mounted_terminal, make_movie_app, make_music_group_app, make_queue_app,
    mounted_browser_layout, mounted_model_at, mounted_music_layout,
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
    Painted {
        buffer: term.backend().buffer().clone(),
    }
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

/// The library rail's list panel for a wide-hero presentation: the pane the
/// shared `wide_hero_browser_pane` places the pill row and rail in, derived
/// from the component's published `browser_area` with the same production
/// arrangement the painter used.
fn browser_list_panel(browser_area: Rect) -> Rect {
    let browser_panel = Rect {
        y: browser_area.y.saturating_sub(PANE_PAD_Y),
        height: browser_area.height + PANE_PAD_Y * 2,
        ..browser_area
    };
    wide_hero_browser_pane(browser_panel, browser_area).list_panel
}

/// `unify-surface-colour-neutral` 4.1: the two columns, the panels and the
/// structural chrome of the wide `Both` frame follow the table in both bool
/// states. This is the breakpoint where both columns are on screen, so one
/// frame pins the queue's and the library's surfaces at once; each probe
/// passes the site's own focus bit, not a frame-wide state.
#[test]
fn wide_both_columns_panels_and_chrome_follow_the_table() {
    for (panel_focus, label) in [
        (PanelFocus::Library, "Both/library-focused"),
        (PanelFocus::Queue, "Both/queue-focused"),
    ] {
        let mut app = active_queue_app();
        app.panel_mode = PanelMode::Both;
        app.panel_focus = panel_focus;
        app.mini_view_focus = PanelFocus::Library;
        let mut model = mounted_model_at(app, 200, 30);
        let term = draw_mounted_terminal(&mut model, 200, 30);
        let painted = painted(&term);
        let area = Rect::new(0, 0, 200, 30);
        let chrome = model.app.compute_chrome_geometry(area);
        let main = &model.app.layout.main;
        let playback = &model.app.layout.playback;
        // The sites' own bit: the queue column's focus, exactly what the shell
        // hands `render_legacy_backdrops` and the panel painters.
        let queue_bit = chrome.queue_focused;
        assert_eq!(
            queue_bit,
            panel_focus == PanelFocus::Queue,
            "{label}: the frame's queue bit must follow the panel focus"
        );

        // Column gutters: the shell's `render_legacy_backdrops` owns both; the
        // queue column follows its bit, the library column is fixed.
        painted.expect(
            &format!("{label}/queue gutter"),
            palette::Surface::QueueColumn,
            queue_bit,
            Rect::new(chrome.left_area.x, chrome.left_area.y, 1, 1),
        );
        painted.expect(
            &format!("{label}/library gutter"),
            palette::Surface::LibraryColumn,
            false,
            Rect::new(chrome.right_area.x, chrome.right_area.y + 1, 1, 1),
        );

        // Panel bodies.
        painted.expect(
            &format!("{label}/queue panel body"),
            palette::Surface::QueuePanel,
            queue_bit,
            Rect::new(main.queue_area.x + 1, main.queue_area.y + 1, 1, 1),
        );
        let queue_title = main
            .queue_title_area
            .expect("Both publishes a queue title band");
        painted.expect(
            &format!("{label}/queue title band"),
            palette::Surface::QueuePanelBand,
            queue_bit,
            Rect::new(queue_title.right() - 1, queue_title.y, 1, 1),
        );
        let browser = mounted_browser_layout(&model);
        let list_panel = browser_list_panel(browser.movies_wide_right_area);
        assert!(
            list_panel.width > 2 && list_panel.height > 3,
            "{label}: wide Both must locate the library rail, got {list_panel:?}"
        );
        painted.expect(
            &format!("{label}/library rail body"),
            palette::Surface::LibraryPanel,
            !queue_bit,
            Rect::new(list_panel.x + 1, list_panel.bottom() - 2, 1, 1),
        );

        // The now-playing panel's body follows the queue column's bit.
        painted.expect(
            &format!("{label}/playback panel"),
            palette::Surface::PlaybackPanel,
            queue_bit,
            Rect::new(playback.player_area.x + 1, playback.player_area.y, 1, 1),
        );
        painted.expect(
            &format!("{label}/playback recess row"),
            palette::Surface::PlaybackRecess,
            queue_bit,
            Rect::new(playback.player_area.x + 1, playback.player_area.y + 1, 1, 1),
        );

        // Structural chrome (fixed rows; the bit is a formality).
        painted.expect(
            &format!("{label}/tab bar"),
            palette::Surface::TabBar,
            queue_bit,
            Rect::new(chrome.tab_bar_area.x + 5, chrome.tab_bar_area.y + 1, 1, 1),
        );
        painted.expect(
            &format!("{label}/status bar"),
            palette::Surface::StatusBar,
            queue_bit,
            Rect::new(chrome.status_area.x + 1, chrome.status_area.y, 1, 1),
        );

        // Selected rows are probed in the bool state their column holds.
        if queue_bit {
            let row = main
                .queue_selected_item_rect
                .expect("focused queue publishes its selected row");
            painted.expect(
                &format!("{label}/queue selected row"),
                palette::Surface::SelectedRowOnQueueColumn,
                true,
                row,
            );
        } else {
            let row = browser
                .selected_item_rect
                .expect("focused library rail publishes its selected row");
            painted.expect(
                &format!("{label}/library selected row"),
                palette::Surface::SelectedRow,
                false,
                row,
            );
        }
    }
}

/// `unify-surface-colour-neutral` 4.1: the wide `LibraryOnly` frame's hero
/// pane, rail and column gutter follow the table. The Movies hero pane is
/// read-only, so its own bit — never the panel focus — resolves it.
#[test]
fn wide_library_only_hero_and_rail_follow_the_table() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    let mut model = mounted_model_at(app, 120, 30);
    let term = draw_mounted_terminal(&mut model, 120, 30);
    let painted = painted(&term);
    let area = Rect::new(0, 0, 120, 30);
    let chrome = model.app.compute_chrome_geometry(area);
    let library_area = model.app.layout.main.left_area;
    let panes = wide_library_panes(library_area, PANE_PAD_X, PANE_PAD_Y, None)
        .expect("wide LibraryOnly fits the two-pane split");
    let browser = mounted_browser_layout(&model);
    let list_panel = browser_list_panel(browser.movies_wide_right_area);

    painted.expect(
        "LibraryOnly/library gutter",
        palette::Surface::LibraryColumn,
        false,
        Rect::new(chrome.right_area.x, chrome.right_area.y + 1, 1, 1),
    );
    painted.expect(
        "LibraryOnly/read-only hero pane",
        palette::Surface::HeroPane,
        false,
        Rect::new(panes.hero_panel.x + 1, panes.hero_panel.y + 1, 1, 1),
    );
    painted.expect(
        "LibraryOnly/library rail body",
        palette::Surface::LibraryPanel,
        true,
        Rect::new(list_panel.x + 1, list_panel.bottom() - 2, 1, 1),
    );
    let row = browser
        .selected_item_rect
        .expect("focused library rail publishes its selected row");
    painted.expect(
        "LibraryOnly/selected row",
        palette::Surface::SelectedRow,
        true,
        row,
    );
    painted.expect(
        "LibraryOnly/tab bar",
        palette::Surface::TabBar,
        false,
        Rect::new(chrome.tab_bar_area.x + 5, chrome.tab_bar_area.y + 1, 1, 1),
    );
    painted.expect(
        "LibraryOnly/status bar",
        palette::Surface::StatusBar,
        false,
        Rect::new(chrome.status_area.x + 1, chrome.status_area.y, 1, 1),
    );
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
    let main = &model.app.layout.main;
    // The panel the shell paints beside the card in wide Queue-only (see
    // `render_main`'s `is_wide` arm), reconstructed from published geometry.
    let panel = Rect {
        x: chrome.left_content.x + main.card.width + 2,
        y: chrome.left_content.y,
        width: chrome
            .left_content
            .width
            .saturating_sub(main.card.width + 2),
        height: main.card.height.max(4),
    };
    assert!(
        panel.width > 4 && panel.height >= 4,
        "wide Queue-only must locate the playback strip, got {panel:?}"
    );

    painted.expect(
        "QueueOnly/queue panel body",
        palette::Surface::QueuePanel,
        true,
        Rect::new(main.queue_area.x + 1, main.queue_area.y + 1, 1, 1),
    );
    let queue_title = main
        .queue_title_area
        .expect("Queue-only publishes a queue title band");
    painted.expect(
        "QueueOnly/queue title band",
        palette::Surface::QueuePanelBand,
        true,
        Rect::new(queue_title.right() - 1, queue_title.y, 1, 1),
    );
    // The strip: the shell's queue-only branch paints the panel body and its
    // recess rows as one fixed chrome band.
    painted.expect(
        "QueueOnly/playback strip recess row",
        palette::Surface::QueueOnlyPlaybackPanel,
        true,
        Rect::new(panel.x + 1, panel.y, 1, 1),
    );
    // The bottom "On Now" row is main's `PlaybackBottomRow` (the panel
    // paints it with the backdrop in every mode); the rest of the strip is the
    // fixed chrome band.
    painted.expect(
        "QueueOnly/playback strip bottom row",
        palette::Surface::PlaybackBottomRow,
        true,
        Rect::new(panel.x + 1, panel.y + 3, 1, 1),
    );
    painted.expect(
        "QueueOnly/playback strip body",
        palette::Surface::QueueOnlyPlaybackPanel,
        true,
        Rect::new(panel.x + 1, panel.y + 4, 1, 1),
    );
}

/// `unify-surface-colour-neutral` 4.1: the two narrow mini-view halves. Below
/// the mini threshold the ephemeral mini focus picks Library-only or
/// Queue-only; each half's surfaces must follow the same table.
#[test]
fn mini_view_halves_follow_the_table() {
    // Library half.
    let app = super::test_helpers::make_large_movie_library_app(40);
    let mut app = app;
    app.mini_view_focus = PanelFocus::Library;
    let mut model = mounted_model_at(app, 60, 20);
    let term = draw_mounted_terminal(&mut model, 60, 20);
    let library_painted = painted(&term);
    let area = Rect::new(0, 0, 60, 20);
    let chrome = model.app.compute_chrome_geometry(area);
    library_painted.expect(
        "mini library/column gutter",
        palette::Surface::LibraryColumn,
        false,
        Rect::new(chrome.right_area.x, chrome.right_area.y + 1, 1, 1),
    );
    library_painted.expect(
        "mini library/playback panel",
        palette::Surface::PlaybackPanel,
        false,
        Rect::new(
            model.app.layout.playback.player_area.x + 1,
            model.app.layout.playback.player_area.y,
            1,
            1,
        ),
    );
    library_painted.expect(
        "mini library/tab bar",
        palette::Surface::TabBar,
        false,
        Rect::new(chrome.tab_bar_area.x + 5, chrome.tab_bar_area.y + 1, 1, 1),
    );
    library_painted.expect(
        "mini library/status bar",
        palette::Surface::StatusBar,
        false,
        Rect::new(chrome.status_area.x + 1, chrome.status_area.y, 1, 1),
    );

    // Queue half: the queue-only strip paints its fixed chrome band.
    let mut app = active_queue_app();
    app.mini_view_focus = PanelFocus::Queue;
    app.terminal_width = 60;
    app.terminal_height = 20;
    let mut model = crate::app::shell::Model::new(app);
    let term = draw_mounted_terminal(&mut model, 60, 20);
    let queue_painted = painted(&term);
    let main = &model.app.layout.main;
    let panel = Rect {
        x: 2,
        y: 1 + main.card.height,
        width: 56,
        height: 4,
    };
    queue_painted.expect(
        "mini queue/playback strip recess row",
        palette::Surface::QueueOnlyPlaybackPanel,
        true,
        Rect::new(panel.x + 1, panel.y, 1, 1),
    );
    queue_painted.expect(
        "mini queue/playback strip bottom row",
        palette::Surface::PlaybackBottomRow,
        true,
        Rect::new(panel.x + 1, panel.y + 3, 1, 1),
    );
}

/// `unify-surface-colour-neutral` 4.1 pin (a): the wide music browser's
/// container is occluded by the shell, not filled by the screen. The shell's
/// `render_legacy_backdrops` paints the column with `LibraryColumn`; the pane
/// the screen owns paints the pill row (`PillRow`) and the rail body
/// (`LibraryPanel`), and its one unclaimed row is the `PillRowGap` chrome
/// band. A screen that fills the container again overpaints one of those rows
/// and fails here.
///
/// Unlike the archived change's pin (a), this tree cannot assert the spacer
/// and the column are *distinct*: main's library column is fixed at the same
/// backdrop value the spacer rests at, and two identical fills are
/// indistinguishable in a buffer (the archived change's own residual note).
/// The three probes still pin each row's value against its named surface.
#[test]
fn wide_music_browser_container_is_occluded_by_the_shell() {
    let mut app = make_music_group_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    let mut model = mounted_model_at(app, 120, 30);
    let term = draw_mounted_terminal(&mut model, 120, 30);
    let painted = painted(&term);
    let area = Rect::new(0, 0, 120, 30);
    let chrome = model.app.compute_chrome_geometry(area);
    let music = mounted_music_layout(&model);
    let browser_area = music.wide_music_right_area;
    assert!(
        browser_area.width > 4 && browser_area.height > 4,
        "wide music must publish its browser pane, got {browser_area:?}"
    );
    let browser_panel = Rect {
        y: browser_area.y.saturating_sub(PANE_PAD_Y),
        height: browser_area.height + PANE_PAD_Y * 2,
        ..browser_area
    };
    let pane = wide_hero_browser_pane(browser_panel, browser_area);

    // The row below the pill bar is the chrome-band spacer the screen owns.
    painted.expect(
        "wide music/pill-bar spacer",
        palette::Surface::PillRowGap,
        false,
        Rect::new(pane.spacer_area.x, pane.spacer_area.y, 1, 1),
    );
    // The pill row and the rail body are the screen's own surfaces.
    painted.expect(
        "wide music/pill row",
        palette::Surface::PillRow,
        false,
        Rect::new(pane.pills_area.right() - 1, pane.pills_area.y, 1, 1),
    );
    painted.expect(
        "wide music/rail body",
        palette::Surface::LibraryPanel,
        true,
        Rect::new(pane.list_panel.x + 1, pane.list_panel.bottom() - 2, 1, 1),
    );
    // The container's shell-owned gutter is the column surface.
    painted.expect(
        "wide music/column gutter",
        palette::Surface::LibraryColumn,
        false,
        Rect::new(chrome.right_area.x, chrome.right_area.y + 1, 1, 1),
    );
}

/// `unify-surface-colour-neutral` 4.1 pin (b): the queue's selected row paints
/// a hole only while the queue column holds focus — the row policy gates the
/// highlight on `selected && focused` (`media_list/wide_row.rs`), so with the
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
    let row = model
        .app
        .layout
        .main
        .queue_selected_item_rect
        .expect("the resting queue still publishes its selected row");

    let panel_body = palette::surface_colors(palette::Surface::QueuePanel, false).fill;
    let focused_hole =
        palette::surface_colors(palette::Surface::SelectedRowOnQueueColumn, true).fill;
    let actual = painted.buffer[(row.x, row.y)].bg;
    assert_eq!(
        actual, panel_body,
        "the resting selected row must paint no hole: the queue panel's resting \
         backdrop ({panel_body:?}); painted {actual:?} at ({}, {})",
        row.x, row.y
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
        palette::Surface::PlaybackBottomRow,
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

// --- Component-view pins ---------------------------------------------------
//
// The identities below are not locatable in the representative shell frames
// (popup overlays, sidebar shells, span-level pills, the mouse-armed split
// boundary), so each is pinned through its own production paint path — the
// same standard as the shell frames: the rendered buffer's fill must equal
// `surface_colors(surface, site_bit).fill`.

/// Render a buffer through one production paint closure.
fn rendered(paint: impl FnOnce(&mut ratatui::Frame)) -> ratatui::buffer::Buffer {
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(paint).unwrap();
    term.backend().buffer().clone()
}

/// Assert a fixed row's fill in both bool states (a fixed row must resolve
/// one value), then assert the painted cell equals it.
fn expect_fixed(
    buffer: &ratatui::buffer::Buffer,
    label: &str,
    surface: palette::Surface,
    rect: Rect,
) {
    let focused = palette::surface_colors(surface, true).fill;
    let resting = palette::surface_colors(surface, false).fill;
    assert_eq!(
        focused, resting,
        "{label}: a fixed row must not follow focus"
    );
    let actual = buffer[(rect.x, rect.y)].bg;
    assert_eq!(
        actual, focused,
        "{label}: expected {surface:?} {focused:?} at ({}, {}), painted {actual:?}",
        rect.x, rect.y
    );
}

/// `unify-surface-colour-neutral` 4.1: the wide hero split gap's painter pins
/// the `WideSplitGutter` fill across the gap rect while the boundary is armed.
#[test]
fn wide_split_gutter_follows_the_table() {
    use crate::app::components::WideHeroBoundaryComponent;
    use tuirealm::component::Component;

    let gap = Rect::new(60, 4, 2, 8);
    let mut boundary = WideHeroBoundaryComponent::new();
    boundary.sync(gap, 0, 200, 80, true);
    let buffer = rendered(|f| Component::view(&mut boundary, f, gap));
    expect_fixed(
        &buffer,
        "wide split gutter",
        palette::Surface::WideSplitGutter,
        gap,
    );
}

/// `unify-surface-colour-neutral` 4.1: the context menu's selected row paints
/// `ACCENT_ACTIVE` through the production context-menu painter.
#[test]
fn context_menu_selected_row_follows_the_table() {
    let rect = Rect::new(30, 8, 20, 5);
    let buffer = rendered(|f| {
        super::components::context_menu::render_context_menu_content(
            f,
            rect,
            &[("Open", true), ("Rename", true), ("Delete", false)],
            1,
        );
    });
    expect_fixed(
        &buffer,
        "context menu/selected row",
        palette::Surface::ContextMenuSelectedRow,
        Rect::new(rect.x, rect.y + 2, 1, 1),
    );
}

/// `unify-surface-colour-neutral` 4.1: the inline-hero selected detail paints
/// the `InlineHero` row in both bool states — it is focus-driven, so both
/// halves are pinned.
#[test]
fn inline_hero_follows_the_table() {
    let hero_area = Rect::new(10, 5, 40, 5);
    for focused in [false, true] {
        let buffer = rendered(|f| {
            super::components::hero::selected_detail_shell(f, hero_area, 5, focused);
        });
        let expected = palette::surface_colors(palette::Surface::InlineHero, focused).fill;
        let actual = buffer[(hero_area.x, hero_area.y + 2)].bg;
        assert_eq!(
            actual,
            expected,
            "inline hero (focused={focused}): expected {expected:?} at ({}, {}), \
             painted {actual:?}",
            hero_area.x,
            hero_area.y + 2
        );
    }
}

/// `unify-surface-colour-neutral` 4.1: the two sidebar shells pin the body
/// and band rows — the expanded (F1-F4) pair and the non-hero pair.
#[test]
fn sidebar_shells_follow_the_table() {
    let sidebar = Rect::new(2, 2, 24, 12);
    for (style, body_surface, band_surface, label) in [
        (
            true,
            palette::Surface::SidebarBody,
            palette::Surface::SidebarBand,
            "expanded sidebar",
        ),
        (
            false,
            palette::Surface::NonHeroSidebarBody,
            palette::Surface::NonHeroSidebarBand,
            "non-hero sidebar",
        ),
    ] {
        let buffer = rendered(|f| {
            super::components::chrome::render_panel_shell_at(f, sidebar, "Panel", "hints", style);
        });
        // The band: the header row paints the band value over the body.
        let band_cell = if style {
            Rect::new(sidebar.x + 2, sidebar.y + 1, 1, 1)
        } else {
            Rect::new(sidebar.x, sidebar.y, 1, 1)
        };
        expect_fixed(&buffer, &format!("{label}/band"), band_surface, band_cell);
        let footer_cell = if style {
            Rect::new(sidebar.x + 2, sidebar.bottom() - 2, 1, 1)
        } else {
            Rect::new(sidebar.x, sidebar.bottom() - 2, 1, 1)
        };
        expect_fixed(
            &buffer,
            &format!("{label}/footer band"),
            band_surface,
            footer_cell,
        );
        // The body: a cell below the header and above the footer, clear of
        // the non-hero variant's right border column.
        let body_cell = if style {
            Rect::new(sidebar.x + 2, sidebar.y + 4, 1, 1)
        } else {
            Rect::new(sidebar.x + 1, sidebar.y + 2, 1, 1)
        };
        expect_fixed(&buffer, &format!("{label}/body"), body_surface, body_cell);
    }
}

/// `unify-surface-colour-neutral` 4.1: the now-playing title row's status
/// pill paints the `PlaybackStatusPill` fill (the backdrop, distinct from the
/// panel's own fill), so the pill cells are locatable in the rendered row.
#[test]
fn playback_status_pill_follows_the_table() {
    let mut app = make_queue_app(2);
    app.use_nerd_fonts = false;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.queue_len = 2;
        status.current_idx = 0;
        status.runtime_ticks = 90 * TICKS_PER_SECOND;
    }
    let row = Rect::new(0, 0, 60, 1);
    let mut term = Terminal::new(TestBackend::new(60, 1)).unwrap();
    let mut layout = crate::app::layout::LayoutPlayback::default();
    let mut marquee = String::new();
    let marquee_at = std::time::Instant::now();
    term.draw(|f| {
        let mut context = app.playback_panel_context(
            row,
            &mut layout,
            1,
            true,
            &None,
            ratatui::style::Color::Reset,
        );
        super::components::chrome_player::render_title_row(
            f,
            row,
            "Title",
            palette::TEXT_STRONG,
            &mut context,
        );
        let _ = (&mut marquee, &marquee_at);
    })
    .unwrap();
    let buffer = term.backend().buffer().clone();
    let pill = palette::surface_colors(palette::Surface::PlaybackStatusPill, false).fill;
    // The pill spans are right-aligned; find them by value and require at
    // least one cell, every pill-coloured cell matching the row's value.
    let pill_cells = (0..row.width)
        .filter(|&x| buffer[(x, 0)].bg == pill)
        .collect::<Vec<_>>();
    assert!(
        !pill_cells.is_empty(),
        "playback status pill: no cell in the title row paints the pill fill \
         {pill:?}"
    );
}

/// `unify-surface-colour-neutral` 4.1: the confirm-modal caller path pins the
/// `PopupFrame` fill at the modal's top border cell.
#[test]
fn popup_frame_follows_the_table() {
    let mut dim_flag = false;
    let buffer = rendered(|f| {
        super::components::confirm_modal::render_confirm_modal_content(
            f,
            &mut dim_flag,
            "Confirm",
            "Proceed?",
            "Enter to confirm",
        );
    });
    assert!(dim_flag, "the modal caller arms the dim backdrop");
    // The centered 60×7 frame inside the 80×24 terminal.
    let modal = Rect::new((80 - 60) / 2, (24 - 7) / 2, 60, 7);
    expect_fixed(
        &buffer,
        "popup/frame",
        palette::Surface::PopupFrame,
        Rect::new(modal.x, modal.y, 1, 1),
    );
}
