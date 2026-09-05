// §3.1 / §3.2 evidence for the wide grouped-Music canonical migration.
//
// Wide grouped Music now composes the canonical `WideMediaList` control
// (`render_wide_media_list`), exactly as the wide TV series rail and wide
// Movies list do. These tests drive `MusicWorkspaceComponent::view` directly
// at the wide breakpoint (mirroring `render_music_component` in
// `tests_conformance_matrix`) and assert the canonical row geometry,
// non-selectable structural rows, the selected-row full-width background, and
// that exactly one wide list painter runs for the destination.

use super::components::media_list::{PLAIN_ROWS_PAINTS, WIDE_MEDIA_LIST_PAINTS};
use super::test_helpers::{buffer_to_string, make_music_group_app};
use super::*;
use crate::app::components::MusicWorkspaceComponent;
use crate::app::layout::LibraryRowTarget;
use crate::app::tests::make_item;
use crate::app::PanelFocus;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::Component;

const W: u16 = 160;
const H: u16 = 40;

/// A grouped-Music fixture with two artist groups so the row flow contains a
/// heading, a spacer between groups, and a second heading.
fn multi_artist_app() -> App {
    let mut app = make_music_group_app();
    app.panel_focus = PanelFocus::Library;
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    for i in 1..4 {
        let mut album = make_item(&format!("Alpha Album {i:02}"), "MusicAlbum");
        album.id = format!("alpha-{i}");
        album.artist = "Alpha".into();
        level.items.push(album);
    }
    for i in 0..3 {
        let mut album = make_item(&format!("Beta Album {i:02}"), "MusicAlbum");
        album.id = format!("beta-{i}");
        album.artist = "Beta".into();
        level.items.push(album);
    }
    level.total_count = level.items.len();
    app
}

fn render_wide(
    app: &App,
    focused: bool,
    cursor: usize,
) -> (Terminal<TestBackend>, MusicWorkspaceComponent) {
    let lib_idx = app.tab.emby_library_index().unwrap();
    let mut context = app.wide_music_render_ctx(lib_idx, None);
    context.focused = focused;
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context);
    component.set_focused(focused);
    component.re_anchor(cursor, 0);
    let mut terminal = Terminal::new(TestBackend::new(W, H)).unwrap();
    terminal
        .draw(|f| component.view(f, Rect::new(0, 0, W, H)))
        .unwrap();
    (terminal, component)
}

#[test]
fn wide_music_composes_the_canonical_control_exactly_once() {
    let app = multi_artist_app();
    WIDE_MEDIA_LIST_PAINTS.with(|c| c.set(0));
    PLAIN_ROWS_PAINTS.with(|c| c.set(0));

    let (terminal, _component) = render_wide(&app, true, 0);

    assert_eq!(
        WIDE_MEDIA_LIST_PAINTS.with(std::cell::Cell::get),
        1,
        "the populated album rail uses the canonical painter"
    );
    assert_eq!(
        PLAIN_ROWS_PAINTS.with(std::cell::Cell::get),
        0,
        "the bespoke / plain-rows underpaint must not run"
    );
    assert!(buffer_to_string(&terminal).contains("First Album"));
}

#[test]
fn wide_music_search_mode_uses_the_plain_rows_painter_not_the_wide_control() {
    // Search mode is a separate, already-canonical path (`render_plain_rows`);
    // it must stay working and must not double-paint the grouped control.
    let app = multi_artist_app();
    let lib_idx = app.tab.emby_library_index().unwrap();
    let mut context = app.wide_music_render_ctx(lib_idx, None);
    context.list = context.list.with_search("al".into(), false);
    WIDE_MEDIA_LIST_PAINTS.with(|c| c.set(0));
    PLAIN_ROWS_PAINTS.with(|c| c.set(0));
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context);
    let mut terminal = Terminal::new(TestBackend::new(W, H)).unwrap();
    terminal
        .draw(|f| component.view(f, Rect::new(0, 0, W, H)))
        .unwrap();

    assert_eq!(PLAIN_ROWS_PAINTS.with(std::cell::Cell::get), 1);
    assert_eq!(WIDE_MEDIA_LIST_PAINTS.with(std::cell::Cell::get), 0);
}

