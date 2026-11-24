//! The plain Emby library's root-list refetch (gui-list-header-buttons design
//! D8): a failed refetch keeps the list the store already held. The Emby
//! boundary is the in-memory mock transport, so no request leaves the process
//! and no worker outlives the assertion.

use super::*;

/// A failed refetch of a plain Emby library's root list keeps its items
/// (gui-list-header-buttons design D8): the store keeps the loaded list it
/// held, so the painted rows do not empty on a refresh that fails.
#[test]
fn a_failed_root_list_refetch_keeps_its_items() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = emby_app(&http, "movies", MOVIES_ID, "Movies");
    app.seed_gui_emby_roots_for_test(0, vec![make_item("Old Movie", "Movie")]);
    http.fail(std::io::ErrorKind::ConnectionRefused);

    stub_gui_state(|state| {
        crate::gui_core::gui_library::emby::refetch_emby_root_items(&mut app, state, 0);
    });
    wait_for_client_event(&mut app);

    assert_eq!(
        app.gui_emby_root_ids_for_test(0),
        Some(vec!["id".to_string()]),
        "the failed refetch keeps the root list's items"
    );
}
