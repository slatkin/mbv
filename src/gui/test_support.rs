//! The GUI Client's test support (`2-split-tui-gui-clients` design D4c): the
//! fixtures and helper calls the GUI's own tests and a dependent Client's
//! `test` build drive. They stand beside `GuiCore` rather than in the core, so
//! the core names no GUI module and the GUI names the core only through the
//! `test`-feature seams `crate::app` re-exports (design D4b rule 5).
//!
//! The core state a call seeds is written through those seams; the Client's own
//! state is held in the sidecar this module keeps (`stub_gui_state`,
//! `gui_state_for_test`), because the `App` never holds it.

use crate::app::App;
use crate::app::handoff_app;
#[cfg(test)]
use crate::app::state::gui_client_events::GuiClientEvent;
use crate::gui::GuiCore;
use crate::gui::gui_client_state::GuiClientState;
#[cfg(test)]
use crate::gui::gui_library::client_events::install_client_event;
use crate::gui::gui_library::{GuiCollectionSelection, GuiCollectionSet};
use crate::gui::gui_library::{GuiEmbyRootItems, GuiGroupAlbums};

/// The GUI Client's own state beside the stub `App` a test holds. The state
/// lives in `GuiCore` (design D4), and these helpers drive the `App` alone, so
/// the state a test seeds is held here beside it and never inside the `App`.
/// Every test runs on its own thread with a stub `App` of its own, so one
/// thread's state reaches no other test.
///
/// The slot starts empty: the core call a helper makes ([`core_holding_stub_gui`],
/// [`with_stub_gui`]) takes the seeded state out and hands the state it leaves
/// back, so the slot holds a state only while a test seeds one or after a core
/// call published one. A read of an empty slot sees a fresh state.
#[cfg(test)]
pub(crate) fn stub_gui_state<R>(run: impl FnOnce(&mut GuiClientState) -> R) -> R {
    STUB_GUI_STATE.with(|slot| {
        let default = GuiClientState::new(false);
        run(slot.borrow_mut().get_or_insert_with(|| default))
    })
}

#[cfg(test)]
thread_local! {
    static STUB_GUI_STATE: std::cell::RefCell<Option<GuiClientState>> =
        const { std::cell::RefCell::new(None) };
}

// The sidecar state thread: every `App`-seam read and seed of a dependent
// Client's `test` build answers from it, and so does the client-event drain
// that `CoreThread::step` runs. `None` until the first read or seed takes
// the default state, so the empty `GuiClientState::new(false)` is built once
// and only when a test needs it.
#[cfg(any(test, feature = "test"))]
thread_local! {
    static CLIENT_GUI_STATE: std::cell::RefCell<Option<GuiClientState>> =
        const { std::cell::RefCell::new(None) };
}

/// Read or seed the GUI Client's own state beside the `App` a dependent
/// Client's test drives (`decouple-client-core` row 8.1). The state belongs to
/// `GuiCore`, and these `App` seams answer from this one copy, so a seed, the
/// frame it asserts on, and the `CoreThread::step` that adopts it all read and
/// write the same state: the step takes the slot's state at its start and
/// publishes its own back at its end, so the slot always holds the live state
/// between steps and the first call's default is what an unseeded test reads.
#[cfg(any(test, feature = "test"))]
pub fn gui_state_for_test<R>(read: impl FnOnce(&mut GuiClientState) -> R) -> R {
    seeded_gui_state_for_test(|slot| read(slot.get_or_insert_with(|| GuiClientState::new(false))))
}

/// Reach the sidecar slot itself (`decouple-client-core` row 8.1): the core
/// takes the seeded state out at a step's start and puts its own back at the
/// step's end, so it needs the slot and not the state a read gets. Empty
/// until a test seeds one or a core call publishes one.
#[cfg(any(test, feature = "test"))]
pub fn seeded_gui_state_for_test<R>(read: impl FnOnce(&mut Option<GuiClientState>) -> R) -> R {
    CLIENT_GUI_STATE.with(|slot| read(&mut slot.borrow_mut()))
}

/// Read or seed the GUI Client's own state beside `app`, over whichever
/// sidecar this build keeps: the crate's own tests use [`stub_gui_state`], and
/// a dependent Client's `test` build uses [`gui_state_for_test`].
fn gui<R>(_app: &App, read: impl FnOnce(&mut GuiClientState) -> R) -> R {
    #[cfg(test)]
    return stub_gui_state(read);
    #[cfg(all(not(test), feature = "test"))]
    {
        gui_state_for_test(read)
    }
}

