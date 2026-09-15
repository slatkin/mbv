//! Buffer coverage for the Wide hero split-override plumbing (task 1.2).
//!
//! Proves `None` is inert — the frame with no override is byte-identical to
//! the frame with the override explicitly pinned to the default ratio, i.e.
//! the pre-override frame — and that a `Some` override repaints the split.

use super::test_helpers::buffer_to_string;
use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::tv_content::TvContent;
use crate::app::components::LibraryKind;
use crate::app::render::arrangements::library::wide_library_panes;
use crate::app::render::arrangements::wide_hero::{
    wide_hero_split, WIDE_HERO_MIN_PANE_WIDTH, WIDE_HERO_PANE_GAP,
};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::TvWideRenderCtx;
use crate::app::tests::make_item;
use mbv_core::config::ServiceKind;
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
/// through the mounted panel's per-frame override seam (task 8.4: the panel
/// hosts the TV owner and receives the session width).
fn render_tv_wide(override_width: Option<u16>) -> String {
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![make_item("Series One", "Series")], 0, 0),
        None,
        None,
        0,
        None,
        true,
    ));
    let key = LibraryKey::Service {
        service: ServiceKind::Emby,
        library_id: "lib".into(),
        kind: LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key.clone(), Box::new(owner));
    panel.set_active(Some(key));
    panel.set_list_pane_width(override_width);
    let area = wide_area();
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    terminal
        .draw(|f| Component::view(&mut panel, f, area))
        .unwrap();
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
        moved.browser_panel.x,
        moved.hero_panel.right() + WIDE_HERO_PANE_GAP,
        "the existing gap follows the moved boundary"
    );
}

/// One session width serves every Wide hero surface: the same raw override is
/// clamped against each surface's own content width, and the session value is
/// never re-clamped in place, so switching to a narrower surface does not
/// shrink the width the next surface sees.
#[test]
fn one_session_width_is_clamped_per_surface_without_mutating_it() {
    let session = Some(130u16);
    let wide_area = Rect::new(0, 0, 220, HEIGHT);
    let narrow_area = Rect::new(0, 0, 110, HEIGHT);

    let wide = wide_library_panes(wide_area, PAD_X, PAD_Y, session).expect("wide fits");
    let narrow = wide_library_panes(narrow_area, PAD_X, PAD_Y, session).expect("narrow fits");

    assert_eq!(wide.browser_panel.width, 130);
    assert_eq!(
        narrow.browser_panel.width,
        narrow_area.width - WIDE_HERO_MIN_PANE_WIDTH - WIDE_HERO_PANE_GAP,
        "the narrower surface clamps the session width to its own range"
    );
    assert!(narrow.browser_panel.width < wide.browser_panel.width);
    assert_eq!(
        session,
        Some(130),
        "the session width is never re-clamped in place"
    );
}
