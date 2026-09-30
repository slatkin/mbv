use super::*;
use crate::app::AudiobookshelfEvent;
use crate::app::SessionEvent;
use crate::app::dispatch::session::service_startup::{
    AudiobookshelfCatalogCompletion, AudiobookshelfCatalogReceiver, AudiobookshelfSetupCompletion,
};
use crate::app::state::events::LibEvent;
use crate::app::tests::make_app_stub;
use crate::app::tests::render_fixtures::make_movie_app;
use mbv_audiobookshelf::{
    AudiobookshelfBookProgress, AudiobookshelfError, AudiobookshelfFailureClass,
    AudiobookshelfLibrary, AudiobookshelfProgress, AudiobookshelfUser,
};
use mbv_core::service_runtime::{ServiceState, SetupGeneration};
use mbv_images::series_image_cache_key;
use mbv_render::components::hero_model::SERIES_LANDSCAPE_IMAGE_TYPES;
use mbv_ui_model::browse::ServerRows;
use rstest::rstest;
use std::collections::HashMap;

fn mounted_wide_tv_model() -> Model {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
        item.image_tags.thumb = "tag".into();
    }
    // Wide breakpoint is now driven synchronously by terminal size
    // (`wide_tv_library_area`), not this previous-frame paint rect.
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut model = Model::new(app);
    model.sync_tv_content();
    model.sync_active_destination();
    model
}

/// The Series artwork placeholder state the wide workspace is currently painting
/// (the panel reserves no image paint while the projected state is not ready).
fn wide_tv_shows_placeholder(model: &mut Model) -> bool {
    model
        .test_paint_library_panel(ratatui::layout::Rect::new(0, 0, 100, 30))
        .is_none()
}

/// Task 2.3: a Series completion is what re-projects the wide workspace, so the
/// cached Thumb-first entry replaces the placeholder. The gate covers the
/// `:ser:` family the painter builds its keys under.
#[test]
fn series_image_completion_repushes_tv_workspace_content() {
    let mut model = mounted_wide_tv_model();
    model.app.images.configure_protocol(None, true);
    // The sync pass is the production projection seam (task 5.10's central
    // hero projection owns the fetch for every migrated owner, TV included
    // since task 8.4). One throwaway draw publishes `root_frame` first — the
    // projection reads the panel's `RootFrame` placement, so it runs on the
    // sync after the shell's startup draw.
    {
        let backend = ratatui::backend::TestBackend::new(160, 40);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| model.draw_frame(f, false, false)).unwrap()
    };
    model.sync_mounted_surfaces();
    assert!(
        wide_tv_shows_placeholder(&mut model),
        "an uncached projection must paint the placeholder"
    );

    let painted_key = series_image_cache_key("movie-focused", SERIES_LANDSCAPE_IMAGE_TYPES);
    assert!(
        model.drain_card_image_completions(),
        "the Series prefetch must resolve into the cache"
    );
    assert!(
        model.app.images.is_cached(&painted_key),
        "the painted Series key must be cached"
    );
    // The fixture's fetch resolves to an empty cache entry (no pixel
    // protocol is available), so the shared producer now treats the
    // placeholder as final while the shell still re-projects the cached key.
    assert!(model.app.images.is_cached(&painted_key));
}

/// Task 2.3: no other image namespace may drive the TV projection. The cached
/// Series entry is planted without a re-push, so only a gate that wrongly
/// matches this key can clear the placeholder the component still holds.
/// Task 3.1: `clear:yes` dismisses the confirmation modal and routes the
/// clear-queue action, and the drain reports that it produced work.
#[test]
fn drain_notif_actions_clear_yes_dismisses_and_clears_queue() {
    let mut app = make_app_stub();
    app.channels
        .notif_action_tx
        .send("clear:yes".into())
        .expect("notif channel");

    assert!(
        app.drain_notif_actions(),
        "a queued action must report produced=true"
    );
    assert!(
        matches!(
            app.pending_overlay,
            Some(mbv_ui_model::overlay::OverlayRequest::DismissConfirm)
        ),
        "clear:yes must dismiss the confirmation modal"
    );
    assert_eq!(
        app.status, "Queue cleared",
        "clear:yes must route the clear-queue action"
    );
}

