//! Buffer coverage for the Wide hero split-override plumbing (task 1.2).
//!
//! Proves `None` is inert — the frame with no override is byte-identical to
//! the frame with the override explicitly pinned to the default ratio, i.e.
//! the pre-override frame — and that a `Some` override repaints the split.

use super::test_helpers::buffer_to_string;
use crate::app::components::TvWorkspaceComponent;
use crate::app::render::arrangements::library::wide_library_panes;
use crate::app::render::arrangements::wide_hero::{wide_hero_split, WIDE_HERO_PANE_GAP};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::TvWideRenderCtx;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::Component;

const WIDTH: u16 = 120;
const HEIGHT: u16 = 30;
const PAD_X: u16 = 2;
const PAD_Y: u16 = 1;

fn wide_area() -> Rect {
    Rect::new(0, 0, WIDTH, HEIGHT)
}

/// Renders the representative wide TV surface with `override_width` pushed
/// through the mounted component's per-draw override seam.
fn render_tv_wide(override_width: Option<u16>) -> String {
    let mut component = TvWorkspaceComponent::new();
    component.set_content(
        TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![make_item("Series One", "Series")], 0, 0),
            None,
            None,
            0,
            None,
            true,
        )
        .with_image_state(false, false),
    );
    component.set_list_pane_width(override_width);
    let area = wide_area();
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    terminal.draw(|f| component.view(f, area)).unwrap();
    buffer_to_string(&terminal)
}

/// `None` must paint exactly the current default ratio: pinning the override
/// to the default split reproduces the no-override frame byte-for-byte.
#[test]
fn none_override_is_byte_identical_to_the_default_frame() {
    let default_browser_w = wide_hero_split(wide_area(), None).0.width;
    assert_eq!(
        render_tv_wide(None),
        render_tv_wide(Some(default_browser_w)),
        "None must paint exactly the default split"
    );
}

/// A `Some` override repaints the split, and the shared arrangement places the
/// gap immediately after the moved list pane.
#[test]
fn an_override_moves_the_painted_panes() {
    let area = wide_area();
    let default_browser_w = wide_hero_split(area, None).0.width;
    let moved_override = Some(default_browser_w + 12);
    assert_ne!(
        render_tv_wide(None),
        render_tv_wide(moved_override),
        "the override must repaint the split"
    );

    let default = wide_library_panes(area, PAD_X, PAD_Y, None).expect("wide fits");
    let moved = wide_library_panes(area, PAD_X, PAD_Y, moved_override).expect("wide fits");
    assert!(moved.browser_panel.width > default.browser_panel.width);
    assert!(
        moved.hero_panel.width < default.hero_panel.width,
        "the hero pane takes the remainder"
    );
    assert_eq!(
        moved.hero_panel.x,
        moved.browser_panel.right() + WIDE_HERO_PANE_GAP,
        "the existing gap follows the moved boundary"
    );
}
