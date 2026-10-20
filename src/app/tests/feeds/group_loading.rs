use super::youtube_root_level;
use crate::app::tests::*;
use mbv_ui_model::browse::ServerRows;
use rstest::{fixture, rstest};

#[fixture]
fn make_home_video_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;

    app.libs.push(LibraryTab {
        nav_stack: vec![youtube_root_level(ServerRows::complete(0), vec![], true)],
        feed_home_video: Some(FeedHomeVideoState {
            loading: true,
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });

    app
}

fn seed_home_video_root_loaded(app: &mut App) -> EmbyItem {
    let mut empty = make_item("Empty Channel", "Folder");
    empty.id = "folder-empty".into();
    empty.is_folder = true;
    empty.path = "/videos/empty".into();

    let mut active = make_item("Active Channel", "Folder");
    active.id = "folder-active".into();
    active.is_folder = true;
    active.path = "/videos/active".into();

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Loaded {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        level: Box::new(youtube_root_level(
            ServerRows::loaded(2, 2),
            vec![empty, active.clone()],
            false,
        )),
    }));

    active
}

fn seed_folder_counts(app: &mut App, folder_id: &str, count: usize) {
    if let Some(state) = app.libs[0].feed_home_video.as_mut() {
        state.folder_counts.insert(folder_id.to_string(), count);
    }
}

fn seed_selection_cache(app: &mut App, selection: FeedHomeVideoSelection, items: Vec<EmbyItem>) {
    if let Some(state) = app.libs[0].feed_home_video.as_mut() {
        state.items.insert(selection, items);
    }
}

/// Contract (tasks.md 3.2): the FIRST selection of a pill triggers its lazy
/// fetch — the fetch-decision mark (`loading`) is the observable — and the
/// result event fills the session cache and clears the mark.
#[rstest]
fn first_folder_selection_triggers_its_fetch_and_the_result_fills_the_cache(
    make_home_video_app: App,
) {
    let mut app = make_home_video_app;
    // The fetch decision needs a Service client; the stub client's fetch
    // fails against the default config and the assertion reads only the
    // synchronous decision mark.
    install_test_emby(&mut app, crate::config::Config::default());
    let active = seed_home_video_root_loaded(&mut app);
    seed_folder_counts(&mut app, &active.id, 2);

    app.select_feed_folder_group(0, 1);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::Folder(active.id.clone())
    );
    assert!(state.loading, "first selection spawns its fetch");

    let mut video = make_item("Episode 1", "Movie");
    video.path = "/videos/active/ep1.mp4".into();
    app.handle_lib_event(LibEvent::Browse(
        BrowseEvent::FeedHomeVideoSelectionLoaded {
            lib_idx: 0,
            parent_id: active.id.clone(),
            selection: FeedHomeVideoSelection::Folder(active.id.clone()),
            items: vec![video],
        },
    ));

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(!state.loading, "the fetch returned");
    assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
    assert_eq!(
        app.feed_home_video_selected_items(0)[0].path,
        "/videos/active/ep1.mp4"
    );
}

/// Contract (tasks.md 3.2): re-selecting a loaded selection reuses the
/// session cache — the items survive and no fetch decision (`loading`) is
/// set for either pill.
#[rstest]
fn reselecting_loaded_selections_reuses_the_cache_without_a_fetch(make_home_video_app: App) {
    let mut app = make_home_video_app;
    let active = seed_home_video_root_loaded(&mut app);
    seed_folder_counts(&mut app, &active.id, 2);
    let mut video = make_item("Episode 1", "Movie");
    video.path = "/videos/active/ep1.mp4".into();
    seed_selection_cache(&mut app, FeedHomeVideoSelection::All, vec![video.clone()]);
    seed_selection_cache(
        &mut app,
        FeedHomeVideoSelection::Folder(active.id.clone()),
        vec![video],
    );

    app.select_feed_folder_group(0, 1);
    assert!(!app.libs[0].feed_home_video.as_ref().unwrap().loading);
    app.select_feed_folder_group(0, 0);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(!state.loading, "a cached selection sets no fetch");
    assert_eq!(
        state.items.len(),
        2,
        "the session cache survives pill ping-pong"
    );
    assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
}