/// Task 3.1: `__notif_failed__` raises the notification-failure flag.
/// Task 3.1: payloads without a retained action -- the explicit no-ops and an
/// empty channel -- change no state; `produced` reflects only whether a
/// message was received, so an unrecognised payload still reports `true`
/// while the empty channel reports `false`.
#[rstest]
/// Task 3.2: a queued `SessionEvent` is dispatched to `handle_session_event`
/// and the drain reports that it produced work.
#[test]
fn drain_session_events_dispatches_a_queued_event() {
    let mut app = make_app_stub();
    app.sessions_loading = true;
    app.channels
        .sessions_tx
        .send(SessionEvent::Loaded {
            sessions: vec![mbv_emby::test_support::make_session("living-room", "mbv")],
        })
        .expect("sessions channel");

    assert!(
        app.drain_session_events(),
        "a queued event must report produced=true"
    );
    assert_eq!(app.sessions.len(), 1, "the Loaded event must be dispatched");
    assert!(!app.sessions_loading, "Loaded must clear the loading flag");
}

/// Task 3.2: an empty sessions channel reports produced=false.
fn audiobookshelf_library(id: &str, media_type: &str) -> AudiobookshelfLibrary {
    AudiobookshelfLibrary {
        id: id.into(),
        name: id.into(),
        media_type: media_type.into(),
    }
}

type CatalogResult = Result<
    (
        Vec<AudiobookshelfLibrary>,
        HashMap<(String, String), AudiobookshelfProgress>,
        HashMap<String, AudiobookshelfBookProgress>,
    ),
    AudiobookshelfError,
>;

fn catalog_receiver(
    generation: SetupGeneration,
    result: CatalogResult,
) -> AudiobookshelfCatalogReceiver {
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(AudiobookshelfCatalogCompletion { generation, result })
        .expect("catalog channel");
    AudiobookshelfCatalogReceiver { rx }
}

/// Reads exactly `expected` library-fetch events off the shell's `lib_tx`
/// channel. The stub config makes the spawned fetchers do no I/O — they send a
/// single `Err` completion and exit — so every expected event arrives promptly.
/// The 1s timeout only bounds a regression that fails to send (failing well
/// above the microseconds a send actually takes); it is not a synchronization
/// sleep.
fn collect_library_events(app: &mut App, expected: usize) -> Vec<LibEvent> {
    let mut events = Vec::new();
    for _ in 0..expected {
        match app
            .channels
            .lib_rx
            .recv_timeout(std::time::Duration::from_secs(1))
        {
            Ok(event) => events.push(event),
            Err(error) => panic!(
                "expected {expected} library-fetch events, received {}: {error:?}",
                events.len()
            ),
        }
    }
    events
}

/// Task 3.3: an open-but-Empty receiver is returned to its slot rather than
/// being dropped (the worker is still running; the drain just found no
/// completion yet). Startup and test receivers share the bookkeeping.
#[rstest]
/// Task 3.3: a receiver whose worker exited without a completion is
/// Disconnected; the drain reports work and drives the worker-disconnect
/// handler. With no configured Audiobookshelf setup the resolved state is
/// `NotConfigured`. Startup and test receivers share the handler.
#[rstest]
/// Task 3.3: the setup receiver runs its own disconnect handler (busy=false,
/// form error, state reset to the form's previous state). With no form the
/// reset lands on `NotConfigured`.
#[test]
fn drain_audiobookshelf_events_setup_disconnect_reports_and_resets() {
    let mut app = make_app_stub();
    app.audiobookshelf_runtime.begin_setup();
    let (tx, rx) = std::sync::mpsc::channel::<AudiobookshelfSetupCompletion>();
    drop(tx);
    app.setup.audiobookshelf_setup_rx = Some(rx);

    assert!(
        app.drain_audiobookshelf_events(),
        "a Disconnected setup receiver must report produced"
    );
    assert_eq!(
        app.audiobookshelf_runtime.state,
        ServiceState::NotConfigured,
        "the setup disconnect resets to the form's previous state"
    );
    assert!(
        app.setup.audiobookshelf_setup_rx.is_none(),
        "the disconnected setup receiver is not put back"
    );
}

