//! `unify-surface-colour` D4 / row 5.2: the surface conformance test.
//!
//! The theme's surface table (`theme::surface_table`, `theme::surface_resolve`)
//! is the only home of a colour decision, and row 5.1 made naming a role in a
//! screen a build failure. This test closes the remaining gap: it renders a
//! representative frame at each breakpoint the change cares about through the
//! real shell paint path and asserts that the pixels a surface region actually
//! paints equal `surface_colors(surface, focus).fill` for the surface the
//! inventory (`design.md`, task 2.1) assigns that region. A wrong or missing
//! painter therefore fails the suite, not just the table's own unit test.
//!
//! # Expectations come from the table
//!
//! Every probe calls the production resolver — `palette::surface_colors` for a
//! frame focus, `palette::surface_colors_for_column_focus` for a surface whose
//! painter is handed its own column bit (`LeftPaneFocus::ReadOnly`, or a
//! collapsed component focus) — with the surface identity the inventory gives
//! that region. No colour literal is duplicated: a role edit moves the expected
//! value and the painted value together, while a painter edited to another
//! level's colour separates them and fails. The only hand-written values are
//! the surface *identities* (the inventory mapping) and the sample rect, which
//! is exactly what this test exists to check.
//!
//! # Breaks the test does not cover, and why
//!
//! The test renders one representative frame per breakpoint and probes only the
//! regions that frame locates. A surface absent from that frame is not silently
//! skipped: the coverage table below names every `Surface::ALL` row and says
//! where it is pinned, by this test or by an existing one, or why a buffer-level
//! pin is impossible. "—" means this test does not probe it; the named test is
//! the pin.
//!
//! | `Surface` | pinned here | otherwise pinned by |
//! | --- | --- | --- |
//! | `QueueColumn` | yes (wide Both) | — (pinned only here at buffer level) |
//! | `LibraryColumn` | yes (Both, LibraryOnly, mini library) | `tests_library_characterization.rs::wide_library_column_gutter_follows_panel_focus` |
//! | `WideSplitGutter` | — | `components::wide_hero_boundary` component tests; painted only while the split boundary is mouse-eligible |
//! | `HeroPane` | yes (LibraryOnly Movies, `ReadOnly`) | `tests_wide_hero_pane_characterization.rs` |
//! | `SelectedRow` | yes (LibraryOnly library rail) | `tests_library_characterization.rs::wide_movies_selected_row_punches_through_to_the_library_backdrop` |
//! | `SelectedRowOnQueueColumn` | yes (wide Both queue focused; pin (b) resting row) | `components::media_list` resolver test |
//! | `SelectedRowOnLibraryPane` | — | `tests_music_wide.rs` / `tests_tv_workspace` selected-row suites |
//! | `ContextMenuSelectedRow` | — | `tests_context_menu.rs` |
//! | `LibraryPanel` | yes (Both, wide LibraryOnly) | wide-hero rail tests in `tests_library_characterization.rs` |
//! | `QueuePanel` | yes (Both, QueueOnly) | `tests_queue.rs` |
//! | `MainContentBox` | — | `tests_music_wide.rs`, `tests_audiobookshelf_podcasts.rs` (soft variant) |
//! | `InlineHero` | — | `tests_home_inline.rs`, `tests_audiobookshelf_podcasts.rs` |
//! | `PlaybackPanel` | yes (Both, wide QueueOnly, mini library) | `tests.rs::wide_panel_bottom_row_follows_the_library_column_surface` |
//! | `SidebarBody` / `NonHeroSidebarBody` | — | `tests_help.rs` |
//! | `QueueCardVisualizer` | — | `components::visualizer` unit tests; the card rect is component-local |
//! | `PlaybackRecess` | yes (QueueOnly, mini queue) | `tests.rs` Queue-only panel tests |
//! | `PlaybackBottomRow` | yes (QueueOnly, mini queue) | `tests.rs::narrow_queue_only_panel_puts_title_on_bottom_now_playing_row` |
//! | `PlaybackStatusPill` | — | `components::chrome_player` pill tests; the pill is a sub-rect of the title row |
//! | `ArtworkPlaceholder` / `ArtworkLoadingPlaceholder` | — | `components::artwork_placeholder` / artwork-loading painters |
//! | `StatusBar` | yes (Both, LibraryOnly, mini library) | `tests.rs` status suites |
//! | `StatusBarPill` | — | `components::chrome_status` pill tests; a sub-rect of the status row |
//! | `QueuePanelBand` | yes (Both, wide QueueOnly) | `queue_title_characterization_tests.rs` |
//! | `PillRow` | yes (wide music rail) | `test_helpers.rs::assert_surface_pills` |
//! | `PillChip` / `PillChipSelected` | — | `test_helpers.rs::assert_surface_pills`, `tests_search_sidebar.rs` |
//! | `QueueScopePillSelected` | — | `components::queue` scope-pill tests |
//! | `PillRowGap` | yes (wide music container pin) | `components::music_workspace_cursor_tests.rs::music_wide_pill_row_spacer_is_the_chrome_band_surface` |
//! | `SidebarBand` / `NonHeroSidebarBand` | — | `tests_help.rs` |
//! | `TabBar` | yes (Both, LibraryOnly, mini library) | `tests.rs` tab-bar suites |
//! | `PopupFrame` / `PopupDimBackdrop` | — | `tests_confirm_modal.rs` and the other popup suites |
//!
//! The 15 surfaces this test does probe, and the breakpoints that pin them:
//! `LibraryColumn` (Both, LibraryOnly, mini library), `QueueColumn`,
//! `QueuePanel`, `QueuePanelBand`, `LibraryPanel`, `PlaybackPanel`, `StatusBar`,
//! `TabBar` (Both); `HeroPane`, `SelectedRow`, `LibraryPanel`, `StatusBar`,
//! `TabBar` (wide LibraryOnly); `QueuePanel`, `QueuePanelBand`,
//! `PlaybackPanel`, `PlaybackRecess`, `PlaybackBottomRow` (wide QueueOnly);
//! `PlaybackPanel`, `StatusBar`, `TabBar` (mini library); `PlaybackRecess`,
//! `PlaybackBottomRow` (mini queue); `PillRow`, `PillRowGap` (wide music rail);
//! `SelectedRowOnQueueColumn` (wide Both, queue focused, plus the pin (b)
//! resting-row check).
//!
//! Where this module overlaps an existing per-surface test (the wide music
//! pill-bar spacer, the wide library column gutter, the queue framing rows),
//! both are kept: the existing test pins the component in isolation, this one
//! pins the same row through the full shell frame, so a composition regression
//! between the two still fails.
//!
//! Two surfaces are pinned **nowhere at buffer level**: `WideSplitGutter` and
//! `QueueCardVisualizer` (the two boundary components paint from
//! shell-derived rects that are only armed while mouse eligibility holds; the
//! visualizer background is painted by a component-local rect). Their painters
//! are covered by component unit tests, but no end-to-end frame asserts the
//! rendered fill. That is an honest gap, not a hidden one.