/// Run one GUI core call over a stub `App`, with the state a test seeded
/// beside it: the call's state installs for its length, and the state it
/// leaves installs back beside the `App`.
#[cfg(test)]
pub(crate) fn with_stub_gui<T>(app: &mut App, run: impl FnOnce(&mut GuiCore) -> T) -> T {
    let mut core = GuiCore::new(std::mem::replace(app, handoff_app()));
    *core.gui_state_mut() = stub_gui_state(std::mem::take);
    let value = run(&mut core);
    core.into_app_keeping_gui(app);
    value
}

/// A `GuiCore` over a stub `App` a test holds, with the state a test seeded
/// beside it: the test takes the `App` back with `into_app_keeping_gui`.
#[cfg(test)]
pub(crate) fn core_holding_stub_gui(app: &mut App) -> GuiCore {
    let mut core = GuiCore::new(std::mem::replace(app, handoff_app()));
    *core.gui_state_mut() = stub_gui_state(std::mem::take);
    core
}

/// Wait for one Client-only fetch result and install it through the GUI's own
/// client-event drain (task 8.3 of `decouple-client-core`): a worker's send
/// lands on the `App`'s channel. Production wakes the tick with the
/// `ClientWaker`; a test has no tick to wake, so it waits on the channel and
/// fails when the worker never reports.
#[cfg(test)]
pub(crate) fn wait_for_client_event(app: &mut App) {
    let event = app
        .wait_for_one_gui_client_event_for_test(std::time::Duration::from_secs(2))
        .expect("the Client-only fetch reported its result");
    stub_gui_state(|gui| install_client_event(gui, event));
}

/// Apply one GUI Library command to a stub `App` a test holds: the test
/// helper for the command tests, which run the command through the GUI core
/// over the `App`, as a client does.
#[cfg(test)]
pub(crate) fn apply_gui_library_command(app: &mut App, command: crate::gui::GuiLibraryCommand) {
    with_stub_gui(app, |core| core.apply_gui_library_command(command));
}

/// One GUI core step over a stub `App` a test holds: the test helper for the
/// launch tests, which run the launch location's phases through the GUI core
/// over the `App`, as a client does.
#[cfg(test)]
pub(crate) fn restore_launch_collections(
    app: &mut App,
    state: &mbv_config::GuiLaunchState,
    destination: &mbv_config::TabIdentity,
) -> bool {
    with_stub_gui(app, |core| {
        core.restore_gui_launch_collections(state, destination)
    })
}

/// The launch location's first phase over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn open_launch_destination(
    app: &mut App,
    state: &mbv_config::GuiLaunchState,
) -> crate::gui::GuiLaunchOpen {
    with_stub_gui(app, |core| core.open_gui_launch_destination(state))
}

/// Run one GUI Library destination's ordered load pass over a stub `App` a
/// test holds: the test helper for the destination load tests, which run the
/// pass through the GUI core over the `App`, as a client does.
#[cfg(test)]
pub(crate) fn ensure_gui_destination(app: &mut App, kind: crate::gui::GuiDestinationKind) {
    with_stub_gui(app, |core| core.ensure_gui_destination(kind));
}

/// One destination's projection over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_audiobooks_projection(
    app: &mut App,
    lib_idx: usize,
) -> crate::gui::GuiAudiobooksProjection {
    with_stub_gui(app, |core| core.gui_audiobooks_projection(lib_idx))
}

/// One destination's projection over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_emby_projection(app: &mut App, lib_idx: usize) -> crate::gui::GuiEmbyProjection {
    with_stub_gui(app, |core| core.gui_emby_projection(lib_idx))
}

/// One destination's projection over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_shows_projection(
    app: &mut App,
    lib_idx: usize,
) -> crate::gui::GuiShowsProjection {
    with_stub_gui(app, |core| core.gui_shows_projection(lib_idx))
}

/// One destination's Podcasts projection over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_podcasts_projection(
    app: &mut App,
    lib_idx: usize,
) -> crate::gui::GuiPodcastsProjection {
    with_stub_gui(app, |core| core.gui_podcasts_projection(lib_idx))
}

