//! The lettered list's common contracts (`gui-tech-debt-simplification`
//! design D10): each test runs one contract over the destinations that carry
//! it, so the assertions live once and no destination's facts are dropped.

use super::*;

/// The painted rows' hook word on one destination.
fn letter_attr<D: Host>() -> String {
    format!("data-library-{}-letter", D::ATTR)
}

/// The thumb hook word on one destination.
fn thumb_attr<D: Host>() -> String {
    format!("data-library-{}-thumb", D::ATTR)
}

/// The lettered destinations paint their letter headings, their rows, and the
/// rail beside them; a rail letter a row carries is enabled.
fn paints_lettered_list<D: Host>() {
    let mut fixture = built::<D>(
        D::frame(D::rows(), false, false),
        D::detail_of(Some(D::IDS[0])),
    );
    fixture.measure(300.0);

    let letter = letter_attr::<D>();
    assert_eq!(
        fixture.all_attrs(&format!("[{letter}]"), &letter),
        ["A".to_owned(), "B".to_owned()],
        "the headings paint"
    );
    assert_eq!(
        fixture.painted_rows(),
        D::IDS.map(str::to_owned),
        "the rows paint in order"
    );
    assert_eq!(
        fixture.attr_of("[data-library-rail-letter=\"B\"]", "disabled"),
        None,
        "the rail's B is enabled by the row under it"
    );

    let thumb = thumb_attr::<D>();
    assert_eq!(
        fixture.all_attrs(&part_selector::<D>(D::IDS[0], "thumb"), &thumb),
        ["loading".to_owned()],
        "the row paints the shared part's thumb with its art state"
    );
    assert_eq!(
        fixture.all_attrs(&part_selector::<D>(D::IDS[1], "thumb"), &thumb),
        ["none".to_owned()],
        "a row with no art source paints the no-source thumb"
    );
}

/// The flat destinations paint their date gutter, their rows, and no letter
/// heading and no rail.
fn paints_flat_list<D: Host>() {
    let mut fixture = built::<D>(
        D::frame(D::rows(), false, false),
        D::detail_of(Some(D::IDS[0])),
    );
    fixture.measure(300.0);

    assert_eq!(
        fixture.painted_rows(),
        D::IDS.map(str::to_owned),
        "the rows paint in order"
    );
    assert!(
        !fixture.presents("[data-library-region=\"rail\"]"),
        "the flat list paints no rail"
    );
    assert!(
        !fixture.presents(&format!("[{}]", letter_attr::<D>())),
        "the flat list paints no heading"
    );
    assert_eq!(
        fixture.text_of(&part_selector::<D>(D::IDS[1], "date")),
        D::SECOND_DATE,
        "the date gutter paints"
    );
    assert_eq!(
        fixture.text_of(&part_selector::<D>(D::IDS[0], "subtitle")),
        D::FIRST_SUBTITLE,
        "the entry paints its subtitle"
    );

    let thumb = thumb_attr::<D>();
    assert_eq!(
        fixture.all_attrs(&part_selector::<D>(D::IDS[0], "thumb"), &thumb),
        ["loading".to_owned()],
        "the row paints the shared part's thumb with its art state"
    );
    assert_eq!(
        fixture.all_attrs(&part_selector::<D>(D::IDS[1], "thumb"), &thumb),
        ["none".to_owned()],
        "a row with no art source paints the no-source thumb"
    );
}

/// The list reports the rows it paints under its own scope, so the core
/// queues their row art without a coordinate (gui-delineated-rows design D7).
fn reports_rows_in_view<D: Host>() {
    let mut fixture = built::<D>(D::frame(D::rows(), false, false), D::detail_of(None));
    fixture.measure(300.0);

    assert_eq!(
        fixture.reported_rows_in_view().last(),
        Some(&crate::gui_core::GuiLibraryCommand::RowsInView {
            scope: D::ART_SCOPE.into(),
            ids: D::IDS.map(str::to_owned).to_vec(),
        }),
        "the report names the painted rows under the destination's scope"
    );
}

/// A press sends the select command with the row id. The press rides the
/// handler function (a headless document cannot fire a DOM click).
fn press_sends_select<D: Host>() {
    let mut fixture = built::<D>(
        D::frame(D::rows(), false, false),
        D::detail_of(Some(D::IDS[0])),
    );
    fixture.measure(300.0);

    fixture.press_row(D::IDS[1]);

    assert_eq!(
        fixture.library_commands(),
        [D::select_command(D::IDS[1])],
        "the select command names the row id"
    );
}

/// A double-click plays the row. The double-click rides the handler function
/// (a headless document cannot fire a DOM click).
fn double_click_plays<D: Host>() {
    let mut fixture = built::<D>(
        D::frame(D::rows(), false, false),
        D::detail_of(Some(D::IDS[0])),
    );

    let id = D::IDS[1].to_owned();
    fixture.play_row(&id);

    assert_eq!(
        fixture.library_commands(),
        [D::play_command(D::IDS[1])],
        "the play command names the row id"
    );
}

