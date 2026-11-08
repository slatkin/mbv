//! A self-contained `App` stub for the Clients' own tests
//! (`extract-client-core` design D11): the ordinary `App::build` construction
//! with stub-friendly defaults, no live Owner, and no live Service. The GUI
//! Client's headless tests build their `App` here, exactly as the TUI
//! Client's tests build theirs in `app::tests`.
//!
//! The module compiles under the root package's `test` feature as well as
//! under `cfg(test)`, so a dependent crate's dev-dependency can enable it;
//! the shipped builds never compile it.

use std::sync::{Arc, Mutex};

use super::{App, AppInit, QueueView};

/// Minimal `App` stub for logic-only tests: the ordinary `App::build`
/// construction ([`make_built_app`]) plus the stub-friendly defaults below.
#[must_use]
pub fn make_app_stub() -> App {
    let mut app = make_built_app();
    apply_app_stub_defaults(&mut app);
    app
}

fn apply_app_stub_defaults(app: &mut App) {
    // `build` doubles the configured image cache size; the stub keeps its
    // original (undoubled) budget so eviction behaviour is unchanged.
    app.images.set_cache_capacity_for_test(50);
    // Ignore any on-disk feed entry state; a stub starts empty.
    app.feed_entry_state = mbv_feed::FeedEntryStore::default();
    // Default to a focused window that reported focus in the past, so
    // existing mouse tests dispatch without arming focus explicitly.
    app.refocus_at = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .unwrap(),
    );
}

/// The `App` a Client's test drives, with `n` projected queue slots so a
/// list surface has content. The slots are Emby movie stubs in canonical
/// order, cursor at the first.
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_queue(n: usize) -> App {
    let mut app = make_app_stub();
    app.local_view = queue_view(n);
    // The stub player is idle, so the Queue playback band collapses and the
    // list keeps the whole window (task 3.6).
    app.player.update_status(|status| status.active = false);
    app
}

/// The `App` a GUI Client's test drives with `n` queue slots over a player
/// whose command channel records what the actions send (add-tauri-gui-client
/// row 6.5): a drag's release or a key move lands one slot move the test
/// reads. The stub owner advertises no `answered-queue-ops`, so a queue edit
/// sends without waiting for an answer.
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_queue_and_cmd_rx(
    n: usize,
) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (mut app, cmd_rx) = app_with_cmd_rx();
    app.local_view = queue_view(n);
    app.player.update_status(|status| status.active = false);
    (app, cmd_rx)
}

/// The stub `App` over a player whose command channel records what an action
/// sends (add-tauri-gui-client row 6.5): the stub owner advertises no
/// `answered-queue-ops`, so a queue edit sends without waiting for an answer.
/// The construction's own preference sync lands on the channel before the
/// caller starts, so it is drained here and a test sees only what its actions
/// send.
#[cfg(feature = "test")]
fn app_with_cmd_rx() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (mut remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    let mut compatibility = mbv_ctrl::CtrlCompatibility::current();
    compatibility.supports_answered_queue_ops = false;
    remote.set_ctrl_compatibility_for_test(compatibility);
    let player = mbv_player::PlayerProxy::from_remote(remote, false);
    let mut app = built_app(player, player_rx);
    apply_app_stub_defaults(&mut app);
    while cmd_rx.try_recv().is_ok() {}
    (app, cmd_rx)
}

/// The projected queue view the queue stubs share: `n` Emby movie slots in
/// canonical order, numbered from 1.
#[cfg(feature = "test")]
fn queue_view(n: usize) -> QueueView {
    let slots: Vec<mbv_ctrl::UnifiedQueueSlot> = (0..n)
        .map(|i| mbv_ctrl::UnifiedQueueSlot {
            slot_id: u64::try_from(i).unwrap_or(u64::MAX) + 1,
            item: mbv_queue::QueueItem::Emby(Box::new(mbv_emby_model::test_support::make_item(
                &format!("Item {i}"),
                "Movie",
            ))),
        })
        .collect();
    let state = mbv_ctrl::UnifiedQueueStateData {
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot: None,
        slots,
        revision: 1,
        source: mbv_queue::QueueSource::Unknown,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    QueueView::from_snapshot(&state)
}

/// The `App` a Client's test drives with `n` projected queue slots and the
/// first one playing, so the Queue playback band shows its transport
/// (`extract-client-core` row 6.5).
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_playing_queue(n: usize) -> App {
    let app = stub_app_with_queue(n);
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.queue_len = n;
    });
    app
}