/// Select one Feeds entry over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_feeds_select(app: &mut App, entry: &str) {
    with_stub_gui(app, |core| core.gui_feeds_select(entry));
}

/// One destination's Feeds projection over a stub `App` a test holds, over the
/// Client state the tests seeded beside it.
#[cfg(test)]
pub(crate) fn gui_feeds_projection_seeded(app: &App) -> crate::gui::GuiFeedsProjection {
    stub_gui_state(|gui| crate::gui::gui_library::feeds_projection(app, gui))
}

/// The entries the Feeds selection lists over the state a test seeded beside
/// the stub `App` it holds.
#[cfg(test)]
pub(crate) fn gui_feeds_entries_seeded(app: &App) -> Vec<mbv_queue::FeedEntry> {
    stub_gui_state(|gui| {
        crate::gui::gui_library::feeds_entries(app, gui)
            .into_iter()
            .cloned()
            .collect()
    })
}

/// The Feeds effective set over the state a test seeded beside the stub `App`
/// it holds.
#[cfg(test)]
pub(crate) fn gui_feeds_effective_set_seeded(app: &App) -> crate::gui::GuiCollectionSet {
    stub_gui_state(|gui| crate::gui::gui_library::feeds_effective_set(app, gui).into_owned())
}

/// The entry the Feeds Hero shows over the state a test seeded beside the stub
/// `App` it holds.
#[cfg(test)]
pub(crate) fn gui_feeds_picked_entry_seeded() -> Option<String> {
    stub_gui_state(|gui| crate::gui::gui_library::feeds_picked_entry(gui).map(str::to_string))
}

/// Open one Shows library over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_tv_open(app: &mut App, lib_idx: usize) {
    with_stub_gui(app, |core| core.gui_tv_open(lib_idx));
}

/// Select one Shows series over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_tv_select_series(app: &mut App, lib_idx: usize, id: &str) {
    with_stub_gui(app, |core| core.gui_tv_select_series(lib_idx, id));
}

/// Select one Shows season over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_tv_select_season(app: &mut App, lib_idx: usize, id: &str) {
    with_stub_gui(app, |core| core.gui_tv_select_season(lib_idx, id));
}

/// Land one Shows New fetch over a stub `App` a test holds: the result
/// installs in the Client's own stores through the one drain the GUI tick runs
/// (design D5 of `decouple-client-core`), as the worker's send would.
#[cfg(test)]
pub(crate) fn handle_gui_shows_new(
    core: &mut GuiCore,
    library_id: &str,
    result: Result<Vec<mbv_emby_model::EmbyItem>, mbv_emby::EmbyError>,
) {
    install_client_event(
        core.gui_state_mut(),
        GuiClientEvent::ShowsNew {
            library_id: library_id.to_string(),
            result,
        },
    );
}

/// The Music destination's projection over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_music_projection(
    app: &mut App,
    lib_idx: usize,
) -> crate::gui::GuiMusicProjection {
    with_stub_gui(app, |core| core.gui_music_projection(lib_idx))
}

/// The Home destination's projection over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_home_projection(app: &mut App) -> crate::gui::GuiHomeProjection {
    with_stub_gui(app, GuiCore::gui_home_projection)
}

/// One music artist row's art over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn music_artist_row_art(
    app: &App,
    key: &mbv_ui_model::music_grouping::ArtistKey,
) -> crate::gui::GuiRowArt {
    crate::gui::gui_library::music_artist_row_art(app, key)
}

/// One flat list's active sort over a stub `App` a test holds, read from the
/// Client state the tests keep beside it.
#[cfg(test)]
pub(crate) fn gui_list_sort(
    app: &App,
    target: crate::gui::GuiSortTarget,
) -> crate::gui::GuiListSort {
    stub_gui_state(|gui| crate::gui::gui_library::list_sort(app, gui, target))
}

/// Set one flat list's sort over a stub `App` a test holds, in the Client
/// state the tests keep beside it.
#[cfg(test)]
pub(crate) fn set_gui_list_sort(
    app: &mut App,
    target: crate::gui::GuiSortTarget,
    sort: crate::gui::GuiListSort,
) {
    stub_gui_state(|gui| {
        crate::gui::gui_library::set_list_sort(app, gui, target, sort);
    });
}

