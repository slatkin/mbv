use crate::app::tests::*;
use mbv_ui_model::browse::BrowseResting;
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
        nav_stack: vec![BrowseLevel {
            rows: ServerRows::complete(0),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![],

            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
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
        level: Box::new(BrowseLevel {
            rows: ServerRows::loaded(2, 2),
            parent_id: "lib-youtube".into(),
            title: "YouTube".into(),
            items: vec![empty, active.clone()],

            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }),
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
    assert!(
        app.feed_home_video_selection_awaits_fetch(0),
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

/// Sends the count fan-out's result event for the library root listing
/// (task 3.3): the successfully obtained `(folder id, count)` pairs.
fn send_folder_counts(app: &mut App, parent_id: &str, counts: &[(&str, usize)]) {
    app.handle_lib_event(LibEvent::Browse(
        BrowseEvent::FeedHomeVideoFolderCountsLoaded {
            lib_idx: 0,
            parent_id: parent_id.to_string(),
            counts: counts
                .iter()
                .map(|(folder, count)| ((*folder).to_string(), *count))
                .collect(),
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

    send_folder_counts(&mut app, "lib-youtube", &[("folder-active", 2)]);

    let counted = app.feed_home_video_counted_folders(0);
    assert_eq!(counted.len(), 1);
    assert_eq!(counted[0].id, active.id);
}

/// Contract (tasks.md 3.3, spec "Empty folder gets no pill"): a folder whose
/// fan-out count is zero gets no pill; a counted sibling keeps its own.
#[rstest]
fn folder_counts_event_denies_a_pill_to_an_empty_folder(make_home_video_app: App) {
    let mut app = make_home_video_app;
    let active = seed_home_video_root_loaded(&mut app);

    send_folder_counts(
        &mut app,
        "lib-youtube",
        &[("folder-empty", 0), ("folder-active", 2)],
    );

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

    send_folder_counts(&mut app, "lib-youtube", &[("folder-active", 2)]);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        app.feed_home_video_counted_folders(0).len(),
        1,
        "the pill row derives from the listing and the counts"
    );
    assert!(state.items.is_empty(), "no video listing was fetched");
    assert!(!state.loading, "no video fetch is outstanding");
}

/// Contract (tasks.md 3.3, B's hand-off note): the count-completion arm
/// resolves a pending persisted pill index into the concrete folder
/// selection — the index could not resolve while the counts were empty.
#[rstest]
fn folder_counts_event_resolves_the_pending_pill_index(make_home_video_app: App) {
    let mut app = make_home_video_app;
    let active = seed_home_video_root_loaded(&mut app);
    if let Some(state) = app.libs[0].feed_home_video.as_mut() {
        state.selected_group = 1;
    }

    send_folder_counts(&mut app, "lib-youtube", &[("folder-active", 2)]);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::Folder(active.id.clone())
    );
}

/// Contract (tasks.md 3.3): a count result whose root parent no longer
/// matches the library's current root is stale-dropped and fills nothing.
#[rstest]
fn folder_counts_event_for_a_foreign_root_is_dropped(make_home_video_app: App) {
    let mut app = make_home_video_app;
    seed_home_video_root_loaded(&mut app);

    send_folder_counts(&mut app, "lib-other", &[("folder-active", 2)]);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(state.folder_counts.is_empty());
}
