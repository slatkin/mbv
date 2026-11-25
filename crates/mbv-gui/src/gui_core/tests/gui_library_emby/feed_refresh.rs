//! A feed-group library's refresh (gui-list-header-buttons design D7a): the
//! press refetches the targets the destination paints and never clears the
//! shared item map, so the rows stay until the new ones land — and a failed
//! refetch keeps the rows the destination already paints. The Emby boundary is
//! the in-memory mock transport, so no request leaves the process and no
//! worker outlives the assertion.

use super::*;

/// The All list a feed-group library paints, seeded as a landed fetch would:
/// the videos the destination shows before the refresh.
fn seed_all_items(app: &mut App, ids: &[&str]) {
    let items = ids
        .iter()
        .map(|id| {
            let mut item = make_item(id, "Video");
            item.id = (*id).to_string();
            item
        })
        .collect();
    app.edit_library_for_test(0, |library| {
        let state = library
            .feed_home_video
            .get_or_insert_with(FeedHomeVideoState::default);
        state.items.insert(FeedHomeVideoSelection::All, items);
    });
}

/// The ids the All list holds: the rows the destination paints.
fn painted_all_ids(app: &App) -> Vec<String> {
    app.libs_for_test()[0]
        .feed_home_video
        .as_ref()
        .and_then(|state| {
            state
                .items
                .get(&FeedHomeVideoSelection::All)
                .map(|items| items.iter().map(|item| item.id.clone()).collect())
        })
        .unwrap_or_default()
}

/// A feed-group library whose root listing holds `folders`, with the checked
/// set still on All.
fn feed_group_app(http: &MockHttp, folders: &[(&str, &str)]) -> App {
    let mut app = emby_app(http, "homevideos", FEED_ID, "YouTube");
    app.config_for_test(|config| config.feed_view_libraries.push("youtube".into()));
    app.edit_library_for_test(0, |library| {
        library.nav_stack.push(level(
            FEED_ID,
            "YouTube",
            folders
                .iter()
                .map(|(id, name)| folder_item(id, name))
                .collect(),
        ));
    });
    app.edit_library_for_test(0, |library| {
        library.feed_home_video = Some(FeedHomeVideoState::default());
    });
    app
}

/// The target whose fetch is outstanding, `None` while none is.
fn fetching_target(app: &App) -> Option<FeedHomeVideoSelection> {
    app.libs_for_test()[0]
        .feed_home_video
        .as_ref()
        .and_then(|state| state.fetching.clone())
}

/// A refresh of a feed-group library whose All list is cached refetches All
/// and keeps the painted rows (design D7a): the press queues All and starts its
/// fetch even though the cache holds that list, which the uncached walk skips,
/// and the destination keeps painting the rows it holds until the new ones
/// land.
#[test]
fn a_refresh_of_a_cached_all_target_starts_its_fetch_and_keeps_the_painted_items() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.respond_when("ParentId=lib-youtube", 200, VIDEO_BODY);
    let mut app = feed_group_app(&http, &[("folder-1", "Channel 1")]);
    seed_all_items(&mut app, &["old-1", "old-2"]);

    crate::gui_core::test_support::with_stub_gui(&mut app, |core| core.refresh_gui_feed_group(0));

    assert_eq!(
        fetching_target(&app),
        Some(FeedHomeVideoSelection::All),
        "the press refetches All even while its list is cached"
    );
    assert_eq!(
        painted_all_ids(&app),
        ["old-1".to_string(), "old-2".to_string()],
        "the painted rows stay until the new ones land"
    );

    land_until_fetched(&mut app);
    assert_eq!(
        http.request_count(),
        1,
        "the queued target's fetch was the one request the refresh issued"
    );
}

/// A failed target refetch keeps the painted rows (design D7a): the failure
/// retires the fetch mark and records the terminal failure beside the claim,
/// and the old items stay in the cache, so the destination paints them on.
#[test]
fn a_failed_feed_target_refetch_keeps_the_painted_items() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    http.fail(std::io::ErrorKind::ConnectionRefused);
    let mut app = feed_group_app(&http, &[("folder-1", "Channel 1")]);
    seed_all_items(&mut app, &["old-1"]);

    crate::gui_core::test_support::with_stub_gui(&mut app, |core| core.refresh_gui_feed_group(0));
    land_until_fetched(&mut app);

    assert_eq!(
        http.request_count(),
        1,
        "the press issued the target's refetch, which failed"
    );
    assert_eq!(
        painted_all_ids(&app),
        ["old-1".to_string()],
        "the failed refetch keeps the rows the destination paints"
    );
}
