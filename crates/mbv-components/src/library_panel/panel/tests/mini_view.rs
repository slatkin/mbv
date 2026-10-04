use super::*;

use mbv_theme::{Surface, surface_colors};

use tuirealm::props::{AttrValue, Attribute};

fn draw_mini_panel(frame_focused: bool) -> (ratatui::buffer::Buffer, ratatui::layout::Rect) {
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(
            FixtureLog::default(),
        )))),
    );
    panel.attr(Attribute::Focus, AttrValue::Flag(true));
    panel.set_mini_view(true);
    panel.set_frame_focused(frame_focused);

    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    let area = Rect::new(0, 0, 60, 24);
    terminal
        .draw(|f| Component::view(&mut panel, f, area))
        .unwrap();
    let list_panel = panel
        .test_narrow_geometry()
        .expect("a sub-breakpoint area paints the narrow skeleton")
        .list_panel;
    (terminal.backend().buffer().clone(), list_panel)
}

/// Mini-view body palette (`pinned-mini-view-focus`): with the projected
/// appearance bit focused, the skeleton paints exactly the wide focused
/// palette -- the focused box fill and the focused zebra stripe -- so the
/// focused mini panel is the wide focused panel, not a third style. The
/// focused zebra tone can appear inside the list box only through a zebra
/// stripe (the box's own fill is the `LibraryPanel` base), and the resting
/// zebra tone has no focused-palette source at all.
#[test]
fn mini_view_rows_paint_the_focused_palette_when_the_appearance_bit_is_focused() {
    let (buf, list_panel) = draw_mini_panel(true);

    let focused_zebra = surface_colors(Surface::LibraryColumn, true).fill;
    let focused_box = surface_colors(Surface::LibraryPanel, true).fill;
    let resting_zebra = surface_colors(Surface::LibraryColumn, false).fill;
    let mut zebra = 0;
    let mut box_fill = 0;
    let mut resting = 0;
    for y in list_panel.y..list_panel.bottom() {
        for x in list_panel.x..list_panel.right() {
            let bg = buf[(x, y)].style().bg;
            zebra += u32::from(bg == Some(focused_zebra));
            box_fill += u32::from(bg == Some(focused_box));
            resting += u32::from(bg == Some(resting_zebra));
        }
    }
    assert!(zebra > 0, "the focused zebra stripe painted");
    assert!(box_fill > 0, "the focused box fill painted");
    assert_eq!(resting, 0, "a resting zebra tone while focused");
}

/// The other side of the contract: with the appearance bit resting (unpinned
/// mini view), the skeleton keeps the resting palette -- no focused zebra, no
/// focused box fill -- even though the component holds interaction focus, and
/// the rows still paint (the resting zebra stripe is visible).
#[test]
fn mini_view_rows_rest_when_the_appearance_bit_is_resting() {
    let (buf, list_panel) = draw_mini_panel(false);

    let focused_zebra = surface_colors(Surface::LibraryColumn, true).fill;
    let focused_box = surface_colors(Surface::LibraryPanel, true).fill;
    let resting_zebra = surface_colors(Surface::LibraryColumn, false).fill;
    let mut zebra = 0;
    let mut focused = 0;
    for y in list_panel.y..list_panel.bottom() {
        for x in list_panel.x..list_panel.right() {
            let bg = buf[(x, y)].style().bg;
            focused += u32::from(bg == Some(focused_zebra) || bg == Some(focused_box));
            zebra += u32::from(bg == Some(resting_zebra));
        }
    }
    assert_eq!(focused, 0, "a focused tone at the resting appearance bit");
    assert!(zebra > 0, "the resting zebra stripe painted");
}
