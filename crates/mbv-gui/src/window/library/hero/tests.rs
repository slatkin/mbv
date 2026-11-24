//! Headless tests of the Hero header and block mapping (D16): the contracts
//! of `hero.test.ts` — the largest unit is the header's big title, plain
//! facts ride its one line, and only the duration and progress rows sit
//! beside the art. Every line is typed segments, because colour carries the
//! data type (add-gui-settings-screen design D14).

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

// An item with a parent puts the parent in the header and the item in the
// block.
#[test]
fn an_item_with_a_parent_puts_the_parent_in_the_header_and_the_item_in_the_block() {
    let layout = hero_layout(
        &HeroLayoutFacts {
            title: "B New",
            parent: Some("Show B"),
            meta_rows: &[
                vec![segment("0 h 54 m", GuiMetaKindFrame::Time)],
                vec![segment("08 Oct", GuiMetaKindFrame::Date)],
            ],
            duration_row: Some(0),
            progress_row: None,
            overview: None,
            episode: true,
        },
        "podcast",
    );

    assert_eq!(text(&layout.header.kicker), "podcast");
    assert_eq!(
        layout.header.kicker,
        vec![segment("podcast", GuiMetaKindFrame::Label)]
    );
    assert_eq!(layout.header.title, "Show B");
    assert_eq!(
        layout.header.facts,
        vec![segment("08 Oct", GuiMetaKindFrame::Date)]
    );
    assert_eq!(layout.block.name, Some("B New".to_owned()));
    assert!(layout.block.name_episode, "an episode's name line");
    assert_eq!(
        layout.block.meta,
        vec![HeroMetaRow {
            segments: vec![segment("0 h 54 m", GuiMetaKindFrame::Time)],
            role: HeroMetaRole::Duration,
        }]
    );
}

// An item that is its own largest unit titles the header and leaves the block
// unnamed.
#[test]
fn an_item_that_is_its_own_largest_unit_titles_the_header_and_leaves_the_block_unnamed() {
    let layout = hero_layout(
        &HeroLayoutFacts {
            title: "The Caves of Steel",
            parent: None,
            meta_rows: &[
                vec![segment("Isaac Asimov", GuiMetaKindFrame::Plain)],
                vec![segment("9 h 51 m", GuiMetaKindFrame::Time)],
                vec![segment("17%", GuiMetaKindFrame::Plain)],
            ],
            duration_row: Some(1),
            progress_row: Some(2),
            overview: None,
            episode: false,
        },
        "audiobook",
    );

    assert_eq!(text(&layout.header.kicker), "audiobook");
    assert_eq!(layout.header.title, "The Caves of Steel");
    assert_eq!(text(&layout.header.facts), "Isaac Asimov");
    assert_eq!(layout.block.name, None);
    assert!(!layout.block.name_episode, "a book is no episode");
    let roles: Vec<HeroMetaRole> = layout.block.meta.iter().map(|row| row.role).collect();
    assert_eq!(roles, [HeroMetaRole::Duration, HeroMetaRole::Progress]);
}

// An item with no plain rows leaves the header facts empty.
#[test]
fn an_item_with_no_plain_rows_leaves_the_header_facts_empty() {
    let layout = hero_layout(
        &HeroLayoutFacts {
            title: "a-new",
            parent: Some("Feed A"),
            meta_rows: &[],
            duration_row: None,
            progress_row: None,
            overview: None,
            episode: true,
        },
        "feed",
    );

    assert_eq!(layout.header.facts, Vec::new());
}

// The Music facts line joins a count and a time (design D14): the count
// paints the count token and the total paints the time token.
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