/// Open one plain Emby library over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_emby_open(app: &mut App, lib_idx: usize) {
    with_stub_gui(app, |core| core.gui_emby_open(lib_idx));
}

/// Select one plain Emby library row over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_emby_select(app: &mut App, lib_idx: usize, id: &str) {
    with_stub_gui(app, |core| core.gui_emby_select(lib_idx, id));
}

/// Open one Audiobooks library over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_abs_open(app: &mut App, lib_idx: usize) {
    with_stub_gui(app, |core| core.gui_abs_open(lib_idx));
}

/// Select one Audiobooks book over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_abs_select(app: &mut App, lib_idx: usize, id: &str) {
    with_stub_gui(app, |core| core.gui_abs_select(lib_idx, id));
}

/// Open one Podcasts destination over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_podcast_open(app: &mut App, lib_idx: usize) {
    with_stub_gui(app, |core| core.gui_podcast_open(lib_idx));
}

/// Select one Podcasts episode over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_podcast_select(
    app: &mut App,
    lib_idx: usize,
    target: &mbv_ui_msg::PodcastEpisodeTarget,
) {
    let row = crate::gui::gui_library::GuiPodcastEpisodeId::from(target).encode();
    apply_gui_library_command(
        app,
        crate::gui::GuiLibraryCommand::PodcastsSelect { lib_idx, row },
    );
}

/// The right-click row menu over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_library_row_menu(app: &mut App) -> Option<crate::gui::GuiLibraryRowMenu> {
    with_stub_gui(app, GuiCore::gui_library_row_menu)
}

/// Queue the in-view row fetches over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_queue_in_view_row_fetches(app: &mut App) {
    with_stub_gui(app, GuiCore::gui_queue_in_view_row_fetches);
}

/// Whether an interrupted art retry came due, over a stub `App` a test holds.
#[cfg(test)]
pub(crate) fn gui_art_retry_due(
    app: &App,
    after: std::time::Instant,
    until: std::time::Instant,
) -> bool {
    crate::gui::gui_library::marks::art_retry_due(app, after, until)
}

/// The GUI Client's own state a test seeds and reads, with the `App` the test
/// drives beside it (`decouple-client-core` design D4, row 8.1). The state
/// lives in `GuiCore` now, and these seams drive an `App` alone, so the state
/// is held in the sidecar this module keeps, not inside the `App`. A test's
/// seeds write that sidecar, so a seeded store reaches the store its call
/// reads and the frame it asserts on.
///
/// A Client's test names this trait and calls the seams as `app.<seam>(..)`,
/// the shape the `App` methods had.
pub trait GuiStateSeams {
    /// Seed the GUI Music store with `albums` for the `group_id` of `lib_idx`,
    /// as a landed fetch would (add-gui-library-panel design D5): a projection
    /// or frame test reads the store alone, with no request behind it. Each
    /// album is a folder, as the live parse makes every `MusicAlbum`
    /// (`mbv_emby::parse_item`), so its item menu is the album menu.
    fn seed_gui_music_albums_for_test(
        &mut self,
        lib_idx: usize,
        group_id: &str,
        albums: &[(&str, &str)],
    );

    /// The Grouped Music library's first checked group id
    /// (add-gui-library-panel design D5; gui-collection-dropdown design D1):
    /// `None` while All is selected or no entry is stored.
    fn gui_library_groups_for_test(&self, lib_idx: usize) -> Option<String>;

    /// Seed the GUI Music selection with the single checked group `group_id`
    /// for `lib_idx`, as a landed set-collections command would
    /// (add-gui-library-panel): a command test pins the selection before its
    /// first step, so the launch restore's own application skips it and the
    /// test's mock answers serve the test's commands alone.
    fn seed_gui_library_groups_for_test(&mut self, lib_idx: usize, group_id: &str);

    /// The album ids the GUI Music store loaded for `group_id`; `None` while
    /// the group is loading or after its fetch failed (design D5).
    fn gui_group_album_ids_for_test(&self, lib_idx: usize, group_id: &str) -> Option<Vec<String>>;

    /// The album a collection group's pill last showed (design D13).
    fn gui_library_pick_for_test(&self, lib_idx: usize, group_id: &str) -> Option<String>;

    /// The row the GUI Library's Hero shows for the plain Emby library
    /// `lib_idx` (add-gui-library-panel row 8.1).
    fn gui_emby_picked_row_for_test(&self, lib_idx: usize) -> Option<String>;