/// The core's selected row paints its mark and its fill, and no row paints a
/// cursor — the list keeps none (`gui-pointer-target-interaction` D6).
fn selected_fill_and_no_cursor<D: Host>() {
    let mut fixture = built::<D>(
        D::frame(D::rows(), false, false),
        D::detail_of(Some(D::SELECTED)),
    );
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of(
            &row_selector::<D>(D::SELECTED),
            &format!("data-library-{}-selected", D::ATTR),
        ),
        Some(String::new()),
        "the selected row paints its mark"
    );
    assert!(
        fixture
            .attr_of(&row_selector::<D>(D::SELECTED), "style")
            .is_some_and(|style| style.contains("background")),
        "the selected row paints its fill"
    );
    assert!(
        !fixture.presents(&format!("[data-library-{}-cursor]", D::ATTR)),
        "no row paints a cursor"
    );
}

/// A list that has not landed paints the loading line.
fn loading_line_paints<D: Host>() {
    let fixture = built::<D>(D::frame(Vec::new(), true, false), D::detail_of(None));

    assert!(
        fixture.presents(&format!("[{}]", D::NOTICES.loading_attr)),
        "the loading line paints"
    );
}

/// A loading tab paints its line and no row.
fn loading_tab_paints_no_row<D: Host>() {
    let fixture = built::<D>(D::frame(Vec::new(), true, false), D::detail_of(None));

    assert!(
        fixture.presents(&format!("[{}]", D::NOTICES.loading_attr)),
        "the loading line paints"
    );
    assert!(
        fixture.painted_rows().is_empty(),
        "no row paints under the loading line"
    );
}

/// A failed fetch paints the failure line.
fn failed_line_paints<D: Host>() {
    let fixture = built::<D>(D::frame(Vec::new(), false, true), D::detail_of(None));

    assert!(
        fixture.presents(&format!("[{}]", D::NOTICES.failed_attr)),
        "the failure line paints"
    );
}

/// A landed list with no row paints the empty line.
fn empty_line_paints<D: Host>() {
    let fixture = built::<D>(D::frame(Vec::new(), false, false), D::detail_of(None));

    assert!(
        fixture
            .text_of(&format!("[{}]", D::NOTICES.empty_attr))
            .contains(D::NOTICES.empty_text),
        "the empty line says so"
    );
}

/// The header marks the destination's Service and paints no selection label:
/// the plain list is the whole library, and the UI is not verbose (design
/// D15).
fn header_without_selection_label<D: Host>() {
    let mut fixture = built::<D>(
        D::frame(D::rows(), false, false),
        D::detail_of(Some(D::IDS[0])),
    );
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of("[data-library-list-mark]", "data-library-list-mark"),
        Some(D::MARK.to_owned()),
        "the header marks the destination's Service"
    );
    assert!(
        !fixture.presents("[data-library-selection-label]"),
        "a whole library paints no selection label"
    );
}

/// The header marks the destination's Service and the collection dropdown's
/// trigger names the selection (design D15, D6a).
fn header_with_collections_trigger<D: Host>() {
    let mut fixture = built::<D>(D::frame(Vec::new(), false, false), D::detail_of(None));
    fixture.measure(300.0);

    assert_eq!(
        fixture.attr_of("[data-library-list-mark]", "data-library-list-mark"),
        Some(D::MARK.to_owned()),
        "the header marks the destination's Service"
    );
    assert_eq!(
        fixture.text_of("[data-library-collections-trigger]"),
        "All",
        "the trigger names All while the selection is All"
    );
}

/// A played row dims its title and an active one paints its resume percent.
fn played_and_active_rows<D: Host>() {
    let mut fixture = built::<D>(D::frame(D::rows(), false, false), D::detail_of(None));
    fixture.measure(300.0);

    assert!(
        fixture
            .attr_of(&part_selector::<D>(D::IDS[1], "title"), "style")
            .is_some_and(|style| style.contains("var(--ef-grey1)")),
        "the played row's title dims"
    );
    assert!(
        fixture.presents(&part_selector::<D>(D::IDS[2], "thumb-progress")),
        "the active row paints its resume percent"
    );
}

/// Contract (`LibraryEmbyList.test.ts`, `LibraryAudiobooksList.test.ts`): the
/// name-sorted lists paint their letter headings, their rows, and the rail
/// beside them.
#[test]
fn the_list_paints_its_letter_headings_its_rows_and_the_rail_beside_them() {
    paints_lettered_list::<EmbyList>();
    paints_lettered_list::<AudiobooksList>();
}

/// Contract (`LibraryPodcastsList.test.ts`, `LibraryFeedsList.test.ts`): the
/// date-ordered lists paint their date gutter with no heading and no rail.
#[test]
fn the_flat_list_paints_its_date_gutter_with_no_heading_and_no_rail() {
    paints_flat_list::<PodcastsList>();
    paints_flat_list::<FeedsList>();
}

/// Contract (gui-delineated-rows design D7): every list reports the rows it
/// paints under its own scope.
#[test]
fn the_list_reports_the_rows_in_view_under_its_scope() {
    reports_rows_in_view::<EmbyList>();
    reports_rows_in_view::<AudiobooksList>();
    reports_rows_in_view::<PodcastsList>();
    reports_rows_in_view::<FeedsList>();
}

