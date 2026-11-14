//! Tests of the launch copy's window attributes (D5). The attributes are
//! built without a window: no test opens one, sleeps, or reaches the core.

use dioxus_native::winit::dpi::{LogicalSize, Size};

use super::window_attributes;

/// Contract: the window keeps the Tauri window's 400x200 logical floor, so a
/// floating window cannot be dragged below the size at which the compact
/// strip lays out.
#[test]
fn the_window_carries_a_400_by_200_logical_minimum() {
    let attributes = window_attributes();

    assert_eq!(
        attributes.min_surface_size,
        Some(Size::Logical(LogicalSize::new(400.0, 200.0)))
    );
}
