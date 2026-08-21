use super::*;
// Characterization coverage stays beside the moved detail component.

#[test]
fn content_rows_is_never_shorter_than_the_rendered_image_height() {
    let short_text_layout = CompactBannerLayout {
        meta_line: None,
        show_playing: false,
        lines: vec!["A short overview.".to_string()],
        director_line_idx: None,
        img_actual_w: 18,
        img_height: 12,
        img_is_placeholder: false,
    };
    assert_eq!(
        short_text_layout.content_rows(),
        12,
        "banner must reserve at least the image's height even when the \
         wrapped text alone would need far fewer rows"
    );

    let tall_text_layout = CompactBannerLayout {
        meta_line: Some("Crime  1974  1h33m".to_string()),
        show_playing: false,
        lines: vec!["line".to_string(); 20],
        director_line_idx: None,
        img_actual_w: 18,
        img_height: 12,
        img_is_placeholder: false,
    };
    assert_eq!(
        tall_text_layout.content_rows(),
        21,
        "when the text is taller than the image, the image must not \
         clip the banner back down to its own height"
    );

    let no_image_layout = CompactBannerLayout {
        meta_line: None,
        show_playing: false,
        lines: vec!["A short overview.".to_string()],
        director_line_idx: None,
        img_actual_w: 0,
        img_height: 0,
        img_is_placeholder: false,
    };
    assert_eq!(
        no_image_layout.content_rows(),
        1,
        "with no image (e.g. images disabled), sizing stays text-only"
    );
}