/// Contract (tasks.md 3.2, spec "Refresh keeps the lazy shape"): the root
/// listing refresh drops the whole session cache and leaves the CURRENT
/// selection awaiting its refetch (the shown pass triggers it); the never-
/// loaded folder pill stays unloaded — only the current selection's result
/// refills the map.
#[rstest]
fn refresh_drops_caches_refetches_only_the_current_selection(make_home_video_app: App) {
    let mut app = make_home_video_app;
    let active = seed_home_video_root_loaded(&mut app);
    seed_folder_counts(&mut app, &active.id, 2);
    let video = make_item("Episode 1", "Movie");
    seed_selection_cache(&mut app, FeedHomeVideoSelection::All, vec![video]);

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Refreshed {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        item_types: None,
        unplayed_only: false,
        items: vec![active],
        total_count: 1,
    }));

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(state.items.is_empty(), "refresh drops the session cache");
    assert_eq!(state.selection, FeedHomeVideoSelection::All);
    let awaits_fetch = !state.loading && !state.items.contains_key(&state.selection);
    assert!(
        awaits_fetch,
        "the current selection refetches after refresh"
    );

    app.handle_lib_event(LibEvent::Browse(
        BrowseEvent::FeedHomeVideoSelectionLoaded {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            selection: FeedHomeVideoSelection::All,
            items: vec![make_item("Episode 2", "Movie")],
        },
    ));

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(state.items.len(), 1, "unloaded pills stay unloaded");
    assert!(state.items.contains_key(&FeedHomeVideoSelection::All));
}

/// Contract (tasks.md 3.2): a result whose selection no longer matches the
/// state's current one is stale-dropped — it stores nothing and leaves the
/// current fetch's `loading` mark alone.
#[rstest]
fn stale_selection_result_is_dropped(make_home_video_app: App) {
    let mut app = make_home_video_app;
    install_test_emby(&mut app, crate::config::Config::default());
    let active = seed_home_video_root_loaded(&mut app);
    seed_folder_counts(&mut app, &active.id, 2);
    app.select_feed_folder_group(0, 1);
    assert!(app.libs[0].feed_home_video.as_ref().unwrap().loading);

    app.handle_lib_event(LibEvent::Browse(
        BrowseEvent::FeedHomeVideoSelectionLoaded {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            selection: FeedHomeVideoSelection::All,
            items: vec![make_item("Stale", "Movie")],
        },
    ));

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(state.items.is_empty(), "the stale result stored nothing");
    assert!(
        state.loading,
        "the current selection's fetch stays outstanding"
    );
}

/// The group level stays a single root level and resolves the selection's
/// items once its cache holds them (kept from the pre-lazy shape).
#[rstest]
fn ensure_feed_home_video_group_level_does_not_push_and_resolves_selected_items(
    make_home_video_app: App,
) {
    let mut app = make_home_video_app;
    let active = seed_home_video_root_loaded(&mut app);

    let mut video = make_item("Episode 1", "Movie");
    video.path = "/videos/active/ep1.mp4".into();

    let state = app.libs[0].feed_home_video.as_mut().unwrap();
    state.loading = false;
    state.selection = FeedHomeVideoSelection::Folder(active.id.clone());
    state.items.insert(
        FeedHomeVideoSelection::Folder(active.id.clone()),
        vec![video],
    );

    app.ensure_feed_home_video_group_level(0);
    assert_eq!(app.libs[0].nav_stack.len(), 1);
    assert_eq!(app.feed_home_video_selected_items(0).len(), 1);
    assert_eq!(
        app.feed_home_video_selected_items(0)[0].path,
        "/videos/active/ep1.mp4"
    );
}

/// Sends one per-folder count event for the library root listing (task 3.3):
/// one fan-out thread's report for one folder.
fn send_folder_count(app: &mut App, parent_id: &str, folder_id: &str, count: usize) {
    app.handle_lib_event(LibEvent::Browse(
        BrowseEvent::FeedHomeVideoFolderCountLoaded {
            lib_idx: 0,
            parent_id: parent_id.to_string(),
            folder_id: folder_id.to_string(),
            count: Some(count),
        },
    ));
}

