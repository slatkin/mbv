//! Artwork never paints above its own pixels (`replace-tauri-with-blitz` D7,
//! the port of `lib/bridge/natural-size.ts`): the core hands the window the
//! server's bytes unchanged, and the art boxes are sized by the layout, so a
//! box larger than the image would stretch it. The cap is the image's natural
//! size in image pixels, each one a CSS pixel: a smaller
//! image paints 1:1, a larger one still scales down to its box.

use crate::window::input::f32_from_f64;

/// An optional box in CSS pixels the image also stays inside.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NaturalSizeBounds {
    /// The widest the image paints, in CSS pixels.
    pub width: Option<f32>,
    /// The tallest the image paints, in CSS pixels.
    pub height: Option<f32>,
}

/// The cap an image paints under: its `max-width` and `max-height`, in CSS
/// pixels, from [`capped_natural_size`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NaturalCap {
    /// The `max-width` value, in CSS pixels.
    pub max_width: f32,
    /// The `max-height` value, in CSS pixels.
    pub max_height: f32,
    /// The width the image paints at once both caps hold and its aspect
    /// stays, in CSS pixels. A box that hugs the image reads its width from
    /// the image's own width, so this is the width it must see (a width the
    /// height cap shrank would otherwise leave the box at the wider cap).
    pub fitted_width: f32,
}

/// The cap for an image whose natural size is `natural_width` ×
/// `natural_height` image pixels (D7): the natural size, each side held inside
/// `bounds` when the caller names one. `None` while the image has no natural
/// size — the source has not loaded, so nothing caps — which the caller
/// repaints when the load lands.
#[must_use]
pub fn capped_natural_size(
    natural_width: u32,
    natural_height: u32,
    bounds: Option<NaturalSizeBounds>,
) -> Option<NaturalCap> {
    if natural_width == 0 || natural_height == 0 {
        return None;
    }
    let width = f32_from_f64(f64::from(natural_width));
    let height = f32_from_f64(f64::from(natural_height));
    let bounded = |side: f32, bound: Option<f32>| bound.map_or(side, |bound| side.min(bound));
    let max_width = bounded(width, bounds.and_then(|bounds| bounds.width));
    let max_height = bounded(height, bounds.and_then(|bounds| bounds.height));
    Some(NaturalCap {
        max_width,
        max_height,
        fitted_width: max_width.min(width * max_height / height),
    })
}