use super::test_helpers::*;
use super::*;
use crate::app::layout::FocusState;
use crate::app::{PanelFocus, PanelMode};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

/// A rendered frame plus the one focus state its surfaces resolve against.
struct Painted {
    buffer: ratatui::buffer::Buffer,
    focus: FocusState,
}

impl Painted {
    /// Assert the cell at `rect`'s top-left paints `surface` resolved from the
    /// frame's focus state — the production `surface_colors` entry point.
    fn expect(&self, label: &str, surface: palette::Surface, rect: Rect) {
        let expected = palette::surface_colors(surface, &self.focus).fill;
        let actual = self.buffer[(rect.x, rect.y)].bg;
        assert_eq!(
            actual, expected,
            "{label}: expected {surface:?} {:?} at ({}, {}), painted {actual:?} \
             (frame focus {:?}, rect {rect:?})",
            expected, rect.x, rect.y, self.focus
        );
    }

    /// Assert a surface whose painter was handed only its own column's focus
    /// (`LeftPaneFocus::ReadOnly`, or a component's collapsed focus bit) paints
    /// that resolution — the production `surface_colors_for_column_focus` entry
    /// point.
    fn expect_own(&self, label: &str, surface: palette::Surface, owned_focused: bool, rect: Rect) {
        let expected = palette::surface_colors_for_column_focus(surface, owned_focused).fill;
        let actual = self.buffer[(rect.x, rect.y)].bg;
        assert_eq!(
            actual, expected,
            "{label}: expected {surface:?} ({owned_focused} own focus) {expected:?} at \
             ({}, {}), painted {actual:?} (rect {rect:?})",
            rect.x, rect.y
        );
    }
}

