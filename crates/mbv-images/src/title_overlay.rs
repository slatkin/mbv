use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use image::{DynamicImage, RgbaImage};
use num_traits::ToPrimitive;
use ratatui_image::FontSize;

static FONT: LazyLock<FontArc> = LazyLock::new(|| {
    FontArc::try_from_slice(include_bytes!("../assets/LexendDeca-SemiBold.ttf"))
        .expect("the embedded Lexend Deca font should be valid")
});

const SCRIM_ALPHA: f32 = 0.50;
const SHADOW_ALPHA: f32 = 0.55;
const GLYPH_SCALE: f32 = 0.90;
const MIN_GLYPH_SCALE: f32 = 0.60;
const SIDE_PADDING_PERCENT: u32 = 3;

/// Text painted into the artwork's title rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TitleOverlayText<'a> {
    pub context: Option<&'a str>,
    pub title: &'a str,
}

/// Resolved foreground colours for context and title text, as RGB triples.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TitleOverlayColours {
    pub context: [u8; 3],
    pub title: [u8; 3],
}

/// Cache identity for an overlay, independent of an Audiobookshelf protocol suffix.
#[must_use]
pub fn title_overlay_cache_key(
    cache_key: &str,
    cols: u16,
    rows: u16,
    text: TitleOverlayText<'_>,
) -> String {
    let identity = if cache_key.starts_with(crate::AUDIOBOOKSHELF_CACHE_KEY_PREFIX) {
        cache_key
            .rsplit_once(':')
            .map_or(cache_key, |(base, _)| base)
    } else {
        cache_key
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.context.hash(&mut hasher);
    text.title.hash(&mut hasher);
    format!("{identity}:t:{cols}x{rows}:{:x}", hasher.finish())
}

/// Whether the embedded font contains every character in `text`.
#[must_use]
pub fn covers(text: &str) -> bool {
    text.chars()
        .all(|character| FONT.glyph_id(character).0 != 0)
}

/// Draws a title over a cover-sized image without changing pixels outside its text rows.
#[must_use]
pub fn compose_title_overlay(
    base: &DynamicImage,
    cell: FontSize,
    text: TitleOverlayText<'_>,
    colours: TitleOverlayColours,
) -> DynamicImage {
    let mut image = base.to_rgba8();
    if image.width() == 0 || image.height() == 0 || cell.height == 0 {
        return DynamicImage::ImageRgba8(image);
    }

    let context = text.context.filter(|context| !context.trim().is_empty());
    let has_context = context.is_some();
    let top_text = context.unwrap_or(text.title);
    if !covers(top_text) || (has_context && !covers(text.title)) {
        return DynamicImage::ImageRgba8(image);
    }

    let row_height = u32::from(cell.height).min(image.height());
    let top_y = 0;
    paint_row(
        &mut image,
        top_y,
        row_height,
        cell.height,
        top_text,
        colours.context,
    );

    if has_context {
        let bottom_y = image.height().saturating_sub(row_height);
        paint_row(
            &mut image,
            bottom_y,
            row_height,
            cell.height,
            text.title,
            colours.title,
        );
    }

    DynamicImage::ImageRgba8(image)
}

fn paint_row(
    image: &mut RgbaImage,
    row_y: u32,
    row_height: u32,
    cell_height: u16,
    text: &str,
    colour: [u8; 3],
) {
    let (width, height) = image.dimensions();
    let padding = (width * SIDE_PADDING_PERCENT / 100).min(width / 2);
    let max_width = f64::from(width.saturating_sub(padding * 2));
    let nominal = (f32::from(cell_height) * GLYPH_SCALE).max(1.0);
    let floor = (nominal * MIN_GLYPH_SCALE).max(1.0);
    let (scale, fitted) = fit_text(text, max_width, nominal, floor);
    let right_limit = width.saturating_sub(padding);
    let scaled_font = FONT.as_scaled(PxScale::from(scale));
    let ascent = scaled_font.ascent();
    let glyph_height = ascent - scaled_font.descent();
    let baseline = row_y
        .to_f32()
        .expect("image row position should fit in floating point")
        + ((f32::from(cell_height) - glyph_height) / 2.0)
        + ascent;

    let alpha = (SCRIM_ALPHA * 255.0)
        .round()
        .to_u8()
        .expect("scrim alpha should fit in one byte");
    for y in row_y..row_y.saturating_add(row_height).min(height) {
        for x in 0..width {
            blend_pixel(image, x, y, [0, 0, 0], alpha);
        }
    }

    let mut x = padding
        .to_f32()
        .expect("image padding should fit in floating point");
    for character in fitted {
        let glyph_id = FONT.glyph_id(character);
        let advance = scaled_font.h_advance(glyph_id);
        let glyph = glyph_id.with_scale_and_position(scale, point(x, baseline));
        if let Some(outlined) = FONT.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let shadow_offset = (glyph_height / 16.0)
                .round()
                .max(1.0)
                .to_i32()
                .expect("glyph shadow offset should fit in a signed pixel coordinate");
            let mut coverage_pixels = Vec::new();
            outlined.draw(|glyph_x, glyph_y, coverage| {
                let origin_x = bounds
                    .min
                    .x
                    .floor()
                    .to_i32()
                    .expect("glyph pixel origin should fit in a signed pixel coordinate");
                let origin_y = bounds
                    .min
                    .y
                    .floor()
                    .to_i32()
                    .expect("glyph pixel origin should fit in a signed pixel coordinate");
                let glyph_x = i32::try_from(glyph_x)
                    .expect("glyph width should fit in a signed pixel coordinate");
                let glyph_y = i32::try_from(glyph_y)
                    .expect("glyph height should fit in a signed pixel coordinate");
                let Some(pixel_x) = origin_x.checked_add(glyph_x) else {
                    return;
                };
                let Some(pixel_y) = origin_y.checked_add(glyph_y) else {
                    return;
                };
                let alpha = (coverage.clamp(0.0, 1.0) * 255.0)
                    .round()
                    .to_u8()
                    .expect("clamped glyph coverage should fit in one byte");
                if alpha == 0 {
                    return;
                }
                coverage_pixels.push((pixel_x, pixel_y, alpha));
            });
            paint_glyph(
                image,
                &coverage_pixels,
                shadow_offset,
                colour,
                right_limit,
                row_y,
                row_height,
            );
        }
        x += advance;
    }
}

