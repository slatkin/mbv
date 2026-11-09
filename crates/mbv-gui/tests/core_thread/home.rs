//! Home's stored content (add-gui-library-panel row 6.1): the mocked
//! asynchronous Emby bootstrap completes on the core thread's own tick, and
//! the frame that tick emits reads the Continue Watching items the App
//! stores. The completion rides the queued startup channel, so no live
//! Owner, Service, or webview runs; the state directory is a
//! `TestStateDirGuard` scratch path (design D13).

use mbv::{ClientTickHost, stub_app_with_completed_emby_startup_and_contents};
use mbv_emby_model::test_support::make_item;

use crate::test_core_with;

/// An asynchronous Emby bootstrap completes on the startup drain, and the
/// next frame's tick reads its Continue Watching items from the App's stored
/// Home content (row 6.1): the later Home projection of row 6.2 reads the
/// stored snapshot, never a per-Client copy.
#[test]
fn an_emby_startup_completion_stores_the_continue_watching_for_the_next_frame() {
    let mut first = make_item("Continue First", "Movie");
    first.id = "cw-first".to_string();
    let mut second = make_item("Continue Second", "Movie");
    second.id = "cw-second".to_string();
    let contents = vec![first, second];
    let mut test = test_core_with(stub_app_with_completed_emby_startup_and_contents(
        Vec::new(),
        contents,
    ));

    // The zeroed-cadence step drains the queued completion and emits the
    // frame that follows it.
    assert!(test.core.step());
    let _frame = test.next_frame();

    let home = test.core.app_mut().home_content_for_test();
    let names: Vec<&str> = home
        .continue_items
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(names, vec!["Continue First", "Continue Second"]);
    assert!(
        !home.loading,
        "the stored snapshot's loading flag settles with the bootstrap"
    );
}
