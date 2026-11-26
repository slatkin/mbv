//! Headless tests of the Hero's facts lines (design D14): the contracts of
//! `hero.test.ts` that survive the one frame (gui-hero-design-polish design
//! D2) — a line is typed segments, because colour carries the data type. The
//! frame's own contracts ride `library_hero_frame/tests.rs`.

use super::*;
use crate::bridge::frame::library::GuiMusicTrackTotalFrame;

/// One line's text, without its types.
fn text(segments: &Segments) -> String {
    segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect()
}

/// One meta segment.
fn segment(text: &str, kind: GuiMetaKindFrame) -> GuiMetaSegmentFrame {
    GuiMetaSegmentFrame {
        text: text.to_owned(),
        kind,
    }
}

// The Music facts line types its count as count and its total as time.
#[test]
fn the_music_facts_line_types_its_count_as_count_and_its_total_as_time() {
    let facts = track_total_facts(&GuiMusicTrackTotalFrame {
        count: 13,
        duration: "44:25".to_owned(),
    });

    assert_eq!(
        facts,
        vec![
            segment("13", GuiMetaKindFrame::Count),
            segment(" tracks · ", GuiMetaKindFrame::Plain),
            segment("44:25", GuiMetaKindFrame::Time),
        ]
    );
    assert_eq!(text(&facts), "13 tracks · 44:25");
}