    /// The root item ids the GUI's Emby root-list store loaded for `lib_idx`;
    /// `None` while the list is loading or after its fetch failed (row 8.1).
    fn gui_emby_root_ids_for_test(&self, lib_idx: usize) -> Option<Vec<String>>;

    /// Seed the GUI's plain Emby root-list store with `items` for `lib_idx`,
    /// as a landed fetch would (add-gui-library-panel row 8.1): a Client's
    /// command test injects the rows and observes the frame with no fetch
    /// behind it.
    fn seed_gui_emby_roots_for_test(
        &mut self,
        lib_idx: usize,
        items: Vec<mbv_emby_model::EmbyItem>,
    );

    /// The book the GUI Library's Hero shows for the Audiobookshelf book
    /// library `lib_idx` (add-gui-library-panel row 9.1).
    fn gui_abs_picked_book_for_test(&self, lib_idx: usize) -> Option<String>;

    /// Record `album_id` as the pick of `group_id` in `lib_idx`, as the album
    /// pick command does: the selected group's Hero shows the pick while the
    /// group's union holds it.
    fn pick_gui_album_for_test(&mut self, lib_idx: usize, group_id: &str, album_id: &str);
}

impl GuiStateSeams for App {
    fn seed_gui_music_albums_for_test(
        &mut self,
        lib_idx: usize,
        group_id: &str,
        albums: &[(&str, &str)],
    ) {
        let albums = albums
            .iter()
            .map(|(id, name)| {
                let mut album = mbv_emby_model::test_support::make_item(name, "MusicAlbum");
                album.id = (*id).to_string();
                album.is_folder = true;
                album
            })
            .collect();
        gui(self, |gui| {
            gui.music_albums
                .entry(lib_idx)
                .or_default()
                .insert(group_id.to_string(), GuiGroupAlbums::Loaded(albums));
        });
    }

    fn gui_library_groups_for_test(&self, lib_idx: usize) -> Option<String> {
        gui(self, |gui| {
            match gui.library_groups.get(&lib_idx).map(|stored| &stored.set) {
                Some(GuiCollectionSet::Checked(keys)) => keys.first().cloned(),
                Some(GuiCollectionSet::All) | None => None,
            }
        })
    }

    fn seed_gui_library_groups_for_test(&mut self, lib_idx: usize, group_id: &str) {
        gui(self, |gui| {
            gui.library_groups.insert(
                lib_idx,
                GuiCollectionSelection {
                    set: GuiCollectionSet::Checked(vec![group_id.to_string()]),
                },
            );
        });
    }

    fn gui_group_album_ids_for_test(&self, lib_idx: usize, group_id: &str) -> Option<Vec<String>> {
        gui(self, |gui| {
            match gui
                .music_albums
                .get(&lib_idx)
                .and_then(|groups| groups.get(group_id))
            {
                Some(GuiGroupAlbums::Loaded(albums)) => {
                    Some(albums.iter().map(|album| album.id.clone()).collect())
                }
                Some(GuiGroupAlbums::Loading | GuiGroupAlbums::Failed) | None => None,
            }
        })
    }

    fn gui_library_pick_for_test(&self, lib_idx: usize, group_id: &str) -> Option<String> {
        gui(self, |gui| {
            gui.library_picks
                .get(&(lib_idx, group_id.to_string()))
                .cloned()
        })
    }

    fn gui_emby_picked_row_for_test(&self, lib_idx: usize) -> Option<String> {
        gui(self, |gui| {
            gui.emby_libs
                .get(&lib_idx)
                .and_then(|selection| selection.picked_row.clone())
        })
    }

    fn gui_emby_root_ids_for_test(&self, lib_idx: usize) -> Option<Vec<String>> {
        gui(self, |gui| match gui.emby_roots.get(&lib_idx) {
            Some(GuiEmbyRootItems::Loaded(items)) => {
                Some(items.iter().map(|item| item.id.clone()).collect())
            }
            Some(GuiEmbyRootItems::Loading | GuiEmbyRootItems::Failed) | None => None,
        })
    }

    fn seed_gui_emby_roots_for_test(
        &mut self,
        lib_idx: usize,
        items: Vec<mbv_emby_model::EmbyItem>,
    ) {
        gui(self, |gui| {
            gui.emby_roots
                .insert(lib_idx, GuiEmbyRootItems::Loaded(items));
        });
    }