/// The `App` a Client's test drives with the queue's slots built from
/// `items` in canonical order and the first one playing, so a frame test can
/// name watched, partial-progress, and active rows from the items' own
/// stored facts (add-tauri-gui-client row 6.2). The playing slot starts
/// fresh: the status carries the first item's runtime and no position, so a
/// queued item's stored progress is the partial-progress row's own fact.
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_playing_items(items: Vec<mbv_queue::QueueItem>) -> App {
    let app = stub_app_with_items(items);
    let runtime = app
        .playback_queue()
        .item_at(0)
        .map_or(0, mbv_queue::QueueItem::runtime_ticks);
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.queue_len = app.playback_queue().slots().len();
        status.runtime_ticks = runtime;
    });
    app
}

/// The `App` a Client's test drives with the queue's slots built from
/// `items` in canonical order, cursor at the first, and an idle player.
#[cfg(feature = "test")]
#[must_use]
fn stub_app_with_items(items: Vec<mbv_queue::QueueItem>) -> App {
    let mut app = make_app_stub();
    let slots: Vec<mbv_ctrl::UnifiedQueueSlot> = items
        .into_iter()
        .enumerate()
        .map(|(i, item)| mbv_ctrl::UnifiedQueueSlot {
            slot_id: u64::try_from(i).unwrap_or(u64::MAX) + 1,
            item,
        })
        .collect();
    let state = mbv_ctrl::UnifiedQueueStateData {
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot: None,
        slots,
        revision: 1,
        source: mbv_queue::QueueSource::Unknown,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    app.local_view = QueueView::from_snapshot(&state);
    app
}

/// The `App` a Client's test drives with `n` projected queue slots and the
/// queue marked unsaved, so the Queue footer carries its dirty pill
/// (`extract-client-core` row 6.4).
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_unsaved_queue(n: usize) -> App {
    let mut app = stub_app_with_queue(n);
    app.queue_dirty = true;
    app
}

/// The `App` a Client's test drives with prefix mode armed and `message`
/// flashed as a live Neutral toast, so the Status bar carries both
/// (`extract-client-core` row 6.5).
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_prefix_and_toast(message: &str) -> App {
    use mbv_ui_model::toast::ToastSeverity;

    let mut app = make_app_stub();
    app.prefix_armed = true;
    app.flash(message.to_string(), ToastSeverity::Neutral);
    app
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
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_open_row_menu(items: Vec<mbv_queue::QueueItem>) -> App {
    let mut app = stub_app_with_playing_items(items);
    let row = app
        .displayed_queue()
        .slot_id_at(0)
        .expect("the stub queue has a first slot");
    app.open_gui_row_menu(&[row]);
    app
}

/// The `App` a Client's test drives with an active player carrying one audio
/// and one subtitle track, and the track chip's popover open on `chip`, so a
/// frame test reads the popover's choices with their chosen marks
/// (add-tauri-gui-client row 6.2).
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_track_popover(chip: mbv_ui_msg::TrackChipKind) -> App {
    use mbv_ui_msg::PlaybackRequest;

    let mut app = stub_app_with_playing_queue(1);
    app.player.update_status(|status| {
        status.active = true;
        status.audio_tracks = vec![(1, "Stereo".to_string())];
        status.audio_id = 1;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });
    app.handle_playback_request(&PlaybackRequest::OpenTrackPopover(chip));
    app
}
/// The `App` a GUI Client's test drives with the cast receivers replaced by
/// `receivers` (add-tauri-gui-client row 6.5's paint/press regression): the
/// target list the last emitted frame painted can then differ from the list
/// a press would read, so a press must address the painted choice.
#[cfg(feature = "test")]
pub fn set_cast_receivers_for_test(app: &mut App, receivers: Vec<(&str, &str)>) {
    app.cast_receivers = receivers
        .into_iter()
        .map(|(id, name)| mbv_cast::discovery::CastReceiver {
            id: id.to_string(),
            friendly_name: name.to_string(),
            host: "192.168.0.5".to_string(),
            port: 8009,
        })
        .collect();
    app.rebuild_panel_targets();
}

/// Replace the local player's subtitle track list (add-tauri-gui-client row
/// 6.5's paint/press regression): the popover's painted rows then name a
/// list that changed before the press.
#[cfg(feature = "test")]
pub fn set_subtitle_tracks_for_test(app: &mut App, tracks: Vec<(i64, String, bool)>) {
    app.player
        .update_status(|status| status.sub_tracks = tracks);
}

/// The `App` a Client's test drives with a pending settings save armed: the
/// config carries `consume_audio` and the save's deadline lies far in the
/// future, so only an exit boundary's flush can persist it — the periodic
/// maintenance the Client ticks run must not win the race the test observes.
///
/// # Panics
///
/// Panics while arming the save when the configuration lock is poisoned: a
/// writer panicked holding it, and the cached configuration cannot be
/// trusted.
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_pending_settings_save() -> App {
    let mut app = make_app_stub();
    app.config.lock().unwrap().consume_audio = true;
    app.settings_save_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3600));
    app
}

