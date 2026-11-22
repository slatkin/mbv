//! The theme colour to render toolkit colour conversion (design D7).
//!
//! [`mbv_theme::Rgb`] is the neutral value that `mbv-theme` resolves every
//! role and surface to. The TUI painters need the render toolkit's colour,
//! and this crate is the owner of that toolkit. So the conversion lives here,
//! outside the type, as the [`ToRatatui`] extension trait. GUI painters read
//! the `mbv_theme::Rgb` fields directly instead and never depend on this
//! crate.

use mbv_theme::Rgb;
use ratatui::style::Color;

/// The render toolkit colour of an `mbv-theme` colour value.
///
/// Implemented for [`Rgb`], the only colour value `mbv-theme` exposes. The
/// conversion is total: every 8-bit channel triple is a valid toolkit colour,
/// so no arm fails or clamps.
pub trait ToRatatui {
    /// The render toolkit colour for this value.
    fn to_ratatui(&self) -> Color;
}

impl ToRatatui for Rgb {
    fn to_ratatui(&self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }
}