/// Task 3.3: the catalog receiver has no disconnect handler — a dropped worker
/// leaves the receiver dropped and produces nothing.
/// Task 3.4: a catalog completion whose generation the runtime no longer
/// accepts is dropped without touching browse or catalog state.
/// Task 3.4: an `AuthenticationRejected` catalog failure moves the runtime to
/// `NeedsAuthentication` and clears the stored credential.
#[test]
fn drain_audiobookshelf_events_auth_rejection_needs_authentication_and_clears_credential() {
    let mut app = make_app_stub();
    let generation = app.audiobookshelf_runtime.generation();
    app.audiobookshelf_runtime.state = ServiceState::Connecting;
    app.audiobookshelf_runtime.user = Some(AudiobookshelfUser {
        id: "user-1".into(),
        username: "listener".into(),
    });
    mbv_config::save_service_secret(mbv_queue::ServiceKind::Audiobookshelf, "saved-token")
        .expect("secret is written under the test state dir");
    app.setup.audiobookshelf_catalog_rx = Some(catalog_receiver(
        generation,
        Err(AudiobookshelfError::from_class(
            AudiobookshelfFailureClass::AuthenticationRejected,
        )),
    ));

    assert!(
        app.drain_audiobookshelf_events(),
        "the completion must be reported"
    );
    assert_eq!(
        app.audiobookshelf_runtime.state,
        ServiceState::NeedsAuthentication
    );
    assert!(
        app.audiobookshelf_runtime.user.is_none(),
        "a rejection clears the runtime user"
    );
    assert!(
        mbv_config::load_service_secret(mbv_queue::ServiceKind::Audiobookshelf).is_none(),
        "a rejection clears the saved credential"
    );
}

/// Task 3.4: a non-auth catalog failure leaves browse and catalog state
/// untouched (it only reports the failure to the log).
/// Task 3.5: under the stub config the catalog success path builds browse for
/// every library, routes podcast progress to the podcast map and book progress
/// to the book map, and dispatches the per-kind library fetch: a podcast
/// library requests shows and shelves, a book library requests books.
#[test]
fn drain_audiobookshelf_events_catalog_success_builds_browse_and_dispatches() {
    let mut app = make_app_stub();
    let generation = app.audiobookshelf_runtime.generation();
    let podcast_progress = HashMap::from([(
        ("pod-1".to_string(), "ep-1".to_string()),
        AudiobookshelfProgress {
            library_item_id: "pod-1".into(),
            episode_id: "ep-1".into(),
            current_time_seconds: 12.0,
            is_finished: false,
        },
    )]);
    let book_progress = HashMap::from([(
        "book-1".to_string(),
        AudiobookshelfBookProgress {
            library_item_id: "book-1".into(),
            current_time_seconds: 34.0,
            is_finished: false,
        },
    )]);
    app.setup.audiobookshelf_catalog_rx = Some(catalog_receiver(
        generation,
        Ok((
            vec![
                audiobookshelf_library("pod-1", "podcast"),
                audiobookshelf_library("book-1", "book"),
            ],
            podcast_progress.clone(),
            book_progress.clone(),
        )),
    ));

    assert!(
        app.drain_audiobookshelf_events(),
        "the catalog completion must be reported"
    );

    assert_eq!(app.audiobookshelf_libraries.len(), 2);
    assert_eq!(
        app.audiobookshelf_browse.len(),
        2,
        "podcast browse is built for every library"
    );
    assert_eq!(
        app.audiobookshelf_book_browse.len(),
        2,
        "book browse is built for every library"
    );
    assert_eq!(
        app.audiobookshelf_browse[0].progress, podcast_progress,
        "podcast progress lands in the podcast map"
    );
    assert!(
        app.audiobookshelf_browse[1].progress.is_empty(),
        "book progress must not land in the podcast map"
    );
    assert_eq!(
        app.audiobookshelf_book_browse[1].progress, book_progress,
        "book progress lands in the book map"
    );
    assert!(
        app.audiobookshelf_book_browse[0].progress.is_empty(),
        "podcast progress must not land in the book map"
    );

    assert_catalog_fetches(&mut app, generation);
}

