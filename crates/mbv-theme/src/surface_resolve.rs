//! Resolves a `Surface` plus the call site's own focus bit to its
//! `SurfaceColors`.
//!
//! This is the table's only resolver (design D4): a production paint site
//! names a `Surface` and hands in the focus input it already computes — a
//! mounted component's focus, a pane's `held` bit, a panel focus bool, or the
//! site's own cursor predicate. Each row states its two slots directly
//! (`surface_table::slots`), so the bool selects between exactly two fills.

use ratatui::style::Color;

use super::slot::{Theme, active};
use super::surface::Surface;
use super::surface_table::slots;

/// The resolved colour of one rendered surface for one frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceColors {
    pub fill: Color,
}

impl SurfaceColors {
    const fn fill(fill: Color) -> Self {
        Self { fill }
    }
}

/// Resolves a surface's fill against `theme` (the theme-swap test resolves
/// through this; design D4).
pub(crate) fn surface_colors_in(theme: &Theme, surface: Surface, focused: bool) -> SurfaceColors {
    let (resting, focused_slot) = slots(surface);
    SurfaceColors::fill(theme.get(if focused { focused_slot } else { resting }))
}

/// Resolves a surface's fill from the call site's own focus bit
/// (design D1/D2/D3).
#[must_use]
pub fn surface_colors(surface: Surface, focused: bool) -> SurfaceColors {
    surface_colors_in(active(), surface, focused)
}