/// Contract (tasks.md 3.3, spec "Watched-only folder keeps its pill"): the
/// count fan-out's positive count alone admits the pill — the folder's
/// cached videos being played changes nothing.
#[rstest]
fn folder_counts_event_gives_a_watched_only_folder_its_pill(make_home_video_app: App) {
    let mut app = make_home_video_app;
    let active = seed_home_video_root_loaded(&mut app);
    let mut watched = make_item("Episode 1", "Movie");
    watched.played = true;
    seed_selection_cache(
        &mut app,
        FeedHomeVideoSelection::Folder(active.id.clone()),
        vec![watched],
    );

    send_folder_count(&mut app, "lib-youtube", "folder-active", 2);

    let counted = app.feed_home_video_counted_folders(0);
    assert_eq!(counted.len(), 1);
    assert_eq!(counted[0].id, active.id);
}

/// Contract (task fan-out reshape): each fan-out thread reports its own
/// per-folder event, so the pill row fills incrementally — one folder's
/// count alone admits that folder's pill, and a later event adds its own
/// without disturbing the first.
#[rstest]
fn separate_count_events_accumulate_into_the_pill_row(make_home_video_app: App) {
    let mut app = make_home_video_app;
    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Loaded {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        level: Box::new(youtube_root_level(
            ServerRows::loaded(3, 3),
            vec![
                make_root_folder("folder-a", "Channel A"),
                make_root_folder("folder-b", "Channel B"),
                make_root_folder("folder-c", "Channel C"),
            ],
            false,
        )),
    }));

    send_folder_count(&mut app, "lib-youtube", "folder-a", 1);

    let counted = app.feed_home_video_counted_folders(0);
    assert_eq!(
        counted
            .iter()
            .map(|folder| folder.id.as_str())
            .collect::<Vec<_>>(),
        vec!["folder-a"],
        "the first folder's event alone fills its pill"
    );

    send_folder_count(&mut app, "lib-youtube", "folder-c", 1);

    let counted = app.feed_home_video_counted_folders(0);
    assert_eq!(
        counted
            .iter()
            .map(|folder| folder.id.as_str())
            .collect::<Vec<_>>(),
        vec!["folder-a", "folder-c"],
        "the second event adds its folder without dropping the first"
    );
}

/// Contract (tasks.md 3.3, spec "Empty folder gets no pill"): a folder whose
/// fan-out count is zero gets no pill; a counted sibling keeps its own.
#[rstest]
fn folder_counts_event_denies_a_pill_to_an_empty_folder(make_home_video_app: App) {
    let mut app = make_home_video_app;
    let active = seed_home_video_root_loaded(&mut app);

    send_folder_count(&mut app, "lib-youtube", "folder-empty", 0);
    send_folder_count(&mut app, "lib-youtube", "folder-active", 2);

    let counted = app.feed_home_video_counted_folders(0);
    assert_eq!(
        counted
            .iter()
            .map(|folder| folder.id.as_str())
            .collect::<Vec<_>>(),
        vec![active.id.as_str()]
    );
}

/// Contract (tasks.md 3.3, spec "Pills appear without a full-library video
/// fetch"): the listing completion and the count arrival derive the pill row
/// while the session's video lists stay empty — nothing in either event
/// fetched any videos.
#[rstest]
fn listing_and_counts_derive_the_pill_row_without_any_video_fetch(make_home_video_app: App) {
    let mut app = make_home_video_app;
    seed_home_video_root_loaded(&mut app);

    send_folder_count(&mut app, "lib-youtube", "folder-active", 2);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        app.feed_home_video_counted_folders(0).len(),
        1,
        "the pill row derives from the listing and the counts"
    );
    assert!(state.items.is_empty(), "no video listing was fetched");
    assert!(!state.loading, "no video fetch is outstanding");
}

/// Contract (tasks.md 3.3, B's hand-off note): once the count fan-out has
/// quiesced — every folder's report landed, success or failure — the count
/// arm resolves a pending restored pill index into the concrete folder
/// selection and consumes the index.
#[rstest]
fn folder_counts_event_resolves_the_pending_pill_index(make_home_video_app: App) {
    let mut app = make_home_video_app;
    let active = seed_home_video_root_loaded(&mut app);
    if let Some(state) = app.libs[0].feed_home_video.as_mut() {
        state.restored_group = Some(1);
        // The spawn's quiescence accounting: one owed report.
        state.outstanding_counts = 1;
    }

    send_folder_count(&mut app, "lib-youtube", "folder-active", 2);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::Folder(active.id.clone())
    );
}

