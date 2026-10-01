use image::{DynamicImage, GenericImageView, Rgb, RgbImage, Rgba, RgbaImage};
use num_traits::ToPrimitive;
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

/// A solid opaque-green logo, so [`bounds_for`]'s green channel isolates the
/// logo's pixels from the base and from the red/blue title ink.
fn logo_image(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(width, height, Rgba([0, 255, 0, 255])))
}

fn has_darker_than_scrim_pixel(
    image: &DynamicImage,
    bounds: (u32, u32, u32, u32),
    down_right: bool,
) -> bool {
    image.to_rgb8().enumerate_pixels().any(|(x, y, pixel)| {
        let beyond_ink = if down_right {
            x > bounds.2 || y > bounds.3
        } else {
            x < bounds.0 && y < bounds.1
        };
        beyond_ink && pixel.0[0] < 40 && pixel.0[1] < 45 && pixel.0[2] < 50
    })
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

/// Mean horizontal position of the ink pixels in `channel`, i.e. the raw ink
/// centroid the centring contract is stated in terms of.
fn centroid_x(image: &DynamicImage, channel: usize) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0.0;
    for (x, _, pixel) in image.to_rgb8().enumerate_pixels() {
        if pixel.0[channel] > 180 && pixel.0[(channel + 1) % 3] < 80 {
            sum += f64::from(x);
            count += 1.0;
        }
    }
    (count > 0.0).then(|| sum / count)
}

#[test]
fn text_rows_are_horizontally_centred_within_the_side_padding() {
    let width = 400;
    let image = compose_title_overlay(
        &black_image(width, 120),
        None,
        FontSize::new(12, 20),
        TitleOverlayText {
            context: Some("Artist"),
            title: "Song",
        },
        COLOURS,
    );
    let centre = f64::from(width) / 2.0;
    let context_centroid = centroid_x(&image, 0).expect("context glyphs should be painted");
    let title_centroid = centroid_x(&image, 2).expect("title glyphs should be painted");

    assert!(
        (context_centroid - centre).abs() <= 3.0,
        "context ink centroid {context_centroid} should sit near {centre}"
    );
    assert!(
        (title_centroid - centre).abs() <= 3.0,
        "title ink centroid {title_centroid} should sit near {centre}"
    );
}

#[test]
fn one_part_paints_top_row_with_down_right_shadow_and_preserves_bottom_pixels() {
    let base = black_image(240, 120);
    let image = compose_title_overlay(
        &base,
        None,
        FontSize::new(12, 20),
        TitleOverlayText {
            context: None,
            title: "A title",
        },
        COLOURS,
    );

    assert_ne!(image.get_pixel(0, 0), base.get_pixel(0, 0));
    let ink = bounds_for(&image, 0).expect("title glyphs should be painted");
    assert!(has_darker_than_scrim_pixel(&image, ink, true));
    assert!(!has_darker_than_scrim_pixel(&image, ink, false));
    assert_eq!(image.get_pixel(239, 0), image.get_pixel(239, 19));
    assert_eq!(image.get_pixel(120, 60), base.get_pixel(120, 60));
    assert_eq!(
        image.crop_imm(0, 100, 240, 20).to_rgb8(),
        base.crop_imm(0, 100, 240, 20).to_rgb8()
    );
    assert!(bounds_for(&image, 0).is_some());
    assert_eq!(bounds_for(&image, 2), None);
}

