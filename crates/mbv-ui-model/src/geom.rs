//! Toolkit-free geometry for presentation models.
//!
//! `Rect` is the neutral rectangle the presentation models carry instead of a
//! render-toolkit type. The TUI edge converts between this shape and
//! `ratatui::layout::Rect` where a render layout meets a model
//! (`mbv_render::layout::rect_to_presentation`).

/// A rectangle in terminal cells: top-left origin, `width` and `height`
/// measured in cells. `right()` and `bottom()` saturate, matching the
/// render-toolkit rectangle's arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The first column right of the rectangle.
    #[must_use]
    pub const fn right(self) -> u16 {
        self.x.saturating_add(self.width)
    }

    /// The first row below the rectangle.
    #[must_use]
    pub const fn bottom(self) -> u16 {
        self.y.saturating_add(self.height)
    }
}
