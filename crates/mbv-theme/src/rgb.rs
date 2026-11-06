//! The neutral colour value (`extract-client-core` task 1.3).
//!
//! [`Rgb`] is the one value roles and surface rows resolve to. It names no
//! toolkit: the TUI crates convert to the render toolkit's colour under the
//! crate's `ratatui` feature ([`Rgb::to_ratatui`]), and a GUI painter reads
//! the fields directly.

/// An 8-bit RGB colour.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[cfg(feature = "ratatui")]
impl Rgb {
    /// The render toolkit colour for this value, for the TUI painters that
    /// enabled the `ratatui` feature.
    #[must_use]
    pub const fn to_ratatui(self) -> ratatui::style::Color {
        ratatui::style::Color::Rgb(self.r, self.g, self.b)
    }
}
