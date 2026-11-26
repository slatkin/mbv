//! Headless tests of the Hero's fact pills (design D14, gui-hero-design-polish
//! design D5): the contracts of `hero.test.ts` that survive the one frame
//! (gui-hero-design-polish design D2) — a pill is typed segments, because colour
//! carries the data type. The frame's own contracts ride
//! `library_hero_frame/tests.rs`.

use super::*;

/// One meta segment.
fn segment(text: &str, kind: GuiMetaKindFrame) -> GuiMetaSegmentFrame {
    GuiMetaSegmentFrame {
        text: text.to_owned(),
        kind,
    }
}

// The Music track pills type their count as count and their total as time, in
// two pills.
#[test]
fn the_track_total_pills_type_their_count_as_count_and_their_total_as_time() {
    let pills = track_total_pills(&GuiMusicTrackTotalFrame {
        count: 13,
        duration: "44:25".to_owned(),
    });

    assert_eq!(
        pills,
        vec![
            GuiHeroPillFrame {
                segments: vec![
                    segment("13", GuiMetaKindFrame::Count),
                    segment(" tracks", GuiMetaKindFrame::Plain),
                ],
                link: None,
            },
            GuiHeroPillFrame {
                segments: vec![segment("44:25", GuiMetaKindFrame::Time)],
                link: None,
            },
        ]
    );
}
