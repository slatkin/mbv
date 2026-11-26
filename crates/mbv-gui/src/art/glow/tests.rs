//! The glow encoder's contract (design D4 of `gui-hero-design-polish`): a
//! source larger than the glow bounds becomes a PNG at most 64 px on its
//! long side that keeps the source's aspect ratio, and bytes that do not
//! decode give nothing. Every fixture is in memory; no cache, no thread, no
//! live anything.

use std::io::Cursor;

use super::glow_png;

/// The glow bounds' long side, the value the encoder's own constant holds.
const LONG_SIDE: u32 = 64;

/// A PNG `width` by `height`, as a cached source carries it.
fn source_png(width: u32, height: u32) -> Vec<u8> {
    let image = image::DynamicImage::new(width, height, image::ColorType::Rgb8);
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).unwrap();
    png.into_inner()
}

/// A 200×100 source becomes a PNG at most 64 px on its long side that keeps
/// the source's 2:1 aspect ratio (design D4: the glow is never a forced
/// square).
#[test]
fn a_large_source_becomes_a_small_png_that_keeps_its_aspect() {
    let glow = glow_png(&source_png(200, 100)).expect("the source decodes and encodes");
    let decoded = image::load_from_memory(&glow).expect("the glow is a PNG");
    let (width, height) = (decoded.width(), decoded.height());
    assert!(
        width <= LONG_SIDE && height <= LONG_SIDE,
        "the glow is {width}x{height}, past the {LONG_SIDE} px bounds"
    );
    let source_ratio = f64::from(200) / f64::from(100);
    let glow_ratio = f64::from(width) / f64::from(height);
    assert!(
        (glow_ratio - source_ratio).abs() / source_ratio < 0.05,
        "the glow ratio {glow_ratio} drifts from the source ratio {source_ratio}"
    );
}

/// Bytes that do not decode give no glow (design D4: a decode failure
/// answers `Missing`).
#[test]
fn undecodable_bytes_give_none() {
    assert_eq!(glow_png(b"not an image"), None);
}
