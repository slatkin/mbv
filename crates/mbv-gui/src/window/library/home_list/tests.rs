//! Headless tests of the Home list's row model (D16): the contracts of
//! `home-list.test.ts` — the sections' heading rows and item rows at fixed
//! heights over the spacer, and the opaque row identity as a key. The model is
//! plain data, so the geometry is asserted without a layout.

use super::*;
use crate::bridge::frame::library::GuiHomeItemRowFrame;
use crate::bridge::frame::library::GuiRowMediaKindFrame;
use crate::window::library::list::{ListWindow, list_window};

/// The `Continue watching` row the fixture carries: an Emby item with its
/// resume percent.
fn continued() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Continue("e1".to_owned()),
        title: "Better Call Saul".to_owned(),
        subtitle: Some("S3E4 · Sabrosito".to_owned()),
        date: None,
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Active { progress: Some(42) },
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The second `Continue watching` row: a played item with its art.
fn watched() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Continue("e2".to_owned()),
        title: "The Wire".to_owned(),
        subtitle: None,
        date: None,
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Played,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// One `Recently added` row: the Feed content id the key encodes.
fn added() -> GuiHomeItemRowFrame {
    GuiHomeItemRowFrame {
        id: GuiHomeRowIdFrame::Recent(QueueItemContentId::Feed("f1".to_owned())),
        title: "A Feed Entry".to_owned(),
        subtitle: Some("The Feed".to_owned()),
        date: Some("12 May".to_owned()),
        kind: GuiHomeRowKindFrame::Media,
        state: GuiHomeRowStateFrame::Ordinary,
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
    }
}

/// The carried rows the fixture paints: the two sections in order, with the
/// spacer between them.
fn rows() -> Vec<GuiHomeRowFrame> {
    vec![
        GuiHomeRowFrame::Heading {
            text: "Continue watching".to_owned(),
        },
        GuiHomeRowFrame::Item(continued()),
        GuiHomeRowFrame::Item(watched()),
        GuiHomeRowFrame::Spacer,
        GuiHomeRowFrame::Heading {
            text: "Recently added".to_owned(),
        },
        GuiHomeRowFrame::Item(added()),
    ]
}

/// The painted row's kind.
fn kind_of(row: &HomeListRow) -> &'static str {
    match row {
        HomeListRow::Heading { .. } => "heading",
        HomeListRow::Spacer { .. } => "spacer",
        HomeListRow::Item { .. } => "item",
    }
}

// The sections paint theirs heading rows, items, and the spacer in order.
#[test]
fn the_sections_paint_their_heading_rows_items_and_the_spacer_in_order() {
    let painted = build_home_rows(&rows(), "");

    let kinds: Vec<&str> = painted.iter().map(kind_of).collect();
    assert_eq!(
        kinds,
        ["heading", "item", "item", "spacer", "heading", "item"]
    );
    let heights: Vec<f32> = painted.iter().map(GeometryRow::height).collect();
    assert_eq!(
        heights,
        [
            HOME_HEADING_ROW_HEIGHT,
            HOME_ITEM_ROW_HEIGHT,
            HOME_ITEM_ROW_HEIGHT,
            HOME_SPACER_ROW_HEIGHT,
            HOME_HEADING_ROW_HEIGHT,
            HOME_ITEM_ROW_HEIGHT,
        ]
    );
    let tops: Vec<f32> = painted.iter().map(GeometryRow::top).collect();
    assert_eq!(tops, [0.0, 28.0, 76.0, 124.0, 144.0, 172.0]);
    let HomeListRow::Heading { text: first, .. } = &painted[0] else {
        panic!("a heading opens the list");
    };
    assert_eq!(first, "Continue watching");
    let HomeListRow::Heading { text: second, .. } = &painted[4] else {
        panic!("the second section carries a heading");
    };
    assert_eq!(second, "Recently added");
}

// The stale-frame contract (design D12): a command must resolve the row the
// frame painted or nothing, so the key is a pure function of the opaque
// identity alone — equal identities key equal, distinct identities key
// distinct.
#[test]
fn the_row_key_is_the_opaque_identity_distinct_per_section() {
    let continue_key = home_row_key(&GuiHomeRowIdFrame::Continue("e1".to_owned()));
    let recent_key = home_row_key(&GuiHomeRowIdFrame::Recent(QueueItemContentId::Feed(
        "f1".to_owned(),
    )));
    let episode_key = home_row_key(&GuiHomeRowIdFrame::Recent(
        QueueItemContentId::Audiobookshelf {
            library_item_id: "li1".to_owned(),
            episode_id: "ep1".to_owned(),
        },
    ));

    assert_ne!(continue_key, recent_key);
    assert_ne!(episode_key, recent_key);
    assert_eq!(
        home_row_key(&GuiHomeRowIdFrame::Recent(QueueItemContentId::Feed(
            "f1".to_owned()
        ))),
        recent_key
    );
}

// The window paints only the rows in view.
#[test]
fn the_window_paints_only_the_rows_in_view() {
    let painted = build_home_rows(&rows(), "");

    assert_eq!(
        list_window(&painted, 0.0, 80.0),
        ListWindow { start: 0, end: 3 }
    );
}