fn paint_glyph(
    image: &mut RgbaImage,
    coverage_pixels: &[(i32, i32, u8)],
    radius: i32,
    colour: [u8; 3],
    right_limit: u32,
    row_y: u32,
    row_height: u32,
) {
    let (width, height) = image.dimensions();
    let row_bottom = row_y.saturating_add(row_height).min(height);
    for &(pixel_x, pixel_y, coverage) in coverage_pixels {
        let shadow_alpha = (f32::from(coverage) * SHADOW_ALPHA)
            .round()
            .to_u8()
            .expect("shadow coverage should fit in one byte");
        let shadow = pixel_x
            .checked_add(radius)
            .zip(pixel_y.checked_add(radius))
            .and_then(|(x, y)| Some((u32::try_from(x).ok()?, u32::try_from(y).ok()?)))
            .filter(|(x, y)| *x < width && *x < right_limit && *y >= row_y && *y < row_bottom);
        if let Some((x, y)) = shadow {
            blend_pixel(image, x, y, [0, 0, 0], shadow_alpha);
        }
    }
    for &(x, y, alpha) in coverage_pixels {
        if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y))
            && x < right_limit
            && y < height
        {
            blend_pixel(image, x, y, colour, alpha);
        }
    }
}

fn fit_text(text: &str, max_width: f64, nominal: f32, floor: f32) -> (f32, Vec<char>) {
    let fitted: Vec<_> = text.chars().collect();
    if text_width(&fitted, nominal) <= max_width {
        return (nominal, fitted);
    }
    let mut scale = nominal;
    loop {
        let fitted: Vec<_> = text.chars().collect();
        if text_width(&fitted, scale) <= max_width {
            return (scale, fitted);
        }
        if scale <= floor {
            break;
        }
        scale = (scale - 1.0).max(floor);
    }

    let mut fitted = Vec::new();
    let ellipsis = '…';
    let ellipsis_width = text_width(&[ellipsis], floor);
    if ellipsis_width > max_width {
        return (floor, fitted);
    }
    for character in text.chars() {
        let candidate_width =
            text_width(&fitted, floor) + text_width(&[character], floor) + ellipsis_width;
        if candidate_width > max_width {
            break;
        }
        fitted.push(character);
    }
    fitted.push(ellipsis);
    (floor, fitted)
}

fn text_width(characters: &[char], scale: f32) -> f64 {
    let scaled_font = FONT.as_scaled(PxScale::from(scale));
    characters
        .iter()
        .map(|character| f64::from(scaled_font.h_advance(FONT.glyph_id(*character))))
        .sum()
}

fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, colour: [u8; 3], alpha: u8) {
    if alpha == 0 {
        return;
    }
    let pixel = image.get_pixel_mut(x, y);
    let source_alpha = u32::from(alpha);
    let destination_alpha = u32::from(pixel.0[3]);
    let output_alpha = source_alpha + (destination_alpha * (255 - source_alpha) + 127) / 255;
    for (channel, source) in colour.into_iter().enumerate() {
        let numerator = u32::from(source) * source_alpha * 255
            + u32::from(pixel.0[channel]) * destination_alpha * (255 - source_alpha);
        pixel.0[channel] = u8::try_from((numerator + output_alpha * 127) / (output_alpha * 255))
            .expect("blended colour channel should fit in one byte");
    }
    pixel.0[3] = u8::try_from(output_alpha).expect("blended alpha should fit in one byte");
}

#[cfg(test)]
mod tests;
