//! The Feeds destination's contract (add-gui-library-panel design D12, row
//! 11.1): one background fetch's completion lands on the App through the GUI
//! tick's own drain, so the destination's list reads the same App data the
//! TUI shell reads. The fetch boundary is the App's own result channel, so no
//! request leaves the process and no worker outlives the assertion.

use mbv::ClientTickHost;

#[test]
fn a_feed_fetch_result_lands_on_the_app_through_the_gui_tick() {
    let mut test = crate::test_core(0);
    test.core.app_mut().seed_gui_feeds_for_test(&[
        ("Feed A", "https://example.test/a"),
        ("Feed B", "https://example.test/b"),
    ]);
    test.core
        .app_mut()
        .send_gui_feed_fetch_result_for_test(0, &[("a-old", Some(100)), ("a-new", Some(300))]);
    test.core
        .app_mut()
        .send_gui_feed_fetch_result_for_test(1, &[("b-mid", Some(200)), ("b-undated", None)]);

    assert!(test.core.step(), "the core thread never quits mid-drain");

    let app = test.core.app_mut();
    let guids: Vec<String> = app
        .gui_feeds_entries()
        .iter()
        .map(|entry| entry.guid.clone())
        .collect();
    assert_eq!(
        guids,
        ["a-new", "b-mid", "a-old", "b-undated"],
        "the tick's drain landed both fetches, merged newest first with the undated entry last"
    );
}