#[test]
fn two_parts_paint_context_at_top_and_title_at_bottom() {
    let image = compose_title_overlay(
        &black_image(240, 120),
        None,
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
fn long_text_is_ellipsised_within_the_artwork_margin_regression_aed359900() {
    let title = "An exceptionally long title that cannot fit";
    let cell = FontSize::new(8, 20);
    let image = compose_title_overlay(
        &black_image(80, 80),
        None,
        cell,
        TitleOverlayText {
            context: None,
            title,
        },
        COLOURS,
    );
    let padding = 80 * 3 / 100;
    let max_width = f64::from(80 - 2 * padding);
    let nominal = (f32::from(cell.height) * super::GLYPH_SCALE).max(1.0);
    let floor = (nominal * super::MIN_GLYPH_SCALE).max(1.0);
    let (scale, fitted) = fit_text(title, max_width, nominal, floor);
    let bounds = bounds_for(&image, 0).expect("title glyphs should be painted");

    assert_eq!(fitted.last(), Some(&'…'));
    assert!(text_width(&fitted, scale) <= max_width);
    assert!(bounds.2 < 80 - padding);
}

#[test]
fn title_fitting_at_floor_is_not_ellipsised_regression_aed359900() {
    let title = "A title that fits at floor";
    let cell = FontSize::new(8, 20);
    let nominal = (f32::from(cell.height) * super::GLYPH_SCALE).max(1.0);
    let floor = (nominal * super::MIN_GLYPH_SCALE).max(1.0);
    let characters: Vec<_> = title.chars().collect();
    let floor_width = text_width(&characters, floor);
    let above_floor = (floor + 1.0).floor();
    let next_width = text_width(&characters, above_floor);
    let artwork_width = (f64::midpoint(floor_width, next_width) / 0.94)
        .round()
        .to_u32()
        .expect("title width should fit in artwork");
    let padding = artwork_width * 3 / 100;
    let max_width = f64::from(artwork_width - 2 * padding);
    let (scale, fitted) = fit_text(title, max_width, nominal, floor);

    assert!(floor_width <= max_width);
    assert!(next_width > max_width);
    assert!((scale - floor).abs() < f32::EPSILON);
    assert_eq!(fitted, characters);
}

#[test]
fn blank_context_is_absent_and_leaves_bottom_untouched_regression_aed359900() {
    let base = black_image(240, 120);
    let without_context = compose_title_overlay(
        &base,
        None,
        FontSize::new(12, 20),
        TitleOverlayText {
            context: None,
            title: "A title",
        },
        COLOURS,
    );
    let with_blank_context = compose_title_overlay(
        &base,
        None,
        FontSize::new(12, 20),
        TitleOverlayText {
            context: Some(" \t "),
            title: "A title",
        },
        COLOURS,
    );

    assert_eq!(with_blank_context, without_context);
    assert_eq!(with_blank_context.get_pixel(0, 119), base.get_pixel(0, 119));
}

#[test]
fn unsupported_cjk_character_is_not_covered() {
    assert!(!covers("界"));
}

#[test]
fn title_height_tracks_cell_rows_at_different_box_sizes() {
    let small = compose_title_overlay(
        &black_image(240, 120),
        None,
        FontSize::new(12, 20),
        TitleOverlayText {
            context: None,
            title: "Same title",
        },
        COLOURS,
    );
    let large = compose_title_overlay(
        &black_image(480, 240),
        None,
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

#[test]
fn a_logo_replaces_the_top_row_without_a_scrim() {
    let base = black_image(240, 120);
    let image = compose_title_overlay(
        &base,
        Some(&logo_image(40, 40)),
        FontSize::new(12, 20),
        TitleOverlayText {
            context: None,
            title: "A title",
        },
        COLOURS,
    );
    let logo = bounds_for(&image, 1).expect("logo pixels should be painted");
    let padding = 240 * 3 / 100;

    // Top-left, inside a box two cell rows tall (40) and half the width (120).
    assert_eq!(logo, (padding, padding, padding + 39, padding + 39));
    assert!(logo.3 - logo.1 < 40);
    assert!(logo.2 - logo.0 < 120);
    // No scrim: the top row outside the logo keeps the base's pixels.
    assert_eq!(image.get_pixel(0, 0), base.get_pixel(0, 0));
    assert_eq!(image.get_pixel(200, 5), base.get_pixel(200, 5));
    assert_eq!(image.get_pixel(239, 19), base.get_pixel(239, 19));
}

#[test]
fn a_two_part_title_keeps_its_bottom_row_when_a_logo_is_present() {
    let image = compose_title_overlay(
        &black_image(240, 120),
        Some(&logo_image(40, 40)),
        FontSize::new(12, 20),
        TitleOverlayText {
            context: Some("Artist"),
            title: "Song",
        },
        COLOURS,
    );
    let logo = bounds_for(&image, 1).expect("logo pixels should be painted");
    let title = bounds_for(&image, 2).expect("title glyphs should be painted");

    assert!(logo.3 - logo.1 < 40);
    assert_eq!(bounds_for(&image, 0), None);
    assert!(title.1 >= 100);
}

#[test]
fn a_one_part_title_with_a_logo_draws_no_text() {
    let image = compose_title_overlay(
        &black_image(240, 120),
        Some(&logo_image(40, 40)),
        FontSize::new(12, 20),
        TitleOverlayText {
            context: None,
            title: "A title",
        },
        COLOURS,
    );

    assert!(bounds_for(&image, 0).is_none());
    assert!(bounds_for(&image, 2).is_none());
    assert!(bounds_for(&image, 1).is_some());
}

#[test]
fn a_logo_is_drawn_when_the_replaced_top_row_text_is_uncovered() {
    let image = compose_title_overlay(
        &black_image(240, 120),
        Some(&logo_image(40, 40)),
        FontSize::new(12, 20),
        TitleOverlayText {
            context: None,
            title: "界",
        },
        COLOURS,
    );

    assert!(bounds_for(&image, 1).is_some());
    assert!(bounds_for(&image, 0).is_none());
}

#[test]
fn an_uncovered_context_row_is_not_gated_when_a_logo_replaces_it() {
    let image = compose_title_overlay(
        &black_image(240, 120),
        Some(&logo_image(40, 40)),
        FontSize::new(12, 20),
        TitleOverlayText {
            context: Some("界"),
            title: "Song",
        },
        COLOURS,
    );

    assert!(bounds_for(&image, 1).is_some());
    assert!(bounds_for(&image, 2).is_some());
}

#[test]
fn an_uncovered_title_row_keeps_the_base_when_a_logo_is_present() {
    let base = black_image(240, 120);
    let image = compose_title_overlay(
        &base,
        Some(&logo_image(40, 40)),
        FontSize::new(12, 20),
        TitleOverlayText {
            context: Some("Artist"),
            title: "界",
        },
        COLOURS,
    );

    assert_eq!(image.to_rgb8(), base.to_rgb8());
}

#[test]
fn without_a_logo_the_text_only_composition_is_unchanged() {
    let base = black_image(240, 120);
    let image = compose_title_overlay(
        &base,
        None,
        FontSize::new(12, 20),
        TitleOverlayText {
            context: None,
            title: "A title",
        },
        COLOURS,
    );

    // The top row is still scrimmed across its whole height (the logo path
    // would leave these pixels at the base value) and the title ink remains.
    assert_eq!(image.get_pixel(239, 0), image.get_pixel(239, 19));
    assert_ne!(image.get_pixel(0, 0), base.get_pixel(0, 0));
    assert!(bounds_for(&image, 0).is_some());
}
