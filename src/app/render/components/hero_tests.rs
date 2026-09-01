use super::hero::{paint_hero_content, HeroContent, HeroImage, HeroLine};
use crate::app::palette;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn inline_hero_image_has_shared_top_right_and_gutter_geometry() {
    let backend = TestBackend::new(14, 6);
    let mut terminal = Terminal::new(backend).unwrap();
    let lines = [
        HeroLine::Plain("ABCDEFGHI".into()),
        HeroLine::Plain("JKLMNOPQR".into()),
        HeroLine::Plain("STUVWXYZ".into()),
    ];

    let mut image_rect = None;
    terminal
        .draw(|frame| {
            image_rect = paint_hero_content(
                frame,
                Rect::new(2, 1, 10, 4),
                &HeroContent {
                    title: Some("123456789"),
                    meta_line: None,
                    meta_color: palette::TEXT_DETAIL_META,
                    show_playing: false,
                    unconditional_spacer_after_meta: false,
                    lines: &lines,
                    image: Some(HeroImage {
                        actual_w: 3,
                        height: 2,
                    }),
                },
                true,
            )
            .img_rect;
        })
        .unwrap();

    assert_eq!(image_rect, Some(Rect::new(9, 1, 3, 2)));
    let buffer = terminal.backend().buffer();
    // The gutter is needed only while the image occupies the row; the row
    // immediately below the image resumes the full text width.
    for y in 1..=2 {
        assert_eq!(buffer[(8, y)].symbol(), " ", "missing gutter on row {y}");
    }
    assert_ne!(
        buffer[(8, 4)].symbol(),
        " ",
        "text did not resume full width"
    );
}