/// Regression (review P1 on 96abfa239..ab4b558bb): the parallel fan-out's
/// per-folder events arrive in any order, and a restored pill index must not
/// bind against a partially arrived count row — folder B's early report must
/// not make restored index 1 (the listing-first folder's pill) select B. The
/// index resolves only when the fan-out has quiesced, and is consumed then
/// even if it names no pill.
#[rstest]
fn out_of_order_count_arrivals_do_not_misbind_a_restored_pill_index(make_home_video_app: App) {
    let mut app = make_home_video_app;
    // Listing order: A first, B second — the restored index 1 names A's pill.
    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Loaded {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        level: Box::new(youtube_root_level(
            ServerRows::loaded(2, 2),
            vec![
                make_root_folder("folder-a", "Channel A"),
                make_root_folder("folder-b", "Channel B"),
            ],
            false,
        )),
    }));
    let state = app.libs[0].feed_home_video.as_mut().unwrap();
    state.restored_group = Some(1);
    // The spawn's quiescence accounting: two owed reports.
    state.outstanding_counts = 2;

    // B's count lands first: the row is not final yet.
    send_folder_count(&mut app, "lib-youtube", "folder-b", 1);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::All,
        "a partial count row must not bind the restored index"
    );
    assert_eq!(
        state.restored_group,
        Some(1),
        "the index stays pending until the fan-out quiesces"
    );

    // A's report settles the fan-out; the final row is [A, B].
    send_folder_count(&mut app, "lib-youtube", "folder-a", 1);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::Folder("folder-a".into())
    );
    assert_eq!(
        state.restored_group, None,
        "the index is consumed once it resolves"
    );
}

/// Contract (tasks.md 3.3): a count result whose root parent no longer
/// matches the library's current root is stale-dropped and fills nothing.
#[rstest]
fn folder_counts_event_for_a_foreign_root_is_dropped(make_home_video_app: App) {
    let mut app = make_home_video_app;
    seed_home_video_root_loaded(&mut app);

    send_folder_count(&mut app, "lib-other", "folder-active", 2);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(state.folder_counts.is_empty());
}

/// Regression (review P1 on b603f329b..9e454b62d): selecting an uncached
/// pill while another selection's fetch is in flight must hand `loading`
/// over to the new selection — the previous fetch's result is stale-dropped,
/// so a leftover mark would wedge the view with a spinner and no fetch for
/// the shown selection.
#[rstest]
fn selecting_an_uncached_pill_while_a_fetch_is_in_flight_hands_loading_over(
    make_home_video_app: App,
) {
    let mut app = make_home_video_app;
    install_test_emby(&mut app, crate::config::Config::default());
    let active = seed_home_video_root_loaded(&mut app);
    seed_folder_counts(&mut app, &active.id, 2);

    // The All selection's fetch is in flight (`loading` set by its spawn).
    app.ensure_feed_home_video_selection_loaded(0, false);
    assert!(app.libs[0].feed_home_video.as_ref().unwrap().loading);

    // The Service drops before the folder pill is selected, so the new
    // selection's spawn must retry through the shown pass — which the
    // hand-off below keeps possible.
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::default();

    app.select_feed_folder_group(0, 1);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::Folder(active.id.clone())
    );
    assert!(
        !state.loading,
        "the stale mark belongs to the old selection"
    );
    assert!(
        !state.loading && !state.items.contains_key(&state.selection),
        "the shown pass can spawn the new selection's fetch"
    );

    // The old selection's late result is stale-dropped and leaves the
    // hand-off intact.
    app.handle_lib_event(LibEvent::Browse(
        BrowseEvent::FeedHomeVideoSelectionLoaded {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            selection: FeedHomeVideoSelection::All,
            items: vec![make_item("Stale", "Movie")],
        },
    ));

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(state.items.is_empty(), "the stale result stored nothing");
    assert!(
        !state.loading && !state.items.contains_key(&state.selection),
        "the shown selection still awaits its fetch"
    );
}

