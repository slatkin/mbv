// Unit tests for change `per-destination-item-navigation` tasks 1.1/1.2/4.2:
// the D1 reveal-item table, the `LibEvent::NavigateTo` landing payload, and
// the drained resolve-failure error event. Mocked Emby boundary only
// (`MockHttp`), per the AGENTS.md mocks-only policy.

use super::*;
use crate::app::state::types::browse::BrowseResting;
use crate::app::tests::{make_app_stub, make_item};
use mbv_core::mock_http::MockHttp;

/// App stub with a scripted in-memory Emby transport installed.
fn app_with_mock_emby(http: &MockHttp) -> App {
    let mut app = make_app_stub();
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    app
}

/// A TV library tab with an already-loaded root series level.
fn app_with_loaded_tv_library() -> App {
    let mut app = make_app_stub();
    let mut library = make_item("TV", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    app.libs.push(LibraryTab::new(library));
    app.libs[0].library_total = Some(2);
    let mut other = make_item("Other Show", "Series");
    other.id = "ser0".into();
    let mut show = make_item("The Show", "Series");
    show.id = "ser1".into();
    app.libs[0].nav_stack.push(BrowseLevel {
        fetched_rows: 2,
        parent_id: "lib-tv".into(),
        title: "TV".into(),
        items: vec![other, show],
        total_count: 2,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Series".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    });
    app
}

mod library_navigate_reveal_album;
mod library_navigate_reveal_series;
mod library_navigate_reveal_target;
mod library_navigate_reveal_worker;
