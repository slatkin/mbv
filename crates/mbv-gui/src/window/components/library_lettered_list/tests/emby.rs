//! The Emby library's own contracts (`gui-tech-debt-simplification` design
//! D10): the plain mode's year as its secondary label, the feed-group mode's
//! date gutter and no rail, a collection row's play, and the row menu.

use super::*;

/// The feed-group mode's date-ordered rows, with a boxset collection row.
fn feed_rows() -> Vec<GuiEmbyRowFrame> {
    vec![
        GuiEmbyRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "f1".to_owned(),
            title: "Family".to_owned(),
            subtitle: None,
            date: Some("1998".to_owned()),
            sort_letter: None,
            kind: GuiEmbyRowKindFrame::Collection,
        },
        GuiEmbyRowFrame {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            id: "f2".to_owned(),
            title: "Fargo".to_owned(),
            subtitle: None,
            date: Some("1996".to_owned()),
            sort_letter: None,
            kind: GuiEmbyRowKindFrame::Media,
        },
    ]
}

/// Contract (`LibraryEmbyList.test.ts`): the plain mode paints the row's year
/// as its secondary label and no date gutter.
#[test]
fn the_plain_mode_paints_the_rows_year_as_its_secondary_label() {
    let mut fixture = built::<EmbyList>(
        EmbyList::frame(EmbyList::rows(), false, false),
        EmbyList::detail_of(Some("m1")),
    );
    fixture.measure(300.0);

    assert_eq!(
        fixture.text_of(&part_selector::<EmbyList>("m1", "subtitle")),
        "1979",
        "the year paints as the secondary label"
    );
    assert!(
        !fixture.presents("[data-library-emby-date]"),
        "the plain mode paints no date gutter"
    );
}

/// Contract (`LibraryEmbyList.test.ts`): the feed-group mode paints the flat
/// date-ordered rows with their gutter and no rail and no heading.
#[test]
fn the_feed_group_mode_paints_the_date_gutter_and_no_rail() {
    let mut fixture = built::<EmbyList>(emby_frame(true, feed_rows()), emby_detail(Some("f1")));
    fixture.measure(300.0);

    assert_eq!(fixture.painted_rows(), ["f1".to_owned(), "f2".to_owned()]);
    assert!(
        !fixture.presents("[data-library-emby-letter]"),
        "the feed-group mode paints no heading"
    );
    assert!(
        !fixture.presents("[data-library-region=\"rail\"]"),
        "the feed-group mode paints no rail"
    );
    assert_eq!(
        fixture.text_of(&part_selector::<EmbyList>("f1", "date")),
        "1998",
        "the date gutter paints"
    );
}

/// Contract (`LibraryEmbyList.test.ts`): a folder row plays through the core's
/// folder-play rule with no drill-in, so the double-click sends the same play
/// command as any other row.
#[test]
fn a_double_click_on_a_collection_row_plays_it() {
    let mut fixture = built::<EmbyList>(emby_frame(true, feed_rows()), emby_detail(None));

    fixture.play_row("f1");

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::EmbyPlay {
            lib_idx: 3,
            item_id: "f1".to_owned(),
        }],
        "the collection row plays with no drill-in"
    );
}

/// Contract (`LibraryEmbyList.test.ts`, design D8): a right-click opens the
/// item menu for the row's stable id, which the panel paints (D15). The
/// right-press rides the handler function (a headless document cannot fire a
/// context-menu event).
#[test]
fn a_right_click_on_a_row_sends_the_row_menu_command_with_its_id() {
    let mut fixture = built::<EmbyList>(
        EmbyList::frame(EmbyList::rows(), false, false),
        EmbyList::detail_of(Some("m1")),
    );

    let state = fixture.state.clone();
    fixture.in_runtime(move || {
        state.open_row_menu_at(
            Some(LibraryState::row_menu_item("m2".to_owned())),
            12.0,
            34.0,
        );
    });

    assert_eq!(
        fixture.library_commands(),
        [crate::gui_core::GuiLibraryCommand::RowMenu {
            row: crate::gui_core::GuiLibraryMenuRow::Item("m2".to_owned()),
        }],
        "the row-menu command names the row's item id"
    );
}
