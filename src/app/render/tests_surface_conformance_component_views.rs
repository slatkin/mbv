//! Component-view half of the surface conformance test
//! (`unify-surface-colour-neutral` task 4.1).
//!
//! The identities pinned here are not locatable in the representative shell
//! frames of `tests_surface_conformance` (popup overlays, sidebar shells,
//! span-level pills, the mouse-armed split boundary), so each is pinned
//! through its own production paint path — the same standard as the shell
//! frames: the rendered buffer's fill must equal
//! `surface_colors(surface, site_bit).fill`, fixed rows in both bool states
//! and focus-driven ones per bit. The coverage table and the guard test live
//! in the shell-frame half of the module.

use super::*;
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

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