/// Contract (`LibraryEmbyList.test.ts`, `LibraryAudiobooksList.test.ts`,
/// `LibraryPodcastsList.test.ts`, `LibraryFeedsList.test.ts`): a press sends
/// the select command with the row id.
#[test]
fn a_press_sends_the_select_command_with_the_row_id() {
    press_sends_select::<EmbyList>();
    press_sends_select::<AudiobooksList>();
    press_sends_select::<PodcastsList>();
    press_sends_select::<FeedsList>();
}

/// Contract (`LibraryEmbyList.test.ts`, `LibraryAudiobooksList.test.ts`,
/// `LibraryPodcastsList.test.ts`, `LibraryFeedsList.test.ts`): a double-click
/// plays the row.
#[test]
fn a_double_click_plays_the_row() {
    double_click_plays::<EmbyList>();
    double_click_plays::<AudiobooksList>();
    double_click_plays::<PodcastsList>();
    double_click_plays::<FeedsList>();
}

/// Contract (`LibraryAudiobooksList.test.ts`, `LibraryPodcastsList.test.ts`,
/// `LibraryFeedsList.test.ts`): the core's selected row carries its fill, and
/// no row paints a cursor.
#[test]
fn the_cores_selected_row_carries_its_fill_and_no_row_paints_a_cursor() {
    selected_fill_and_no_cursor::<AudiobooksList>();
    selected_fill_and_no_cursor::<PodcastsList>();
    selected_fill_and_no_cursor::<FeedsList>();
}

/// Contract (`LibraryEmbyList.test.ts`, `LibraryAudiobooksList.test.ts`): the
/// load fact paints the loading line.
#[test]
fn the_loading_line_paints() {
    loading_line_paints::<EmbyList>();
    loading_line_paints::<AudiobooksList>();
}

/// Contract (`LibraryPodcastsList.test.ts`, `LibraryFeedsList.test.ts`): a
/// loading tab paints its line and no row.
#[test]
fn a_loading_tab_paints_its_line_and_no_row() {
    loading_tab_paints_no_row::<PodcastsList>();
    loading_tab_paints_no_row::<FeedsList>();
}

/// Contract (`LibraryEmbyList.test.ts`, `LibraryAudiobooksList.test.ts`,
/// `LibraryPodcastsList.test.ts`): a failed fetch paints the failure line.
#[test]
fn the_failed_line_paints() {
    failed_line_paints::<EmbyList>();
    failed_line_paints::<AudiobooksList>();
    failed_line_paints::<PodcastsList>();
}

/// Contract (`LibraryEmbyList.test.ts`, `LibraryPodcastsList.test.ts`,
/// `LibraryFeedsList.test.ts`): a landed list with no row paints the empty
/// line.
#[test]
fn the_empty_line_paints() {
    empty_line_paints::<EmbyList>();
    empty_line_paints::<PodcastsList>();
    empty_line_paints::<FeedsList>();
}

/// Contract (`LibraryEmbyList.test.ts`, `LibraryAudiobooksList.test.ts`,
/// design D15): the list column carries the Music list's header without a
/// selection label.
#[test]
fn the_list_paints_its_header_without_the_selection_label() {
    header_without_selection_label::<EmbyList>();
    header_without_selection_label::<AudiobooksList>();
}

/// Contract (`LibraryPodcastsList.test.ts`, `LibraryFeedsList.test.ts`, design
/// D15, D6a): the list column carries the Music list's header with the
/// collection dropdown's trigger naming the selection.
#[test]
fn the_list_paints_its_header_with_the_collections_trigger() {
    header_with_collections_trigger::<PodcastsList>();
    header_with_collections_trigger::<FeedsList>();
}

/// Contract (`LibraryPodcastsList.test.ts`, `LibraryFeedsList.test.ts`): a
/// played row dims its title and an active one paints its resume percent.
#[test]
fn a_played_row_dims_its_title_and_an_active_one_paints_its_resume_percent() {
    played_and_active_rows::<PodcastsList>();
    played_and_active_rows::<FeedsList>();
}

/// A row that names no collection offers Play and Add and no Shuffle: the
/// sample rows of every lettered destination are books, episodes, entries, and
/// media, so the shared part paints the two controls alone.
fn offers_play_and_add<D: Host>() {
    let mut fixture = built::<D>(
        D::frame(D::rows(), false, false),
        D::detail_of(Some(D::IDS[0])),
    );
    fixture.measure(300.0);

    assert_eq!(
        fixture.hover_controls_of(D::IDS[0]),
        ["play", "queue"],
        "the row offers Play and Add and no Shuffle"
    );
}

/// Contract (gui-row-hover-actions design D3, D4): every lettered destination's
/// rows offer Play and Add on hover, wired to the destination's own play and
/// queue paths.
#[test]
fn every_row_offers_play_and_add() {
    offers_play_and_add::<EmbyList>();
    offers_play_and_add::<AudiobooksList>();
    offers_play_and_add::<PodcastsList>();
    offers_play_and_add::<FeedsList>();
}