fn painted(model: &crate::app::shell::Model, term: &Terminal<TestBackend>) -> Painted {
    Painted {
        buffer: term.backend().buffer().clone(),
        focus: model.app.focus_state(),
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

/// `unify-surface-colour` 5.2: the two columns, the panels, and the structural
/// chrome of the wide `Both` frame follow the table in both focus states. This
/// is the breakpoint where both columns are on screen, so one frame pins the
/// queue's and the library's surfaces at once.
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
        let painted = painted(&model, &term);
        let area = Rect::new(0, 0, 200, 30);
        let chrome = model.app.compute_chrome_geometry(area);
        let main = &model.app.layout.main;
        let playback = &model.app.layout.playback;

        // Column gutters: the shell's `render_legacy_backdrops` owns both.
        painted.expect(
            &format!("{label}/library gutter"),
            palette::Surface::LibraryColumn,
            Rect::new(chrome.right_area.x, chrome.right_area.y + 1, 1, 1),
        );
        painted.expect(
            &format!("{label}/queue gutter"),
            palette::Surface::QueueColumn,
            Rect::new(chrome.left_area.x, chrome.left_area.y, 1, 1),
        );

        // Panel bodies.
        painted.expect(
            &format!("{label}/queue panel body"),
            palette::Surface::QueuePanel,
            Rect::new(main.queue_area.x + 1, main.queue_area.y + 1, 1, 1),
        );
        let queue_title = main
            .queue_title_area
            .expect("Both publishes a queue title band");
        painted.expect(
            &format!("{label}/queue title band"),
            palette::Surface::QueuePanelBand,
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
            Rect::new(list_panel.x + 1, list_panel.bottom() - 2, 1, 1),
        );

        // The now-playing panel's own content rows, and the row above the pill
        // bar, which belongs to the library column (design D5).
        painted.expect(
            &format!("{label}/playback panel"),
            palette::Surface::PlaybackPanel,
            Rect::new(playback.player_area.x + 1, playback.player_area.y, 1, 1),
        );
        painted.expect(
            &format!("{label}/row above pill bar"),
            palette::Surface::LibraryColumn,
            Rect::new(playback.player_area.x + 1, playback.player_area.y + 3, 1, 1),
        );

        // Structural chrome.
        painted.expect(
            &format!("{label}/tab bar"),
            palette::Surface::TabBar,
            Rect::new(chrome.tab_bar_area.x + 5, chrome.tab_bar_area.y + 1, 1, 1),
        );
        painted.expect(
            &format!("{label}/status bar"),
            palette::Surface::StatusBar,
            Rect::new(chrome.status_area.x + 1, chrome.status_area.y, 1, 1),
        );

        // Selected rows only paint a hole while their column holds focus, so
        // each is probed in the focus state that makes it observable.
        if panel_focus == PanelFocus::Library {
            let row = browser
                .selected_item_rect
                .expect("focused library rail publishes its selected row");
            painted.expect(
                &format!("{label}/library selected row"),
                palette::Surface::SelectedRow,
                row,
            );
        } else {
            let row = main
                .queue_selected_item_rect
                .expect("focused queue publishes its selected row");
            painted.expect(
                &format!("{label}/queue selected row"),
                palette::Surface::SelectedRowOnQueueColumn,
                row,
            );
        }
    }
}

/// `unify-surface-colour` 5.2: the wide `LibraryOnly` frame's hero pane, rail
/// and column gutter follow the table. The Movies hero pane is `ReadOnly`, so
/// its own focus bit — never the frame's — resolves it.
#[test]
fn wide_library_only_hero_and_rail_follow_the_table() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    let mut model = mounted_model_at(app, 120, 30);
    let term = draw_mounted_terminal(&mut model, 120, 30);
    let painted = painted(&model, &term);
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
        Rect::new(chrome.right_area.x, chrome.right_area.y + 1, 1, 1),
    );
    painted.expect_own(
        "LibraryOnly/read-only hero pane",
        palette::Surface::HeroPane,
        false,
        Rect::new(panes.hero_panel.x + 1, panes.hero_panel.y + 1, 1, 1),
    );
    painted.expect(
        "LibraryOnly/library rail body",
        palette::Surface::LibraryPanel,
        Rect::new(list_panel.x + 1, list_panel.bottom() - 2, 1, 1),
    );
    let row = browser
        .selected_item_rect
        .expect("focused library rail publishes its selected row");
    painted.expect(
        "LibraryOnly/selected row",
        palette::Surface::SelectedRow,
        row,
    );
    painted.expect(
        "LibraryOnly/tab bar",
        palette::Surface::TabBar,
        Rect::new(chrome.tab_bar_area.x + 5, chrome.tab_bar_area.y + 1, 1, 1),
    );
    painted.expect(
        "LibraryOnly/status bar",
        palette::Surface::StatusBar,
        Rect::new(chrome.status_area.x + 1, chrome.status_area.y, 1, 1),
    );
}

