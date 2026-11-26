//! The backdrop encoder's contract (design D4 of `gui-hero-design-polish`):
//! the bottom row is exactly the pane background, so the image has no edge;
//! the top-right keeps the artwork's colour; the title area stays legible
//! even for a white source; undecodable bytes give nothing. Every fixture is
//! in memory.

use std::io::Cursor;

use image::{Rgb, RgbImage};

use super::{
    BACKDROP_HEIGHT, BackdropStyle, MIN_TITLE_CONTRAST, PANE_BG, backdrop_jpeg, bake, baked_width,
    blurred_fields, title_contrast,
};

/// The baked width the tests use.
const WIDTH: u32 = 600;

/// A PNG of one colour, as a cached source carries it.
fn source_png(colour: [u8; 3]) -> Vec<u8> {
    let image = RgbImage::from_pixel(64, 36, Rgb(colour));
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .expect("the fixture encodes");
    png.into_inner()
}

fn baked(colour: [u8; 3], style: BackdropStyle) -> RgbImage {
    let jpeg = backdrop_jpeg(&source_png(colour), WIDTH, style).expect("the source bakes");
    image::load_from_memory(&jpeg)
        .expect("the backdrop is a JPEG")
        .into_rgb8()
}

#[test]
fn the_bottom_row_is_exactly_the_pane_background() {
    let source = image::load_from_memory(&source_png([220, 40, 40]))
        .expect("the fixture decodes")
        .into_rgb8();
    let fields = blurred_fields(&source, WIDTH, BackdropStyle::Video);
    let image = bake(&fields, 0.55);

    assert_eq!(image.dimensions(), (WIDTH, BACKDROP_HEIGHT));
    assert!(
        (0..WIDTH).all(|x| image.get_pixel(x, BACKDROP_HEIGHT - 1).0 == PANE_BG),
        "a bottom-row pixel differs from the pane background"
    );
}

#[test]
fn the_top_right_keeps_the_artwork_colour() {
    let image = baked([220, 40, 40], BackdropStyle::Video);
    let pixel = image.get_pixel(WIDTH - 1, 0).0;

    assert!((0..3).any(|channel| pixel[channel].abs_diff(PANE_BG[channel]) > 20));
}

#[test]
fn a_white_source_keeps_the_title_legible() {
    let image = baked([255, 255, 255], BackdropStyle::Video);

    assert!(title_contrast(&image) >= MIN_TITLE_CONTRAST - 0.2);
}

#[test]
fn undecodable_bytes_give_none() {
    assert_eq!(
        backdrop_jpeg(b"not an image", WIDTH, BackdropStyle::Music),
        None
    );
}

#[test]
fn the_width_rounds_up_to_the_next_step() {
    assert_eq!(baked_width(1001.0), 1200);
    assert_eq!(baked_width(1000.0), 1000);
}
