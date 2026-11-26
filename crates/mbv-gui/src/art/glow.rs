//! The glow encoder (design D4 of `gui-hero-design-polish`): the pure
//! bytes-to-PNG step the `mbv-art` glow path's worker thread runs off the
//! core thread. It holds no cache and no `App`: it decodes the cached source,
//! shrinks it, blurs it, and encodes it, so the window paints the blur
//! without running one on every repaint.

use image::DynamicImage;
use image::ImageFormat;
use std::io::Cursor;

#[cfg(test)]
mod tests;

/// The thumbnail's long side, in pixels (design D4).
const GLOW_LONG_SIDE: u32 = 64;

/// The blur radius (design D4).
const GLOW_BLUR_SIGMA: f32 = 4.0;

/// The glow PNG for the cached source `bytes` (design D4): decode, shrink to
/// aspect-preserving bounds whose long side is [`GLOW_LONG_SIDE`] (never a
/// forced square, never an upscale of a smaller source), blur, and encode
/// PNG. `None` when the bytes do not decode or the PNG does not encode.
pub(crate) fn glow_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (thumb_width, thumb_height) = glow_bounds(image.width(), image.height());
    let thumb = image::imageops::thumbnail(&image, thumb_width, thumb_height);
    let blurred = image::imageops::blur(&thumb, GLOW_BLUR_SIGMA);
    let mut png = Vec::new();
    DynamicImage::ImageRgba8(blurred)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    Some(png)
}

/// The aspect-preserving bounds whose long side is [`GLOW_LONG_SIDE`] for a
/// `width` by `height` source: scaled down when the long side is larger,
/// unchanged otherwise, and never zero on a side.
fn glow_bounds(width: u32, height: u32) -> (u32, u32) {
    let (long, short) = if width >= height {
        (width, height)
    } else {
        (height, width)
    };
    if long <= GLOW_LONG_SIDE {
        return (width, height);
    }
    // Rounded so a near-square source keeps near-square bounds; the result
    // never exceeds `GLOW_LONG_SIDE`, so the conversion cannot truncate.
    let scaled_short =
        (u64::from(short) * u64::from(GLOW_LONG_SIDE) + u64::from(long) / 2) / u64::from(long);
    let scaled_short = u32::try_from(scaled_short).unwrap_or(1).max(1);
    if width >= height {
        (GLOW_LONG_SIDE, scaled_short)
    } else {
        (scaled_short, GLOW_LONG_SIDE)
    }
}
