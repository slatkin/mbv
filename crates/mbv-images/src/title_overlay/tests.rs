use super::{TitleOverlayColours, TitleOverlayText, compose_title_overlay, split_one_part_title};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use ratatui_image::FontSize;
use rstest::rstest;

/// Contract: a one-part title splits at the space nearest its middle.
#[rstest]
#[case::even_split("ab cd ef gh", Some(("ab cd", "ef gh")))]
#[case::tie_picks_the_earlier_space("Hello Brave World", Some(("Hello", "Brave World")))]
#[case::no_space_gives_none("Hello", None)]
fn splits_at_the_space_nearest_the_middle(
    #[case] title: &str,
    #[case] expected: Option<(&str, &str)>,
) {
    assert_eq!(split_one_part_title(title), expected);
}

/// Contract: a long one-part title paints its second half in the bottom row.
#[test]
fn long_one_part_title_paints_the_bottom_row() {
    let base = DynamicImage::ImageRgba8(RgbaImage::from_pixel(64, 48, Rgba([200, 200, 200, 255])));
    let composed = compose_title_overlay(
        &base,
        None,
        FontSize::new(8, 16),
        TitleOverlayText {
            context: None,
            title: "Strawberry Breakfast",
        },
        TitleOverlayColours {
            context: [255, 215, 0],
            title: [255, 250, 240],
        },
    );
    let (width, height) = base.dimensions();
    assert_ne!(
        composed.get_pixel(0, height - 1),
        base.get_pixel(0, height - 1),
        "the bottom row keeps its scrim for a split one-part title on a {width}x{height} image"
    );
}
