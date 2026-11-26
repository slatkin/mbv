//! The Emby library's own contracts (`gui-tech-debt-simplification` design
//! D10): the plain mode's year as its secondary label, the feed-group mode's
//! date gutter and no rail, a collection row's play, and the row menu.

use super::*;

/// One feed-group row.
fn feed_row(id: &str, title: &str, year: &str, kind: GuiEmbyRowKindFrame) -> GuiEmbyRowFrame {
    GuiEmbyRowFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: None,
        date: Some(year.to_owned()),
        sort_letter: None,
        kind,
        state: GuiHomeRowStateFrame::Ordinary,
    }
}

/// The feed-group mode's date-ordered rows, with a boxset collection row.
fn feed_rows() -> Vec<GuiEmbyRowFrame> {
    vec![
        feed_row("f1", "Family", "1998", GuiEmbyRowKindFrame::Collection),
        feed_row("f2", "Fargo", "1996", GuiEmbyRowKindFrame::Media),
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

/// Contract (gui-emby-row-played-marks design D3, `gui-client` spec "An Emby
/// library row carries the shared play state"): the painter passes the row's
/// carried state to the flat row, so a played row paints the played mark and
/// the played title weight, and an ordinary row paints the unplayed mark.
#[test]
fn a_played_row_paints_the_played_mark_and_weight_and_an_ordinary_row_the_unplayed_mark() {
    let mut rows = EmbyList::rows();
    rows[0].state = GuiHomeRowStateFrame::Played;
    let mut fixture = built::<EmbyList>(
        EmbyList::frame(rows, false, false),
        EmbyList::detail_of(None),
    );
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of(
            &format!("{} [data-play-mark]", row_selector::<EmbyList>("m1")),
            "data-play-mark",
        ),
        Some("played".to_owned()),
        "the played row paints the played mark"
    );
    assert!(
        fixture
            .attr_of(&part_selector::<EmbyList>("m1", "title"), "style")
            .is_some_and(|style| style.contains("font-weight: 400")),
        "the played row paints the played title weight"
    );
    assert_eq!(
        fixture.attr_of(
            &format!("{} [data-play-mark]", row_selector::<EmbyList>("m2")),
            "data-play-mark",
        ),
        Some("unplayed".to_owned()),
        "the ordinary row paints the unplayed mark"
    );
}

/// Contract (gui-row-hover-actions design D3, D4): an Emby collection row's
/// menu carries Shuffle, so its hover offers Play, Add, and Shuffle; a media
/// row offers Play and Add alone.
#[test]
fn a_collection_row_offers_shuffle_and_a_media_row_does_not() {
    let mut fixture = built::<EmbyList>(emby_frame(true, feed_rows()), emby_detail(None));
    fixture.measure(300.0);

    assert_eq!(
        fixture.hover_controls_of("f1"),
        ["play", "queue", "shuffle"],
        "the collection row offers Shuffle"
    );
    assert_eq!(
        fixture.hover_controls_of("f2"),
        ["play", "queue"],
        "the media row offers Play and Add only"
    );
}
