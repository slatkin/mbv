//! The GUI Library's Shows data path through the GUI tick (add-gui-library-
//! panel design D12, row 7.1, contract a): the App actions the later core
//! surface's commands will apply seed the selection and start the mode's
//! root load, and the tick's library drain applies the fetch completion
//! through the App arms — the same arms the presenting Client's shell drain
//! joins. The Emby answers come from the in-memory mock transport, so no
//! request leaves the process, and no live Owner, Service, or webview runs
//! (design D13).

use mbv::ClientTickHost;
use mbv_net::mock_http::MockHttp;

use super::TestCore;
use crate::{test_core_with, test_emby_client};

/// A Shows library's series listing, as the mode load's ranged fetch
/// returns it.
const SERIES: &str = r#"{"Items":[{"Id":"series-a","Name":"Alfred","Type":"Series"},{"Id":"series-b","Name":"Bridgerton","Type":"Series"}],"TotalRecordCount":2}"#;

/// A core thread over an App whose Emby runtime holds a client over `http`,
/// with a Shows library seeded as the active destination: an empty root
/// level with no resolved TV mode, as a first open finds it. The seeding
/// lands after the core exists, so the construction's launch Home load saw
/// no Service and scripts no mock answer (design D13).
fn shows_core(http: &MockHttp) -> TestCore {
    let (app, _cmd_rx) = mbv::stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    test.core
        .app_mut()
        .seed_gui_tv_library_for_test(test_emby_client(http));
    test
}

/// Pump GUI ticks until the root level holds rows (the fetch worker's
/// completion is asynchronous over the in-memory mock), bounded so a broken
/// fetch fails the assertion instead of hanging the test.
fn pump_until_root_rows(test: &mut TestCore) {
    for _ in 0..250 {
        if test
            .core
            .app_mut()
            .tv_root_item_ids_for_test(0)
            .as_ref()
            .is_some_and(|ids| !ids.is_empty())
        {
            return;
        }
        assert!(test.core.step(), "the core thread never quits mid-drain");
    }
    panic!("the All series mode's root load never landed on the App");
}

/// Opening the Shows library in the All series mode lands the series root
/// rows on the App through the GUI tick (row 7.1, contract a): the open and
/// set-mode actions run the TUI's own mode load, and the tick's
/// `drain_lib_event` applies the refresh completion's App arm. The mock's
/// request wait is the sanctioned seam for the fetch worker's own progress,
/// so the step pump never starves it.
#[test]
fn opening_shows_in_all_mode_lands_the_series_root_rows_on_the_tick() {
    let http = MockHttp::new();
    http.respond(200, SERIES);
    let mut test = shows_core(&http);

    let app = test.core.app_mut();
    app.gui_tv_open(0);
    app.gui_tv_set_mode(0, &mbv_queue::TvContentMode::All);
    assert!(
        http.wait_for_requests(1, std::time::Duration::from_secs(2)),
        "the mode load's fetch was never sent"
    );
    pump_until_root_rows(&mut test);

    assert_eq!(
        test.core.app_mut().tv_root_item_ids_for_test(0),
        Some(vec!["series-a".to_string(), "series-b".to_string()]),
        "the series root rows land on the App the GUI tick drains"
    );
}