fn assert_catalog_fetches(app: &mut App, generation: mbv_core::service_runtime::SetupGeneration) {
    let mut podcast_shows = 0;
    let mut podcast_shelves = 0;
    let mut book_fetches = 0;
    for event in collect_library_events(app, 3) {
        match event {
            LibEvent::Audiobookshelf(AudiobookshelfEvent::ShowsFetched {
                generation: event_generation,
                library_id,
                ..
            }) => {
                assert_eq!(event_generation, generation);
                assert_eq!(library_id, "pod-1");
                podcast_shows += 1;
            }
            LibEvent::Audiobookshelf(AudiobookshelfEvent::ShelfFetched {
                generation: event_generation,
                library_id,
                ..
            }) => {
                assert_eq!(event_generation, generation);
                assert_eq!(library_id, "pod-1");
                podcast_shelves += 1;
            }
            LibEvent::Audiobookshelf(AudiobookshelfEvent::BooksFetched {
                generation: event_generation,
                library_id,
                ..
            }) => {
                assert_eq!(event_generation, generation);
                assert_eq!(library_id, "book-1");
                book_fetches += 1;
            }
            _ => panic!("unexpected library event for a stub-config Audiobookshelf catalog"),
        }
    }
    assert_eq!(
        (podcast_shows, podcast_shelves, book_fetches),
        (1, 1, 1),
        "a podcast library requests shows and shelves; a book library requests books"
    );
}

// ── Regression: a stale `library_position_state` album cursor must never
// re-anchor the mounted Grouped Music owner. Artist selections never write
// that album position, so a saved Soul level keeps pointing at a Pizzicato
// Five album under a collapsed root; `3d1a0a5df` made programmatic selection
// non-expanding, so re-anchoring onto that hidden album lands on that other
// artist's collapsed root. The owner's local selection is authoritative, and
// startup restores only through `reanchor_launch_state`. ─────────────────

fn music_two_artist_items() -> (Vec<mbv_emby_model::EmbyItem>, Vec<mbv_emby_model::EmbyItem>) {
    let mut group = mbv_emby_model::test_support::make_item("Soul", "MusicArtist");
    group.id = "group-soul".into();
    group.is_folder = true;
    let mut aaliyah = mbv_emby_model::test_support::make_item("Aaliyah", "MusicAlbum");
    aaliyah.id = "album-aaliyah".into();
    aaliyah.artist = "Aaliyah".into();
    aaliyah.is_folder = true;
    let mut pf = mbv_emby_model::test_support::make_item("Pizzicato Five", "MusicAlbum");
    pf.id = "album-pf".into();
    pf.artist = "Pizzicato Five".into();
    pf.is_folder = true;
    (vec![group], vec![aaliyah, pf])
}

