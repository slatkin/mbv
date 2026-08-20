//! Table-driven tests for `ContextMenu::place_menu` and `rendered_size`.
use super::*;
use ratatui::layout::Rect;

/// A fixed-size test cell and panel mirrored between percent and pixel
/// expectations. Placement is pure geometry; only the anchor/panel inputs
/// vary per test.
const PANEL: Rect = Rect::new(0, 0, 100, 30);
const SIZE: (u16, u16) = (40, 8);

#[test]
fn down_placement_aligns_top_right_to_selected_item() {
    // Item at (60, 2, 30, 1): menu (40 wide) right-aligns to x=90, opens
    // down from y=2 (fits: 2+8 <= 30).
    let anchor = Rect::new(60, 2, 30, 1);
    assert_eq!(
        ContextMenu::place(PANEL, SIZE, Some(&anchor), None),
        (60, 2)
    );
}

#[test]
fn up_placement_aligns_bottom_right_to_selected_item() {
    // Item at (60, 26, 30, 1): down would end at 34 > 30, so opens up with
    // its bottom at 27 -> y=19.
    let anchor = Rect::new(60, 26, 30, 1);
    assert_eq!(
        ContextMenu::place(PANEL, SIZE, Some(&anchor), None),
        (60, 19)
    );
}

#[test]
fn horizontal_clamp_keeps_menu_inside_panel() {
    // Item hugs the right edge: (80, 5, 20, 1). Right-align would put x=100
    // (outside), so clamp to 100-40=60.
    let anchor = Rect::new(80, 5, 20, 1);
    assert_eq!(
        ContextMenu::place(PANEL, SIZE, Some(&anchor), None),
        (60, 5)
    );
    // Item hugs the left edge (anchor right = 10): menu would start at
    // -30, clamps to panel.x=0.
    let anchor = Rect::new(0, 5, 10, 1);
    assert_eq!(ContextMenu::place(PANEL, SIZE, Some(&anchor), None), (0, 5));
}

#[test]
fn vertical_clamp_keeps_menu_inside_panel_when_no_room() {
    // Item at the very bottom (0, 27, 5, 1): down overflows, so the menu's
    // bottom aligns to the item's bottom (28) -> y = 28-8 = 20.
    let anchor = Rect::new(0, 27, 5, 1);
    assert_eq!(
        ContextMenu::place(PANEL, SIZE, Some(&anchor), None),
        (0, 20)
    );
}

#[test]
fn pointer_placement_is_click_anchored_and_clamped() {
    // Pointer at (95, 28): menu (40x8) would extend past both edges; x
    // clamps to 60, y clamps to 22.
    assert_eq!(
        ContextMenu::place(PANEL, SIZE, None, Some((95, 28))),
        (60, 22)
    );
    // Pointer near top-left keeps exact click position.
    assert_eq!(ContextMenu::place(PANEL, SIZE, None, Some((5, 5))), (5, 5));
}

#[test]
fn missing_selected_geometry_falls_back_to_panel_origin() {
    let anchor = None;
    assert_eq!(ContextMenu::place(PANEL, SIZE, anchor, None), (0, 0));
}

#[test]
fn zero_dimensions_never_underflow() {
    // Menu with zero size against a zero-size panel: no math underflows.
    let tiny = Rect::default();
    assert_eq!(ContextMenu::place(tiny, (0, 0), None, Some((0, 0))), (0, 0));
    // Pointer far outside a zero-size panel stays clamped to the origin.
    assert_eq!(
        ContextMenu::place(tiny, (10, 10), None, Some((50, 50))),
        (0, 0)
    );
}

#[test]
fn menu_larger_than_panel_pins_to_panel_edge() {
    // Menu taller than the panel (100x100 menu in 30-tall panel): y clamps
    // to the bottom edge and the overflow is left for the renderer to clip.
    assert_eq!(
        ContextMenu::place(PANEL, (100, 100), None, Some((5, 5))),
        (0, 0)
    );
    assert_eq!(
        ContextMenu::place(PANEL, (100, 100), Some(&Rect::new(10, 10, 5, 1)), None),
        (0, 0)
    );
}

#[test]
fn rendered_size_uses_wide_entry_and_plus_two_rows() {
    let entries = vec![
        ContextMenuEntry {
            label: "Play",
            action: Some(ContextAction::Play),
        },
        ContextMenuEntry {
            label: "A much longer label",
            action: None,
        },
    ];
    assert_eq!(
        ContextMenu::rendered_size(&entries),
        ("A much longer label".len() as u16 + 4, 4)
    );
    // Separator-only menus still get a minimum width (renderer's `unwrap_or(4)`).
    assert_eq!(ContextMenu::rendered_size(&[]), (8, 2));
}
