//! The Search contracts (add-gui-library-panel design D12, row 12.1): the
//! top bar's field sends a query, the GUI tick host's drain applies the landed
//! result, the next frame carries its rows, an activated row opens its own
//! destination, and the close clears the section.
//!
//! The result boundary is the App's own search channel, so the test owns every
//! row it asserts and no search worker runs behind it; the activation's Emby
//! answers come from the in-memory mock transport, so no request leaves the
//! process and no live Service runs (design D13).

use mbv::{ClientTickHost, GuiDestinationKind};
use mbv_emby_model::EmbyItem;
use mbv_emby_model::test_support::make_item;
use mbv_gui::bridge::frame::GuiFrame;
use mbv_gui::bridge::frame::library::GuiSearchFrame;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::tab_selection::TabSelection;

use super::{LIB_EVENT_BOUND, library_of, send_library};
use crate::{TestCore, test_core_with, test_emby_client};

/// The query the tests type into the field.
const QUERY: &str = "blue";

/// The movie id the mocked search returns, and the id the mock transport
/// answers the activation's reveal fetches with.
const MOVIE_ID: &str = "m-blue";

/// The plain Movies library's position: the seeded Music library is 0, and the
/// pushed Movies library is 1, so the activation's destination differs from
/// the one the query was typed on.
const MOVIES_LIB: usize = 1;

/// One movie's reveal record, as the mock transport returns it: the item the
/// search result names, in the shape `parse_item` reads.
const MOVIE: &str = r#"{"Id":"m-blue","Name":"Blue Train","Type":"Movie","MediaType":"Video"}"#;

/// A core over an App whose Search field has sent `QUERY`, with the Emby
/// Service attached and a Movies library beside the seeded Music one. The
/// query lands while no Service is attached, so the production worker starts
/// nothing and the seam owns the result the test asserts.
fn search_core(http: &MockHttp) -> TestCore {
    let (app, _cmd_rx) = mbv::stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    send_library(
        &mut test,
        GuiLibraryCommand::SearchQuery {
            query: QUERY.into(),
        },
    );
    let core = test.core.app_mut();
    core.seed_gui_music_library_for_test(test_emby_client(http));
    core.push_gui_library_tab_for_test(EmbyItem::folder(
        "lib-movies".into(),
        "Movies".into(),
        "movies".into(),
    ));
    test
}

