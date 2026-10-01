use image::{DynamicImage, GenericImageView, Rgb, RgbImage};
use ratatui_image::FontSize;

use super::{
    TitleOverlayColours, TitleOverlayText, compose_title_overlay, covers, fit_text, text_width,
};

const COLOURS: TitleOverlayColours = TitleOverlayColours {
    context: [255, 0, 0],
    title: [0, 0, 255],
};

fn black_image(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_pixel(width, height, Rgb([80, 90, 100])))
}

fn bounds_for(image: &DynamicImage, channel: usize) -> Option<(u32, u32, u32, u32)> {
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for (x, y, pixel) in image.to_rgb8().enumerate_pixels() {
        if pixel.0[channel] > 180 && pixel.0[(channel + 1) % 3] < 80 {
            bounds = Some(match bounds {
                Some((left, top, right, bottom)) => {
                    (left.min(x), top.min(y), right.max(x), bottom.max(y))
                }
                None => (x, y, x, y),
            });
        }
    }
    bounds
}

#[test]
fn one_part_paints_top_row_and_preserves_bottom_pixels() {
    let base = black_image(240, 120);
    let image = compose_title_overlay(
        &base,
        FontSize::new(12, 20),
        TitleOverlayText {
            context: None,
            title: "A title",
        },
        COLOURS,
    );

    assert_ne!(image.get_pixel(0, 0), base.get_pixel(0, 0));
    assert_eq!(image.get_pixel(120, 60), base.get_pixel(120, 60));
    assert_eq!(image.get_pixel(0, 119), base.get_pixel(0, 119));
    assert!(bounds_for(&image, 0).is_some());
    assert_eq!(bounds_for(&image, 2), None);
}

#[test]
fn two_parts_paint_context_at_top_and_title_at_bottom() {
    let image = compose_title_overlay(
        &black_image(240, 120),
        FontSize::new(12, 20),
        TitleOverlayText {
            context: Some("Artist"),
            title: "Song",
        },
        COLOURS,
    );
    let top = bounds_for(&image, 0).expect("context glyphs should be painted");
    let bottom = bounds_for(&image, 2).expect("title glyphs should be painted");

    assert!(top.3 < 20);
    assert!(bottom.1 >= 100);
}

#[test]
fn long_text_is_ellipsised_inside_the_text_width() {
    let title = "An exceptionally long title that cannot fit";
    let image = compose_title_overlay(
        &black_image(80, 80),
        FontSize::new(8, 20),
        TitleOverlayText {
            context: None,
            title,
        },
        COLOURS,
    );
    let max_width = f64::from(80_u8 - 2 * (80_u8 * 3 / 100));
    let (scale, fitted) = fit_text(title, max_width, 14.0, 8.4);

    assert_eq!(fitted.last(), Some(&'…'));
    assert!(text_width(&fitted, scale) <= max_width);
    assert!(bounds_for(&image, 0).is_some());
    assert!(
        bounds_for(&image, 0)
            .expect("title glyphs should be painted")
            .2
            < 80
    );
}

#[test]
fn unsupported_cjk_character_is_not_covered() {
    assert!(!covers("界"));
}

#[test]
fn title_height_tracks_cell_rows_at_different_box_sizes() {
    let small = compose_title_overlay(
        &black_image(240, 120),
        FontSize::new(12, 20),
        TitleOverlayText {
            context: None,
            title: "Same title",
        },
        COLOURS,
    );
    let large = compose_title_overlay(
        &black_image(480, 240),
        FontSize::new(24, 40),
        TitleOverlayText {
            context: None,
            title: "Same title",
        },
        COLOURS,
    );
    let small_bounds = bounds_for(&small, 0).expect("small title should be painted");
    let large_bounds = bounds_for(&large, 0).expect("large title should be painted");
    let small_rows = f64::from(small_bounds.3 - small_bounds.1 + 1) / 20.0;
    let large_rows = f64::from(large_bounds.3 - large_bounds.1 + 1) / 40.0;

    assert!((small_rows - large_rows).abs() < 0.05);
}