/// `unify-surface-colour` 5.2: the wide Queue-only strip is the table's one
/// mode-driven appearance, and its bottom "On Now" row is the Mini-only
/// `PlaybackBottomRow` recess.
#[test]
fn queue_only_strip_and_queue_follow_the_table() {
    let mut app = active_queue_app();
    app.panel_mode = PanelMode::QueueOnly;
    app.panel_focus = PanelFocus::Queue;
    app.mini_view_focus = PanelFocus::Queue;
    let mut model = mounted_model_at(app, 120, 30);
    let term = draw_mounted_terminal(&mut model, 120, 30);
    let painted = painted(&model, &term);
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
        Rect::new(main.queue_area.x + 1, main.queue_area.y + 1, 1, 1),
    );
    let queue_title = main
        .queue_title_area
        .expect("Queue-only publishes a queue title band");
    painted.expect(
        "QueueOnly/queue title band",
        palette::Surface::QueuePanelBand,
        Rect::new(queue_title.right() - 1, queue_title.y, 1, 1),
    );
    painted.expect(
        "QueueOnly/playback panel below the painted rows",
        palette::Surface::PlaybackPanel,
        Rect::new(panel.x + 1, panel.y + 4, 1, 1),
    );
    painted.expect(
        "QueueOnly/playback recess",
        palette::Surface::PlaybackRecess,
        Rect::new(panel.x + 1, panel.y, 1, 1),
    );
    painted.expect(
        "QueueOnly/bottom now-playing row",
        palette::Surface::PlaybackBottomRow,
        Rect::new(panel.x + 1, panel.y + 3, 1, 1),
    );
}

/// `unify-surface-colour` 5.2: the two narrow mini-view halves. Below
/// `MINI_VIEW_THRESHOLD` the ephemeral mini focus picks Library-only or
/// Queue-only; each half's surfaces must follow the same table.
#[test]
fn mini_view_halves_follow_the_table() {
    // Library half.
    let mut app = make_large_movie_library_app(40);
    app.mini_view_focus = PanelFocus::Library;
    let mut model = mounted_model_at(app, 60, 20);
    let term = draw_mounted_terminal(&mut model, 60, 20);
    let library_painted = painted(&model, &term);
    let area = Rect::new(0, 0, 60, 20);
    let chrome = model.app.compute_chrome_geometry(area);
    library_painted.expect(
        "mini library/column gutter",
        palette::Surface::LibraryColumn,
        Rect::new(chrome.right_area.x, chrome.right_area.y + 1, 1, 1),
    );
    library_painted.expect(
        "mini library/playback panel",
        palette::Surface::PlaybackPanel,
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
        Rect::new(chrome.tab_bar_area.x + 5, chrome.tab_bar_area.y + 1, 1, 1),
    );
    library_painted.expect(
        "mini library/status bar",
        palette::Surface::StatusBar,
        Rect::new(chrome.status_area.x + 1, chrome.status_area.y, 1, 1),
    );

    // Queue half.
    let mut app = active_queue_app();
    app.mini_view_focus = PanelFocus::Queue;
    app.terminal_width = 60;
    app.terminal_height = 20;
    let mut model = crate::app::shell::Model::new(app);
    let term = draw_mounted_terminal(&mut model, 60, 20);
    let queue_painted = painted(&model, &term);
    let main = &model.app.layout.main;
    let panel = Rect {
        x: 2,
        y: 1 + main.card.height,
        width: 56,
        height: 4,
    };
    queue_painted.expect(
        "mini queue/playback recess",
        palette::Surface::PlaybackRecess,
        Rect::new(panel.x + 1, panel.y, 1, 1),
    );
    queue_painted.expect(
        "mini queue/bottom now-playing row",
        palette::Surface::PlaybackBottomRow,
        Rect::new(panel.x + 1, panel.y + 3, 1, 1),
    );
}

