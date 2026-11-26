//! The blurred-artwork backdrop (design D4 of `gui-hero-design-polish`): the
//! pure bytes-to-JPEG step the `mbv-art` backdrop path's worker thread runs
//! off the core thread. The Hero's own artwork, cover-fitted to the pane,
//! blurred into soft colour fields, and baked over the pane background with a
//! per-pixel scrim, so the window paints one opaque image: no CSS opacity,
//! gradient, filter, or blend anywhere.

use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::{DynamicImage, RgbImage};

mod worker;

#[cfg(test)]
mod tests;

pub(crate) use worker::BackdropWorker;

/// One backdrop request: the image-cache key of the source art, the baked
/// width, and the style. The request path carries all three, so a pane
/// resize or a style change is a different request and a different cache entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackdropRequest {
    pub(crate) key: String,
    pub(crate) width: u32,
    pub(crate) style: BackdropStyle,
}

impl BackdropRequest {
    /// The path after `backdrop/`: `{style}/{width}/{encoded key}`.
    pub(crate) fn path_tail(&self) -> String {
        format!(
            "{}/{}/{}",
            self.style.code(),
            self.width,
            super::percent_encode(&self.key)
        )
    }

    /// Parse what [`Self::path_tail`] wrote; `None` when malformed.
    pub(crate) fn parse_tail(tail: &str) -> Option<Self> {
        let (style, rest) = tail.split_once('/')?;
        let (width, key) = rest.split_once('/')?;
        Some(Self {
            key: super::percent_decode(key)?,
            width: width.parse().ok().filter(|width| *width > 0)?,
            style: BackdropStyle::from_code(style)?,
        })
    }

    /// The id the worker's cache and the disk cache key on: the source key,
    /// the baked size, the scrim strength, and the bake's version. A change to
    /// any of them is a different image.
    pub(crate) fn cache_id(&self) -> String {
        format!(
            "backdrop:{}:{}:{BACKDROP_HEIGHT}:{}:v1",
            self.key,
            self.width,
            self.style.strength()
        )
    }
}

/// The pane's solid background, `#272e33`: the colour every scrim fades to.
pub(crate) const PANE_BG: [u8; 3] = [39, 46, 51];

/// The backdrop's height in CSS pixels.
pub(crate) const BACKDROP_HEIGHT: u32 = 560;

/// The width step: the pane width rounds up to the next multiple, so a
/// resize does not bake a new image every frame.
pub(crate) const WIDTH_STEP: u32 = 200;

/// The source's width before the blur, the size a tiny `maxWidth=32` fetch
/// would give: it makes the blur cheap and strong.
const SOURCE_WIDTH: u32 = 32;

/// The blur's sigma at the output resolution.
const BLUR_SIGMA: f32 = 8.0;

/// The blur runs at a quarter of the output size with a quarter of the sigma,
/// which is the same blur at a sixteenth of the pixels.
const BLUR_DOWNSCALE: u32 = 4;

/// One in the scrim's 16.16 fixed-point arithmetic.
const FIXED_ONE: i64 = 1 << 16;

/// The title colour `#ece3cc`, the contrast reference.
const TITLE_RGB: [u8; 3] = [0xec, 0xe3, 0xcc];

/// The least contrast the title area keeps against the title colour.
const MIN_TITLE_CONTRAST: f64 = 4.5;

/// What kind of artwork the backdrop blurs: it picks the scrim strength and
/// the vertical crop anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackdropStyle {
    /// A poster or a backdrop image: scrim 0.55, cropped 30% from the top.
    Video,
    /// Square album art: scrim 0.45, cropped from the middle.
    Music,
}

impl BackdropStyle {
    fn strength(self) -> f64 {
        match self {
            Self::Video => 0.55,
            Self::Music => 0.45,
        }
    }

    /// The share of the spare height cropped from the top, in percent.
    fn anchor_percent(self) -> u32 {
        match self {
            Self::Video => 30,
            Self::Music => 50,
        }
    }

    /// The one-letter spelling the request path carries.
    pub(crate) fn code(self) -> char {
        match self {
            Self::Video => 'v',
            Self::Music => 'm',
        }
    }

    /// The style a request path's letter names.
    pub(crate) fn from_code(code: &str) -> Option<Self> {
        match code {
            "v" => Some(Self::Video),
            "m" => Some(Self::Music),
            _ => None,
        }
    }
}

/// The baked width for a pane `pane_width` CSS pixels wide: the next
/// [`WIDTH_STEP`] up, and never zero.
pub(crate) fn baked_width(pane_width: f32) -> u32 {
    let width: u32 = pane_width
        .max(1.0)
        .ceil()
        .to_string()
        .parse()
        .unwrap_or(WIDTH_STEP);
    width.div_ceil(WIDTH_STEP) * WIDTH_STEP
}