/// Script the Emby answers the activation's navigation reads (row 12.1): the
/// reveal item, its empty ancestor chain (so the landing is the library level
/// itself), and the library level that holds the movie.
fn script_navigation(http: &MockHttp) {
    http.respond_when("Ancestors", 200, "[]");
    http.respond_when(
        "Ids=m-blue",
        200,
        &format!(r#"{{"Items":[{MOVIE}],"TotalRecordCount":1}}"#),
    );
    http.respond_when(
        "ParentId",
        200,
        &format!(r#"{{"Items":[{MOVIE}],"TotalRecordCount":1}}"#),
    );
}

/// Land the mocked search result on the App's search channel, as the search
/// worker does, and run the step whose tick drains it.
fn land_result(test: &mut TestCore) {
    let mut movie = make_item("Blue Train", "Movie");
    movie.id = MOVIE_ID.into();
    test.core
        .app_mut()
        .send_gui_search_result_for_test(QUERY, vec![movie]);
    assert!(test.core.step(), "the core thread never quits mid-drain");
}

/// The Search section a frame carries (row 12.1).
fn search_of(frame: GuiFrame) -> GuiSearchFrame {
    library_of(frame)
        .search
        .expect("the frame carries the Search section")
}

/// Pump steps until the newest frame's Library content is the plain Emby
/// destination, waiting for the next library completion instead of sleeping:
/// the activation's navigation lands on the shared library channel.
fn pump_until_movies_destination(test: &mut TestCore) {
    loop {
        assert!(test.core.step(), "the core thread never quits mid-drain");
        if test.core.app_mut().gui_active_destination()
            == Some(GuiDestinationKind::Library {
                lib_idx: MOVIES_LIB,
            })
        {
            return;
        }
        assert!(
            test.core
                .app_mut()
                .drain_lib_event_blocking_for_test(LIB_EVENT_BOUND),
            "no library completion landed while waiting for the activated row's destination"
        );
        test.waker.wake();
    }
}

/// The newest frame, built with the Library content re-carried by a layout
/// report, as a window content reload sends one: the read sees the destination
/// the activation opened, whatever the memo carried before.
fn reloaded_frame(test: &mut TestCore) -> GuiFrame {
    test.tx
        .send(crate::measured_layout(10, Vec::new()))
        .unwrap();
    assert!(test.core.step(), "the core thread never quits mid-drain");
    test.latest_frame().expect("the report emitted a frame")
}

/// Contract: a landed query fills the store the GUI tick host drains, and the
/// next frame carries the query with the provider's rows — the identity an
/// activate command sends back (row 12.1).
#[test]
fn the_search_result_reaches_the_next_frame() {
    let http = MockHttp::new();
    let mut test = search_core(&http);
    land_result(&mut test);

    let search = search_of(test.latest_frame().expect("the drain emitted a frame"));
    assert_eq!(search.query, QUERY);
    assert!(!search.loading);
    assert!(!search.failed);
    assert_eq!(search.results.len(), 1);
    assert_eq!(search.results[0].id, MOVIE_ID);
    assert_eq!(search.results[0].item_type, "Movie");
    assert_eq!(search.results[0].title, "Blue Train");
}

/// Contract: activating a result opens the destination its item type names,
/// through the TUI's own navigation (row 12.1) — the App's destination becomes
/// the plain Emby Movies library, which the seeded Music one is not.
#[test]
fn activating_a_result_opens_its_destination() {
    let http = MockHttp::new();
    script_navigation(&http);
    let mut test = search_core(&http);
    land_result(&mut test);
    let row = search_of(test.latest_frame().expect("the drain emitted a frame"))
        .results
        .first()
        .cloned()
        .expect("the result list holds the found row");

    send_library(
        &mut test,
        GuiLibraryCommand::SearchActivate {
            id: row.id,
            item_type: row.item_type,
        },
    );
    pump_until_movies_destination(&mut test);

    assert_eq!(
        test.core.app_mut().gui_active_destination(),
        Some(GuiDestinationKind::Library {
            lib_idx: MOVIES_LIB,
        }),
        "the activated row's own destination is active"
    );
    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::EmbyLibrary(MOVIES_LIB),
        "the navigation switched the Emby tab"
    );
    assert_eq!(
        test.core.app_mut().tv_root_item_ids_for_test(MOVIES_LIB),
        Some(vec![MOVIE_ID.to_string()]),
        "the TUI's navigation landed the library level on the activated item"
    );
    let frame = reloaded_frame(&mut test);
    let library = library_of(frame);
    assert!(
        library
            .content
            .as_ref()
            .is_some_and(|content| content.emby.is_some()),
        "the frame paints the activated row's destination"
    );
    assert!(
        library.search.is_none(),
        "the activation closes the results, as the TUI's dismiss unmounts them"
    );
}

/// Contract: the close command clears the section, so the next frame carries
/// `null` and the window paints no results (row 12.1).
#[test]
fn the_close_command_clears_the_section() {
    let http = MockHttp::new();
    let mut test = search_core(&http);
    land_result(&mut test);
    let landed = search_of(test.latest_frame().expect("the drain emitted a frame"));
    assert_eq!(landed.results.len(), 1, "the result list holds the row");

    send_library(&mut test, GuiLibraryCommand::SearchClose);

    let frame = test.latest_frame().expect("the close emitted a frame");
    assert!(
        library_of(frame).search.is_none(),
        "the closed Search rides as the section's null"
    );
}