    fn gui_abs_picked_book_for_test(&self, lib_idx: usize) -> Option<String> {
        gui(self, |gui| {
            gui.abs_libs
                .get(&lib_idx)
                .and_then(|selection| selection.picked_book.clone())
        })
    }

    fn pick_gui_album_for_test(&mut self, lib_idx: usize, group_id: &str, album_id: &str) {
        gui(self, |gui| {
            gui.library_picks
                .insert((lib_idx, group_id.to_string()), album_id.to_string());
        });
    }
}

/// The entries of the GUI queue row menu a right-click on `slot` opens, as the
/// Client's frame paints them: the labels and their order.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn gui_row_menu_entries_for_test(
    app: &mut App,
    slot: mbv_queue::QueueSlotId,
) -> Vec<mbv_ui_model::context_menu::ContextMenuEntry> {
    let mut core = GuiCore::new(std::mem::replace(app, handoff_app()));
    core.open_gui_row_menu(&[slot]);
    let entries = match core.gui.take_overlay_for_test() {
        Some(crate::gui::gui_client_state::GuiOverlay::RowMenu(menu)) => menu.entries,
        _ => Vec::new(),
    };
    *app = core.into_app();
    entries
}

/// The `App` a Client's test drives with the queue built from `items` and
/// the row menu open over its first slot, so a frame test reads the open
/// menu's entries and their key hints (add-tauri-gui-client row 6.2). The
/// menu is the same one `RowContextMenu`'s open path builds.
///
/// # Panics
///
/// Panics while resolving the menu's row when `items` is empty: the stub
/// queue then has no first slot to open the menu over.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_open_row_menu(items: Vec<mbv_queue::QueueItem>) -> App {
    let app = crate::app::stub_app_with_playing_items(items);
    let row = app
        .displayed_queue()
        .slot_id_at(0)
        .expect("the stub queue has a first slot");
    let mut core = GuiCore::new(app);
    core.open_gui_row_menu(&[row]);
    // The overlay is the wrapper's own Client state, and the test reads the
    // state beside the `App` (row 8.1 of `decouple-client-core`), so what the
    // chord opened goes back out.
    core.publish_seeded_gui_state_for_test();
    core.into_app()
}

/// The `App` a Client's test drives with an active player carrying one audio
/// and one subtitle track, and the track chip's popover open on `chip`, so a
/// frame test reads the popover's choices with their chosen marks
/// (add-tauri-gui-client row 6.2).
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_track_popover(chip: mbv_ui_msg::TrackChipKind) -> App {
    let mut app = crate::app::stub_app_with_playing_queue(1);
    app.edit_player_status_for_test(|status| {
        status.active = true;
        status.audio_tracks = vec![(1, "Stereo".to_string())];
        status.audio_id = 1;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });
    // The popover opens through the GUI Client's own half of the chip press:
    // the shared playback dispatch's arm for it is a documented core no-op
    // (design D4a (a)), and the GUI entry applies it after the core. An
    // inactive player lists no choice, so it opens nothing.
    let mut core = GuiCore::new(std::mem::replace(&mut app, crate::app::make_app_stub()));
    core.toggle_gui_track_popover(chip);
    // The overlay is the wrapper's own Client state, and the test reads the
    // state beside the `App` (row 8.1 of `decouple-client-core`), so what the
    // press opened goes back out.
    core.publish_seeded_gui_state_for_test();
    core.into_app()
}

/// The `App` a GUI Client's test drives with the Settings screen open at
/// Services and a failed Emby setup form whose password field still holds the
/// typed secret (`add-gui-settings-screen` row 4.3): the frame's Services
/// content must carry the error and the server URL and never the password.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub fn stub_app_with_failed_emby_setup() -> App {
    let mut core = GuiCore::new(crate::app::stub_app_with_failed_emby_setup_form());
    core.apply_gui_settings_overlay(&mbv_ui_model::overlay::OverlayRequest::OpenSidebar(
        mbv_ui_model::overlay::SidebarId::Settings,
    ));
    // The open screen is the wrapper's own Client state, and the test reads
    // the state beside the `App` (row 8.1 of `decouple-client-core`), so it
    // goes back out.
    core.publish_seeded_gui_state_for_test();
    core.into_app()
}