/// The backdrop JPEG for the cached source `bytes`: `None` when the bytes do
/// not decode or the JPEG does not encode. A source whose title area would
/// fall under [`MIN_TITLE_CONTRAST`] bakes with a weaker scrim strength
/// stepped down until it passes.
pub(crate) fn backdrop_jpeg(bytes: &[u8], width: u32, style: BackdropStyle) -> Option<Vec<u8>> {
    let source = image::load_from_memory(bytes).ok()?.into_rgb8();
    let blurred = blurred_fields(&source, width.max(1), style);
    let mut strength = style.strength();
    let mut baked = bake(&blurred, strength);
    while title_contrast(&baked) < MIN_TITLE_CONTRAST && strength > 0.01 {
        strength *= 0.9;
        baked = bake(&blurred, strength);
    }
    if (strength - style.strength()).abs() > f64::EPSILON {
        tracing::info!(
            strength,
            "the backdrop scrim was reduced to keep the title legible"
        );
    }
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 88)
        .encode_image(&DynamicImage::ImageRgb8(baked))
        .ok()?;
    Some(jpeg)
}

/// The cover-fitted, blurred artwork at `width` by [`BACKDROP_HEIGHT`], with
/// no scrim.
fn blurred_fields(source: &RgbImage, width: u32, style: BackdropStyle) -> RgbImage {
    let tiny = if source.width() > SOURCE_WIDTH {
        let height =
            u64::from(source.height()) * u64::from(SOURCE_WIDTH) / u64::from(source.width());
        let height = u32::try_from(height).unwrap_or(1).max(1);
        imageops::resize(source, SOURCE_WIDTH, height, FilterType::Triangle)
    } else {
        source.clone()
    };
    // Cover-fit: the smallest size at least `width` by the backdrop height
    // that keeps the source's aspect ratio.
    let (tiny_width, tiny_height) = (u64::from(tiny.width()), u64::from(tiny.height()));
    let fit_width = (tiny_width * u64::from(BACKDROP_HEIGHT)).div_ceil(tiny_height);
    let fit_height = (tiny_height * u64::from(width)).div_ceil(tiny_width);
    let big_width = u32::try_from(fit_width).unwrap_or(u32::MAX).max(width);
    let big_height = u32::try_from(fit_height)
        .unwrap_or(u32::MAX)
        .max(BACKDROP_HEIGHT);
    let big = imageops::resize(&tiny, big_width, big_height, FilterType::CatmullRom);
    let x0 = (big_width - width) / 2;
    let y0 = (big_height - BACKDROP_HEIGHT) * style.anchor_percent() / 100;
    let cropped = imageops::crop_imm(&big, x0, y0, width, BACKDROP_HEIGHT).to_image();
    let small = imageops::resize(
        &cropped,
        (width / BLUR_DOWNSCALE).max(1),
        (BACKDROP_HEIGHT / BLUR_DOWNSCALE).max(1),
        FilterType::Triangle,
    );
    let soft = imageops::blur(&small, BLUR_SIGMA / 4.0);
    imageops::resize(&soft, width, BACKDROP_HEIGHT, FilterType::Triangle)
}

/// A 0..=1 fraction in 16.16 fixed point.
fn fixed(fraction: f64) -> i64 {
    (fraction * 65536.0)
        .round()
        .to_string()
        .parse()
        .unwrap_or(0)
}

/// The blurred fields over [`PANE_BG`] through the scrim: full at the top,
/// none at the bottom row, and heavier on the left behind the title. The
/// strength folds into the row fade, so a pixel's alpha is two fixed-point
/// factors.
fn bake(fields: &RgbImage, strength: f64) -> RgbImage {
    let (width, height) = fields.dimensions();
    let last_row = f64::from(height.saturating_sub(1).max(1));
    let row_fade: Vec<i64> = (0..height)
        .map(|y| fixed(strength * (1.0 - f64::from(y) / last_row).powf(1.4)))
        .collect();
    let column_fade: Vec<i64> = (0..width)
        .map(|x| fixed(0.55 + 0.45 * (f64::from(x) / (0.6 * f64::from(width))).min(1.0)))
        .collect();
    let mut out = RgbImage::new(width, height);
    for (x, y, pixel) in fields.enumerate_pixels() {
        let alpha = row_fade[y as usize] * column_fade[x as usize] / FIXED_ONE;
        let mut baked = [0u8; 3];
        for (channel, slot) in baked.iter_mut().enumerate() {
            let bg = i64::from(PANE_BG[channel]);
            let value = bg
                + ((i64::from(pixel[channel]) - bg) * alpha + FIXED_ONE / 2).div_euclid(FIXED_ONE);
            *slot = u8::try_from(value.clamp(0, 255)).unwrap_or(0);
        }
        out.put_pixel(x, y, image::Rgb(baked));
    }
    out
}

/// The least contrast of the title area (the top-left 60% by 120 px) against
/// the title colour: the brightest pixel decides.
fn title_contrast(baked: &RgbImage) -> f64 {
    let area_width = baked.width() * 6 / 10;
    let area_height = 120.min(baked.height());
    let brightest = (0..area_height)
        .flat_map(|y| (0..area_width).map(move |x| (x, y)))
        .map(|(x, y)| luminance(baked.get_pixel(x, y).0))
        .fold(0.0_f64, f64::max);
    (luminance(TITLE_RGB) + 0.05) / (brightest + 0.05)
}

/// The WCAG relative luminance of an sRGB colour.
fn luminance(rgb: [u8; 3]) -> f64 {
    let linear = |channel: u8| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgb[0]) + 0.7152 * linear(rgb[1]) + 0.0722 * linear(rgb[2])
}