fn make_root_folder(id: &str, name: &str) -> EmbyItem {
    let mut folder = make_item(name, "Folder");
    folder.id = id.into();
    folder.is_folder = true;
    folder
}

/// Regression (review P2, b603f329b..9e454b62d): a refresh that lands only
/// the first page of a multi-page root listing must not replace
/// `folder_counts` from its page-1 folders, nor clamp or re-resolve the pill
/// index against the partial listing while `selection` keeps a later-page
/// folder. The final page append completes the refresh: the pill row
/// re-derives from the full listing and the selection stays the folder the
/// user picked.
#[rstest]
fn partial_root_refresh_keeps_counts_and_the_later_page_pill(make_home_video_app: App) {
    let mut app = make_home_video_app;
    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Loaded {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        level: Box::new(youtube_root_level(
            ServerRows::loaded(3, 3),
            vec![
                make_root_folder("folder-a", "Channel A"),
                make_root_folder("folder-b", "Channel B"),
                make_root_folder("folder-c", "Channel C"),
            ],
            false,
        )),
    }));
    for id in ["folder-a", "folder-b", "folder-c"] {
        seed_folder_counts(&mut app, id, 1);
    }
    if let Some(state) = app.libs[0].feed_home_video.as_mut() {
        state.selection = FeedHomeVideoSelection::Folder("folder-c".into());
    }

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Refreshed {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        item_types: None,
        unplayed_only: false,
        items: vec![make_root_folder("folder-a", "Channel A")],
        total_count: 3,
    }));

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.folder_counts.len(),
        3,
        "the partial refresh did not replace the counts"
    );
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::Folder("folder-c".into()),
        "the selection outlives the partial listing"
    );

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::PageAppended {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        items: vec![
            make_root_folder("folder-b", "Channel B"),
            make_root_folder("folder-c", "Channel C"),
        ],
        total_count: 3,
    }));

    assert_eq!(
        app.feed_home_video_counted_folders(0)
            .iter()
            .map(|folder| folder.id.as_str())
            .collect::<Vec<_>>(),
        vec!["folder-a", "folder-b", "folder-c"],
        "the repaginated listing restores the pill row"
    );
    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(state.folder_counts.len(), 3);
    assert_eq!(
        app.feed_home_video_selected_group_index(0),
        3,
        "the repaginated row highlights the selected folder again"
    );
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::Folder("folder-c".into())
    );
}

/// Contract (design D2 refresh; replaces the deleted `pending_count_refresh`
/// marker, review P2 9e454b62d..8e5ce710a): a fully paginated refresh
/// invalidates the per-folder counts, so the next listing completion's
/// non-forced fan-out fails its dedupe and re-queries every folder — a
/// refresh must not leave the pre-refresh counts serving the pill row.
#[rstest]
fn a_full_refresh_invalidates_the_folder_counts_so_the_fan_out_requeries(make_home_video_app: App) {
    let mut app = make_home_video_app;
    let active = seed_home_video_root_loaded(&mut app);
    seed_folder_counts(&mut app, &active.id, 2);

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Refreshed {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        item_types: None,
        unplayed_only: false,
        items: vec![active],
        total_count: 1,
    }));

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(
        state.folder_counts.is_empty(),
        "the refresh cleared the counts so the fan-out re-queries"
    );
}

/// Regression (review P2 on 96abfa239..ab4b558bb): `spawn_refresh` fetches
/// only `loaded_count.max(PAGE_SIZE)` rows, so a refresh of a root listing
/// that has not fully paginated lands partial — and that refresh must still
/// re-query the counts (design D2), or a folder crossing 0<->positive
/// videos never gains or loses its pill for the rest of the session. The
/// partial refresh commits the fan-out synchronously (the quiescence
/// accounting mark) while leaving the existing counts in place: page-1
/// folders must not shrink the row (review P2, b603f329b..9e454b62d).
#[rstest]
fn a_partial_refresh_commits_a_count_fanout_without_shrinking_the_counts(make_home_video_app: App) {
    let mut app = make_home_video_app;
    // The stub client's count queries fail into the unprocessed event
    // channel, so the synchronous accounting mark stays observable.
    install_test_emby(&mut app, crate::config::Config::default());
    app.libs[0].nav_stack[0] = youtube_root_level(
        ServerRows::loaded(3, 3),
        vec![
            make_root_folder("folder-a", "Channel A"),
            make_root_folder("folder-b", "Channel B"),
            make_root_folder("folder-c", "Channel C"),
        ],
        false,
    );
    seed_folder_counts(&mut app, "folder-a", 1);
    seed_folder_counts(&mut app, "folder-b", 1);
    seed_folder_counts(&mut app, "folder-c", 1);

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Refreshed {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        item_types: None,
        unplayed_only: false,
        items: vec![make_root_folder("folder-a", "Channel A")],
        total_count: 3,
    }));

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.folder_counts.len(),
        3,
        "page-1 folders did not shrink the count map"
    );
    assert_eq!(
        state.outstanding_counts, 1,
        "the partial refresh committed a fan-out for the listing's folders"
    );
}