fn music_level(
    parent_id: &str,
    title: &str,
    items: Vec<mbv_emby_model::EmbyItem>,
    resting: usize,
) -> crate::app::BrowseLevel {
    crate::app::BrowseLevel {
        rows: ServerRows::complete(0),
        parent_id: parent_id.into(),
        title: title.into(),
        items,

        resting: mbv_ui_model::browse::BrowseResting::new(resting, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}

fn music_two_artist_stale_pf() -> App {
    let mut app = make_app_stub();
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.music_levels = vec!["group".into(), "album".into()];
    let mut library = mbv_emby_model::test_support::make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();
    let (groups, albums) = music_two_artist_items();
    app.libs.push(crate::app::LibraryTab {
        nav_stack: vec![
            music_level("lib-music", "Music", groups, 0),
            music_level("group-soul", "Soul", albums, 1),
        ],
        ..crate::app::LibraryTab::new(library)
    });
    app
}

fn music_launch_state_aaliyah() -> mbv_config::TuiLaunchState {
    mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::ServiceLibrary {
            kind: mbv_queue::ServiceKind::Emby,
            library_id: "lib-music".into(),
        },
        panel_focus: mbv_config::LaunchPanelFocus::Library,
        selector: Some(mbv_config::SelectorIdentity::Emby {
            key: mbv_config::EmbySelectorKey::Group("group-soul".into()),
        }),
        item: Some(mbv_config::LibraryItemIdentity::Emby {
            id: "Aaliyah".into(),
        }),
    }
}

/// Deliver the production stale-position restore: a saved Soul level whose
/// `focused_item_id` is the Pizzicato Five album. `restore_library_position_levels`
/// turns that id into the restored album level's resting cursor, exactly the
/// stale cursor the owner must ignore.
fn deliver_music_stale_pf_restore(model: &mut Model) {
    let (groups, albums) = music_two_artist_items();
    let position = mbv_queue::LibraryPosition {
        levels: vec![
            mbv_queue::LibraryPositionLevel {
                parent_id: "lib-music".into(),
                title: "Music".into(),
                focused_item_id: Some("group-soul".into()),
                cursor_index: 0,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                ..Default::default()
            },
            mbv_queue::LibraryPositionLevel {
                parent_id: "group-soul".into(),
                title: "Soul".into(),
                focused_item_id: Some("album-pf".into()),
                cursor_index: 1,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    model
        .app
        .library_position_state
        .libraries
        .insert("lib-music".into(), position.clone());
    model
        .app
        .channels
        .lib_tx
        .send(LibEvent::Browse(BrowseEvent::RestoreLibraryPosition {
            lib_idx: 0,
            requested_position: position.clone(),
            position,
            nav_stack: vec![
                music_level("lib-music", "Music", groups, 0),
                music_level("group-soul", "Soul", albums, 1),
            ],
        }))
        .expect("lib event channel");
    model.drain_lib_events();
}

#[test]
fn music_launch_state_artist_survives_a_stale_saved_position_restore() {
    let mut app = music_two_artist_stale_pf();
    app.emby_runtime.state = mbv_core::service_runtime::ServiceState::Ready;
    app.launch_restore =
        crate::app::state::app_struct::LaunchRestore::Pending(music_launch_state_aaliyah());
    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    assert_eq!(
        model
            .test_music_owner()
            .selected_artist_launch_id()
            .as_deref(),
        Some("Aaliyah"),
    );

    deliver_music_stale_pf_restore(&mut model);

    assert_eq!(
        model
            .test_music_owner()
            .selected_artist_launch_id()
            .as_deref(),
        Some("Aaliyah"),
        "a stale saved album cursor (bug: Pizzicato Five) must not override the launch-state artist"
    );
}

#[test]
fn music_owner_artist_survives_a_stale_restore_after_a_tab_round_trip() {
    let mut model = Model::new(music_two_artist_stale_pf());
    model.sync_mounted_surfaces();
    assert_eq!(
        model
            .test_music_owner()
            .selected_artist_launch_id()
            .as_deref(),
        Some("Aaliyah"),
    );

    model.app.tab = crate::app::TabSelection::Home;
    model.sync_mounted_surfaces();
    model.app.tab = crate::app::TabSelection::EmbyLibrary(0);
    model.sync_mounted_surfaces();
    assert_eq!(
        model
            .test_music_owner()
            .selected_artist_launch_id()
            .as_deref(),
        Some("Aaliyah"),
        "the retained owner keeps its artist across a tab round trip",
    );

    deliver_music_stale_pf_restore(&mut model);

    assert_eq!(
        model
            .test_music_owner()
            .selected_artist_launch_id()
            .as_deref(),
        Some("Aaliyah"),
        "a stale saved album cursor (bug: Pizzicato Five) must not override the retained owner after a tab round trip"
    );
}