/// The `App` a Client's test drives with `quit` rebound to `chord` in the
/// Session section of the compiled keybinds: the one router resolves the
/// action's configured chords, so the rebound chord routes to quit in the
/// GUI exactly as a user's `[keys]` override would (add-tauri-gui-client
/// row 6.2: the GUI-only context captures the default `q` for the fold).
///
/// # Panics
///
/// Panics while parsing the chord when `chord` is not a chord grammar
/// string.
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_rebound_quit(chord: &str) -> App {
    use mbv_keybinds::{Chord, KeySection, Keybinds, SectionBindings};

    let app = stub_app_with_queue(2);
    app.config.lock().unwrap().keybinds = Keybinds {
        prefix: None,
        sections: vec![(
            KeySection::Session,
            SectionBindings {
                router: vec![("quit", Chord::parse(chord).expect("test chord must parse"))],
                prefix: vec![],
            },
        )],
    };
    app
}

/// The `App` a GUI Client's test drives with a cast receiver the Owner
/// offers as a playback target and a ten-minute item playing
/// (add-tauri-gui-client row 6.4): the `panel_targets` merge holds the
/// receiver, the player's Owner advertises the `playback-output`
/// capability the cast row's presence is gated on, and the command channel
/// records what a chooser selection or a transport request sends to the
/// Owner. The receiver's id is `receiver-1`.
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_cast_target() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (mut remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    let mut compatibility = mbv_ctrl::CtrlCompatibility::current();
    compatibility.supports_playback_output = true;
    remote.set_ctrl_compatibility_for_test(compatibility);
    remote.update_status(|status| {
        status.active = true;
        status.runtime_ticks = 10 * 60 * mbv_emby_model::TICKS_PER_SECOND;
    });
    let player = mbv_player::PlayerProxy::from_remote(remote, false);
    let mut app = built_app(player, player_rx);
    apply_app_stub_defaults(&mut app);
    app.cast_receivers = vec![mbv_cast::discovery::CastReceiver {
        id: "receiver-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    }];
    app.rebuild_panel_targets();
    // The construction's preference sync lands on the channel before the
    // test starts; drain it so the test sees only commands its actions send.
    while cmd_rx.try_recv().is_ok() {}
    (app, cmd_rx)
}

/// The `App` a GUI Client's test drives with an active local player
/// carrying one audio track (id 1, chosen) and one forced subtitle track
/// (id 2; id 0, Off, is chosen), so a chip press opens its popover and a
/// choice records on the command channel (add-tauri-gui-client row 6.4):
/// the same shape `stub_app_with_track_popover` arms, over a remote stub
/// that exposes what a track switch sends the Owner.
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_track_choices() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.update_status(|status| {
        status.active = true;
        status.audio_tracks = vec![(1, "Stereo".to_string())];
        status.audio_id = 1;
        status.sub_tracks = vec![(2, "Forced".to_string(), true)];
        status.sub_id = 0;
    });
    let player = mbv_player::PlayerProxy::from_remote(remote, false);
    let mut app = built_app(player, player_rx);
    apply_app_stub_defaults(&mut app);
    // The construction's preference sync lands on the channel before the
    // test starts; drain it so the test sees only commands its actions send.
    while cmd_rx.try_recv().is_ok() {}
    (app, cmd_rx)
}

/// The `App` a GUI Client's test drives with the Emby client an attach
/// brings (`owner_link`'s `cached_emby_client`): the runtime is Ready and
/// holds `client`, so the shared startup step starts no Emby startup worker
/// (add-gui-library-panel design D11). The test builds the client over an
/// isolated mock transport, so no request leaves the process.
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_cached_emby_client(client: mbv_emby::EmbyClient) -> App {
    let mut app = make_app_stub();
    app.emby_runtime =
        crate::app::state::service_runtime::EmbyRuntime::ready(Arc::new(Mutex::new(client)));
    app
}

/// The library id the seeded Music library tab holds.
#[cfg(feature = "test")]
pub const GUI_MUSIC_LIBRARY_ID: &str = "lib-music";

/// A music group level: the library root's children, its cursor on the first.
#[cfg(feature = "test")]
fn music_group_level(groups: Vec<mbv_emby_model::EmbyItem>) -> mbv_ui_model::browse::BrowseLevel {
    use mbv_ui_model::browse::{BrowseResting, ServerRows};

    mbv_ui_model::browse::BrowseLevel {
        parent_id: GUI_MUSIC_LIBRARY_ID.to_string(),
        title: "Music".to_string(),
        items: groups,
        rows: ServerRows::new(0),
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".to_string(),
        sort_order: "Ascending".to_string(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

/// A group's album level: the group's children, its cursor on the first.
#[cfg(feature = "test")]
fn music_album_level(albums: Vec<mbv_emby_model::EmbyItem>) -> mbv_ui_model::browse::BrowseLevel {
    let mut level = music_group_level(albums);
    level.parent_id = "grp-hiphop".to_string();
    level.title = "Hip-Hop".to_string();
    level
}

/// A group row of the music group level.
#[cfg(feature = "test")]
fn group_item(id: &str, name: &str) -> mbv_emby_model::EmbyItem {
    let mut item = mbv_emby_model::test_support::make_item(name, "MusicArtist");
    item.id = id.to_string();
    item.is_folder = true;
    item
}

/// An album row of a group's album level: a folder with no playable media of
/// its own, as Emby returns a `MusicAlbum`.
#[cfg(feature = "test")]
fn album_item(id: &str, name: &str) -> mbv_emby_model::EmbyItem {
    let mut item = mbv_emby_model::test_support::make_item(name, "MusicAlbum");
    item.id = id.to_string();
    item.is_folder = true;
    item.media_type.clear();
    item
}

/// The `App` a GUI Client's test drives with a successful Emby startup
/// completion already queued for the startup drain, over `views` as the
/// bootstrap's library catalog. The worker's thread never runs: the test
/// hands the drain the one completion a worker would send (add-gui-library-panel
/// design D11).
#[cfg(feature = "test")]
#[must_use]
pub fn stub_app_with_completed_emby_startup(views: Vec<mbv_emby_model::EmbyItem>) -> App {
    let mut app = make_app_stub();
    let generation = app.emby_runtime.generation();
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = tx.send(crate::app::dispatch::session::service_startup::Completion {
        generation,
        result: Ok(crate::app::dispatch::session::service_startup::Startup {
            client: mbv_emby::EmbyClient::new(crate::config::Config::default()),
            bootstrap: mbv_emby::EmbyBootstrap {
                continue_items: Vec::new(),
                views,
            },
            setup: mbv_config::EmbySetup::default(),
        }),
    });
    app.setup.emby_startup_rx =
        Some(crate::app::dispatch::session::service_startup::StartupReceiver { generation, rx });
    app
}

/// The test-only reads a Client's test makes of this `App`'s state. The gate
/// spans the crate's own tests and a dependent Client's `test` build, as
/// `make_app_stub` above does.
#[cfg(any(test, feature = "test"))]
impl App {
    /// The Library tabs the catalog holds, for a Client's test that asserts a
    /// startup landed the catalog (add-gui-library-panel design D11).
    #[must_use]
    pub fn libs_for_test(&self) -> &[mbv_ui_model::library_tab::LibraryTab] {
        &self.libs
    }

    /// Make the grouped Music library the GUI Library test's destination
    /// (add-gui-library-panel row 3.1): the Emby runtime becomes Ready with
    /// `client` (built over a mock transport), and the Music tab's group level
    /// holds Hip-Hop and Soul with Hip-Hop's albums above it. A Client seeds it
    /// after its core thread is built, because the launch Home load rebuilds
    /// the catalog from the Service's views; Music is then the active
    /// destination, as the Library panel keeps it.
    #[cfg(feature = "test")]
    pub fn seed_gui_music_library_for_test(&mut self, client: mbv_emby::EmbyClient) {
        self.emby_runtime =
            crate::app::state::service_runtime::EmbyRuntime::ready(Arc::new(Mutex::new(client)));
        self.seed_gui_music_library_tab_for_test();
    }

    /// The grouped Music library without a Service client (add-gui-library-panel
    /// design D9): a launch-location test seeds the catalog this way, so the
    /// launch Home load starts no Emby request and the seeded tabs survive it.
    /// The Music tab's group level holds Hip-Hop and Soul with Hip-Hop's albums
    /// above it, and Music is the active destination.
    #[cfg(feature = "test")]
    pub fn seed_gui_music_library_tab_for_test(&mut self) {
        self.music_levels = vec!["group".to_string(), "album".to_string()];
        let mut library = mbv_emby_model::test_support::make_item("Music", "CollectionFolder");
        library.id = GUI_MUSIC_LIBRARY_ID.to_string();
        library.collection_type = "music".to_string();
        library.is_folder = true;
        let mut tab = mbv_ui_model::library_tab::LibraryTab::new(library);
        tab.nav_stack.push(music_group_level(vec![
            group_item("grp-hiphop", "Hip-Hop"),
            group_item("grp-soul", "Soul"),
        ]));
        tab.nav_stack.push(music_album_level(vec![
            album_item("alb-ill", "Licensed to Ill"),
            album_item("alb-paul", "Paul's Boutique"),
        ]));
        self.libs = vec![tab];
        self.tab = mbv_ui_model::tab_selection::TabSelection::EmbyLibrary(0);
        for album_id in ["alb-ill", "alb-paul"] {
            self.album_artist_cache
                .insert(album_id.to_string(), "Beastie Boys".to_string());
        }
    }

    /// The active destination, for a Client's test that asserts an open-
    /// destination command landed (add-gui-library-panel design D2).
    #[must_use]
    pub fn tab_for_test(&self) -> mbv_ui_model::tab_selection::TabSelection {
        self.tab
    }

    /// Push `library` as another Emby library tab (add-gui-library-panel
    /// design D9): a launch-location test holds a catalog destination that
    /// the GUI Client does not present.
    #[cfg(feature = "test")]
    pub fn push_gui_library_tab_for_test(&mut self, library: mbv_emby_model::EmbyItem) {
        self.libs
            .push(mbv_ui_model::library_tab::LibraryTab::new(library));
    }

    /// Seed the GUI Music store with `albums` for the `group_id` of `lib_idx`,
    /// as a landed fetch would (add-gui-library-panel design D5): a projection
    /// or frame test reads the store alone, with no request behind it. Each
    /// album is a folder, as the live parse makes every `MusicAlbum`
    /// (`mbv_emby::parse_item`), so its item menu is the album menu.
    #[cfg(feature = "test")]
    pub fn seed_gui_music_albums_for_test(
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
        self.gui_music_albums.insert(
            (lib_idx, group_id.to_string()),
            crate::app::gui_library::GuiGroupAlbums::Loaded(albums),
        );
    }

    /// Deliver `key`'s image as a completed fetch would (add-gui-library-panel
    /// design D13): the next tick's card-image drain lands it in the cache, so
    /// a frame test observes the art with no request and no thread.
    #[cfg(feature = "test")]
    pub fn complete_card_image_for_test(&self, key: &str, img: image::DynamicImage) {
        let _ = self
            .images
            .card_image_tx()
            .send((key.to_string(), mbv_images::ImageFetchOutcome::Image(img)));
    }

    /// The Grouped Music library's selected group ids in pill order
    /// (add-gui-library-panel design D5).
    #[must_use]
    pub fn gui_library_groups_for_test(&self, lib_idx: usize) -> &[String] {
        self.gui_library_groups
            .get(&lib_idx)
            .map_or(&[], Vec::as_slice)
    }

    /// Seed the GUI Music selection with `group_ids` for `lib_idx`, as a
    /// landed set-collections command would (add-gui-library-panel): a
    /// command test pins the selection before its first step, so the launch
    /// restore's own application skips it and the test's mock answers serve
    /// the test's commands alone.
    #[cfg(feature = "test")]
    pub fn seed_gui_library_groups_for_test(&mut self, lib_idx: usize, group_ids: &[&str]) {
        self.gui_library_groups.insert(
            lib_idx,
            group_ids.iter().map(|id| (*id).to_string()).collect(),
        );
    }

    /// The album ids the GUI Music store loaded for `group_id`; `None` while
    /// the group is loading or after its fetch failed (design D5).
    #[must_use]
    pub fn gui_group_album_ids_for_test(
        &self,
        lib_idx: usize,
        group_id: &str,
    ) -> Option<Vec<String>> {
        match self.gui_music_albums.get(&(lib_idx, group_id.to_string())) {
            Some(crate::app::gui_library::GuiGroupAlbums::Loaded(albums)) => {
                Some(albums.iter().map(|album| album.id.clone()).collect())
            }
            Some(
                crate::app::gui_library::GuiGroupAlbums::Loading
                | crate::app::gui_library::GuiGroupAlbums::Failed,
            )
            | None => None,
        }
    }

    /// The item the GUI Library's Hero shows (design D2).
    #[must_use]
    pub fn gui_library_detail_for_test(&self) -> Option<&mbv_emby_model::EmbyItem> {
        self.gui_library_detail.as_ref()
    }

    /// Whether the album's track fetch is in flight (design D5): the open-
    /// album command's effect before its completion lands.
    #[must_use]
    pub fn album_tracks_loading_for_test(&self, album_id: &str) -> bool {
        self.album_tracks_loading.contains(album_id)
    }

    /// Seed the album track cache with `tracks` for `album_id`, as a landed
    /// album-track fetch would (add-gui-library-panel): the autoload add and
    /// play rules read the cache, so a command test injects the tracks and
    /// observes the Owner request with no fetch behind it.
    #[cfg(feature = "test")]
    pub fn seed_gui_album_tracks_for_test(
        &mut self,
        album_id: &str,
        tracks: &[(&str, &str, i64, i64)],
    ) {
        let tracks = tracks
            .iter()
            .map(|(id, name, number, seconds)| {
                let mut track = mbv_emby_model::test_support::make_item(name, "Audio");
                track.id = (*id).to_string();
                track.media_type = "Audio".to_string();
                track.index_number = *number;
                track.runtime_ticks = seconds * mbv_emby_model::TICKS_PER_SECOND;
                track
            })
            .collect();
        self.album_tracks_cache.insert(album_id.to_string(), tracks);
    }

    /// Set the autoload setting (add-gui-library-panel): the Library's add
    /// and play commands read it, and a Client's test has no settings UI.
    ///
    /// # Panics
    /// Panics if the settings mutex is poisoned: a previous holder panicked.
    #[cfg(feature = "test")]
    pub fn set_autoload_for_test(&mut self, autoload: bool) {
        self.config.lock().unwrap().autoload = autoload;
    }

    /// Whether the core holds no pending TUI overlay (add-gui-library-panel
    /// row 3.6): the GUI runs an item-menu entry without arming one.
    #[must_use]
    pub fn pending_overlay_is_none_for_test(&self) -> bool {
        self.pending_overlay.is_none()
    }

    /// Whether the `/Sessions` load a playback-target discovery starts is in
    /// flight.
    #[must_use]
    pub fn sessions_load_started_for_test(&self) -> bool {
        self.sessions_loading
    }

    /// How many cast discovery browses this `App` started.
    #[must_use]
    pub fn cast_discovery_starts_for_test(&self) -> usize {
        self.cast_discovery_starts
    }
}

/// The ordinary `App::build` construction over stub channels: no live
/// Owner, no live Service, and the stub player whose command channel is
/// already dropped.
#[must_use]
pub fn make_built_app() -> App {
    use mbv_player::PlayerProxy;

    let (remote, player_rx) = mbv_remote_player::RemotePlayer::stub(Vec::new(), 0);
    remote.update_status(|status| status.volume_max = 100);
    built_app(PlayerProxy::from_remote(remote, false), player_rx)
}

/// The `App::build` construction over `player` and its event channel, the
/// shared half of every stub that needs its own player wiring (a command
/// channel of its own, say).
fn built_app(
    player: mbv_player::PlayerProxy,
    player_rx: std::sync::mpsc::Receiver<mbv_ctrl::player::PlayerEvent>,
) -> App {
    let (_, ws_rx) = std::sync::mpsc::channel();
    let (_, transport_rx) = std::sync::mpsc::channel();
    let (card_image_tx, card_image_rx) = std::sync::mpsc::channel();
    let channels = crate::app::state::runtime_channels::RuntimeChannels::new();

    let config = crate::config::Config::default();

    App::build(AppInit {
        config: Arc::new(Mutex::new(config)),
        emby_runtime: crate::app::state::service_runtime::EmbyRuntime::default(),
        audiobookshelf_runtime: crate::app::state::service_runtime::AudiobookshelfRuntime::new(
            false,
        ),
        player,
        player_rx,
        ws_rx,
        transport_rx,
        ws_send_tx: None,
        audiobookshelf_socket_rx: {
            let (_, rx) = std::sync::mpsc::channel();
            rx
        },
        audiobookshelf_socket_tx: None,
        audiobookshelf_socket_generation: None,
        local_view: QueueView::default(),
        remote_view: None,
        system_notifications: false,
        image_protocol: None,
        image_protocol_enabled: false,
        hidden_libraries: Vec::new(),
        library_routes: std::collections::BTreeMap::new(),
        music_levels: Vec::new(),
        use_nerd_fonts: false,
        indicator_style: mbv_render::indicators::IndicatorStyle::default(),
        image_cache_size: 50,
        visualizer_glyph: crate::config::DEFAULT_VISUALIZER_GLYPH.into(),
        card_image_tx,
        card_image_rx,
        channels,
        idle_feed: None,
    })
}