/// `unify-surface-colour` 5.2 pin (a): the wide music browser's container is
/// occluded by the shell, not filled by the screen. The shell's
/// `render_legacy_backdrops` paints the column with `LibraryColumn`; the pane
/// the screen owns paints the pill row (`PillRow`) and the rail body
/// (`LibraryPanel`), and its one unclaimed row is the `PillRowGap` chrome
/// band. A screen that fills the container again overpaints one of those rows
/// and fails here.
#[test]
fn wide_music_browser_container_is_occluded_by_the_shell() {
    let mut app = make_music_group_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    let mut model = mounted_model_at(app, 120, 30);
    let term = draw_mounted_terminal(&mut model, 120, 30);
    let painted = painted(&model, &term);
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
        Rect::new(pane.spacer_area.x, pane.spacer_area.y, 1, 1),
    );
    // The pill row and the rail body are the screen's own surfaces.
    painted.expect(
        "wide music/pill row",
        palette::Surface::PillRow,
        Rect::new(pane.pills_area.right() - 1, pane.pills_area.y, 1, 1),
    );
    painted.expect(
        "wide music/rail body",
        palette::Surface::LibraryPanel,
        Rect::new(pane.list_panel.x + 1, pane.list_panel.bottom() - 2, 1, 1),
    );
    // The container's shell-owned gutter is the column surface, and the
    // focused column is a *different* value from the spacer: if a screen
    // re-filled the container with the column surface the spacer check above
    // would fail, and this asserts the two rows are genuinely distinct.
    painted.expect(
        "wide music/column gutter",
        palette::Surface::LibraryColumn,
        Rect::new(chrome.right_area.x, chrome.right_area.y + 1, 1, 1),
    );
    let spacer = palette::surface_colors(palette::Surface::PillRowGap, &painted.focus).fill;
    let column = palette::surface_colors(palette::Surface::LibraryColumn, &painted.focus).fill;
    assert_ne!(
        spacer, column,
        "the focused library column and the spacer must be distinct, or the \
         container-occlusion pin is vacuous"
    );
}

/// `unify-surface-colour` 5.2 pin (b): an unfocused selected row paints no
/// hole at all, which makes `selected_row_surface_color`'s parameter
/// value-neutral while the row's column rests. The test accepts either the
/// current no-hole paint *or* a future hole painted from the containing
/// column's focus, and fails if the row's colour is instead driven by a cursor
/// bit (the row being selected) rather than its column's focus.
#[test]
fn unfocused_selected_row_paints_no_hole_or_the_columns_focus() {
    let mut app = active_queue_app();
    app.panel_mode = PanelMode::Both;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    let mut model = mounted_model_at(app, 200, 30);
    let term = draw_mounted_terminal(&mut model, 200, 30);
    let painted = painted(&model, &term);
    assert!(
        !model.app.focus_state().queue_column_focused(),
        "the queue must rest for this pin"
    );
    let row = model
        .app
        .layout
        .main
        .queue_selected_item_rect
        .expect("the resting queue still publishes its selected row");

    let panel_body = palette::surface_colors(palette::Surface::QueuePanel, &painted.focus).fill;
    let hole_from_column =
        palette::surface_colors(palette::Surface::SelectedRowOnQueueColumn, &painted.focus).fill;
    let hole_from_cursor = palette::surface_colors(
        palette::Surface::SelectedRowOnQueueColumn,
        &crate::app::layout::FocusState::queue_focused_for_test(),
    )
    .fill;
    let actual = painted.buffer[(row.x, row.y)].bg;
    assert!(
        actual == panel_body || actual == hole_from_column,
        "the resting selected row must paint no hole ({panel_body:?}) or the \
         containing column's resting hole ({hole_from_column:?}); painted \
         {actual:?} at ({}, {})",
        row.x,
        row.y
    );
    assert_ne!(
        actual, hole_from_cursor,
        "the selected row's colour must follow its containing column's focus, \
         not a cursor bit"
    );
}

/// `unify-surface-colour` 5.2: every table row is either probed by this module
/// (see the module coverage table) or explicitly accounted for. This test fails
/// if a new `Surface` variant is added without the module documentation being
/// revisited, so a silent, unpinned surface cannot appear.
#[test]
fn coverage_table_accounts_for_every_surface_row() {
    // Every identity the conformance module documents. Kept as a list rather
    // than reading the private table, because the point is to force a human
    // decision when a row is added.
    let accounted: &[palette::Surface] = &[
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
        palette::Surface::SidebarBody,
        palette::Surface::NonHeroSidebarBody,
        palette::Surface::QueueCardVisualizer,
        palette::Surface::PlaybackRecess,
        palette::Surface::PlaybackBottomRow,
        palette::Surface::PlaybackStatusPill,
        palette::Surface::ArtworkPlaceholder,
        palette::Surface::ArtworkLoadingPlaceholder,
        palette::Surface::StatusBar,
        palette::Surface::StatusBarPill,
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
        palette::Surface::PopupDimBackdrop,
    ];
    for &surface in palette::Surface::ALL {
        assert!(
            accounted.contains(&surface),
            "{surface:?} is not accounted for in this module's coverage table"
        );
    }
    for &surface in accounted {
        assert!(
            palette::Surface::ALL.contains(&surface),
            "{surface:?} is documented but not a declared surface"
        );
    }
}