#[test]
fn wide_music_headings_and_spacers_are_not_selectable_row_targets() {
    let app = multi_artist_app();
    let (_terminal, component) = render_wide(&app, true, 0);
    let layout = component.layout();

    let album_rows = layout
        .left_row_targets
        .iter()
        .filter(|t| matches!(t, Some(LibraryRowTarget::Album(_))))
        .count();
    let structural_rows = layout
        .left_row_targets
        .iter()
        .filter(|t| t.is_none())
        .count();
    assert_eq!(album_rows, 7, "seven album rows are selectable targets");
    assert!(
        structural_rows >= 2,
        "the two artist headings (and the inter-group spacer) publish no target: {:?}",
        layout.left_row_targets
    );
    // The first painted row is the "Alpha" heading -> no target.
    assert!(layout.left_row_targets[0].is_none());
}

#[test]
fn wide_music_selected_row_fills_the_whole_panel_width_when_focused() {
    let app = multi_artist_app();
    let (terminal, component) = render_wide(&app, true, 0);
    let layout = component.layout();
    let rect = layout
        .selected_item_rect
        .expect("selected-row rect published");
    let buffer = terminal.backend().buffer();
    // The canonical rail paints into the full panel row: `x` is 2 columns
    // (`PANE_PAD_X`) left of the padded content rect the layout publishes, so
    // the flush edge marker and the selected background reach the panel
    // border, and the title lands at the padded content edge (one column
    // left of the old bespoke `padded_rect` + extra leading space).
    let paint_x = rect.x - 2;
    assert_eq!(
        buffer[(paint_x, rect.y)].symbol(),
        "\u{258e}",
        "flush edge marker sits at the full-panel paint x"
    );
    assert_eq!(
        buffer[(rect.x, rect.y)].symbol(),
        "F",
        "selected album title lands at the padded content edge"
    );
    let bg = buffer[(rect.x + 4, rect.y)].bg;
    for x in paint_x..rect.x + rect.width {
        assert_eq!(
            buffer[(x, rect.y)].bg,
            bg,
            "selected-row background fills the full row at x={x}"
        );
    }
    // The row directly below (an ordinary album row) is not filled.
    assert_ne!(
        buffer[(rect.x + 4, rect.y + 1)].bg,
        bg,
        "only the selected row carries the highlight background"
    );
}

#[test]
fn wide_music_left_edge_alignment_matches_the_canonical_left_inset() {
    let app = multi_artist_app();
    let (terminal, component) = render_wide(&app, true, 0);
    let layout = component.layout();
    let buffer = terminal.backend().buffer();
    let area = layout.wide_music_browser_area;
    // Row 0 is the "Alpha" heading. Canonical non-selected row:
    // `[space][space][text]` painted from 2 columns left of `area.x`, so the
    // heading text lands exactly at `area.x` (the padded content edge).
    let first_text = ((area.x - 2)..area.x + area.width)
        .find(|&x| buffer[(x, area.y)].symbol().trim() != "")
        .map(|x| x as i32 - area.x as i32);
    assert_eq!(
        first_text,
        Some(0),
        "heading text lands at the padded content edge"
    );
}

#[test]
fn wide_music_unfocused_selection_matches_the_canonical_control() {
    // Canonical parity with wide Movies / TV / Feeds: the selected-row
    // highlight is a focused-only affordance. The old bespoke painter left a
    // partial text-width `SURFACE_RESTING` smear on the unfocused selected
    // row; that is gone.
    let app = multi_artist_app();
    let (terminal, component) = render_wide(&app, false, 0);
    let layout = component.layout();
    let rect = layout
        .selected_item_rect
        .expect("selected-row rect still published when unfocused");
    let buffer = terminal.backend().buffer();
    let row_bg = buffer[(rect.x, rect.y)].bg;
    for x in rect.x..rect.x + rect.width {
        assert_eq!(
            buffer[(x, rect.y)].bg,
            row_bg,
            "unfocused selected row has a single uniform background (no partial smear)"
        );
    }
}

#[test]
fn wide_music_renders_with_images_enabled() {
    let mut app = multi_artist_app();
    app.image_protocol_enabled = true;
    let (terminal, component) = render_wide(&app, true, 4);
    let layout = component.layout();
    assert!(buffer_to_string(&terminal).contains("Album"));
    assert!(layout.selected_item_rect.is_some());
    // The left hero still owns the album art area in the wide arrangement.
    assert!(layout.wide_music_art_area.width > 0);
}