/// Regression (round-2 review on 96abfa239..30a12b0e8): a partial refresh
/// whose folder set is UNCHANGED leaves the beyond-page-1 counts stale unless
/// the listing completion that ends the pagination re-queries them — the map
/// already covers every folder, so a coverage-only spawn condition
/// short-circuits and a folder beyond page 1 that crossed 0<->positive
/// videos keeps its stale pill for the session (design D2). The
/// listing-completion transition therefore spawns the full fan-out even when
/// the counts already cover the root; the stale entries keep serving the row
/// until the re-query's reports land.
#[rstest]
fn the_listing_completion_after_a_partial_refresh_requeries_every_folder(make_home_video_app: App) {
    let mut app = make_home_video_app;
    // The stub client's count queries fail into the unprocessed event
    // channel, so the synchronous accounting mark stays observable.
    install_test_emby(&mut app, crate::config::Config::default());
    app.libs[0].nav_stack[0] = youtube_root_level(
        ServerRows::loaded(3, 3),
        vec![
            make_root_folder("folder-a", "Channel A"),
            make_root_folder("folder-b", "Channel B"),
            make_root_folder("folder-c", "Channel C"),
        ],
        false,
    );
    seed_folder_counts(&mut app, "folder-a", 1);
    seed_folder_counts(&mut app, "folder-b", 1);
    seed_folder_counts(&mut app, "folder-c", 1);

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Refreshed {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        item_types: None,
        unplayed_only: false,
        items: vec![make_root_folder("folder-a", "Channel A")],
        total_count: 3,
    }));

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::PageAppended {
        lib_idx: 0,
        parent_id: "lib-youtube".into(),
        items: vec![
            make_root_folder("folder-b", "Channel B"),
            make_root_folder("folder-c", "Channel C"),
        ],
        total_count: 3,
    }));

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.outstanding_counts, 4,
        "the completion re-queried every folder despite full coverage \
         (1 refresh fan-out + 3 completion fan-out)"
    );
    assert_eq!(
        state.folder_counts.len(),
        3,
        "the stale entries still serve the row until the re-query lands"
    );
}

/// Contract (review P2 on task 5.1, commit 46b0ac739): the UI-state reset
/// restores a feed-view library's root to a plain folder listing — no
/// `item_types`, no unplayed-only flag — so a later refresh builds its fetch
/// from folder-listing fields instead of fetching videos onto the root and
/// destroying the pill source.
#[rstest]
fn ui_state_reset_restores_the_feed_view_root_to_a_folder_listing(make_home_video_app: App) {
    let mut app = make_home_video_app;
    seed_home_video_root_loaded(&mut app);
    if let Some(root) = app.libs[0].nav_stack.first_mut() {
        root.item_types = Some("Video".into());
        root.unplayed_only = true;
        root.sort_by = "DateCreated".into();
        root.sort_order = "Ascending".into();
    }

    app.reset_browse_root_defaults();

    let root = &app.libs[0].nav_stack[0];
    assert_eq!(root.item_types, None, "the reset drops the Video filter");
    assert!(
        !root.unplayed_only,
        "the reset drops the unplayed-only flag"
    );
    assert_eq!(root.sort_by, "SortName");
    assert_eq!(root.sort_order, "Ascending");
    assert_eq!(root.items.len(), 2, "the root keeps its folder items");
}
