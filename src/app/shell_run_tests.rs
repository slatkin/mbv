use super::*;
use crate::app::images::{series_image_cache_key, CachedImage};
use crate::app::render::components::hero_model::SERIES_LANDSCAPE_IMAGE_TYPES;
use crate::app::render::make_movie_app;
use crate::app::service_startup::{
    AudiobookshelfCatalogCompletion, AudiobookshelfCatalogReceiver, AudiobookshelfSetupCompletion,
    AudiobookshelfStartupReceiver,
};
use crate::app::tests::{make_app_stub, make_session};
use crate::app::types_events::LibEvent;
use crate::app::SessionEvent;
use mbv_core::audiobookshelf::{
    AudiobookshelfBookProgress, AudiobookshelfError, AudiobookshelfFailureClass,
    AudiobookshelfLibrary, AudiobookshelfProgress, AudiobookshelfUser,
};
use mbv_core::service_runtime::{ServiceState, SetupGeneration};
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

/// The fixture has no Emby client, so `spawn_image_fetch` resolves its own
/// request synchronously into `card_image_rx`. A step that must start from a
/// quiet channel drops those completions first.
fn drop_pending_image_completions(model: &mut Model) {
    while model.app.card_image_rx.try_recv().is_ok() {}
}

/// Task 2.3: a Series completion is what re-projects the wide workspace, so the
/// cached Thumb-first entry replaces the placeholder. The gate covers the
/// `:ser:` family the painter builds its keys under.
#[test]
fn series_image_completion_repushes_tv_workspace_content() {
    let mut model = mounted_wide_tv_model();
    model.app.image_protocol_enabled = true;
    // The sync pass is the production projection seam (task 5.10's central
    // hero projection owns the fetch for every migrated owner, TV included
    // since task 8.4). One throwaway draw publishes `root_frame` first — the
    // projection reads the panel's `RootFrame` placement, so it runs on the
    // sync after the shell's startup draw.
    {
        let backend = ratatui::backend::TestBackend::new(160, 40);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    }
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
        model.app.card_image_states.contains_key(&painted_key),
        "the painted Series key must be cached"
    );
    // The fixture's fetch resolves to an empty cache entry (no pixel
    // protocol is available), so the shared producer now treats the
    // placeholder as final while the shell still re-projects the cached key.
    assert!(model.app.card_image_states.contains_key(&painted_key));
}

/// Task 2.3: no other image namespace may drive the TV projection. The cached
/// Series entry is planted without a re-push, so only a gate that wrongly
/// matches this key can clear the placeholder the component still holds.
#[test]
fn non_series_image_completion_leaves_tv_projection_alone() {
    let mut model = mounted_wide_tv_model();
    model.app.image_protocol_enabled = true;
    {
        let backend = ratatui::backend::TestBackend::new(160, 40);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    }
    model.sync_mounted_surfaces();
    drop_pending_image_completions(&mut model);
    assert!(
        wide_tv_shows_placeholder(&mut model),
        "the uncached projection must paint the placeholder"
    );

    model.app.card_image_states.insert(
        series_image_cache_key("movie-focused", SERIES_LANDSCAPE_IMAGE_TYPES),
        CachedImage::empty(),
    );
    model
        .app
        .card_image_tx
        .send(("movie-focused:P".into(), None))
        .expect("image completion channel");

    assert!(
        model.drain_card_image_completions(),
        "the card completion must be drained"
    );
    assert!(
        model.app.card_image_states.contains_key("movie-focused:P"),
        "the drained entry must reach the cache"
    );
    assert!(
        wide_tv_shows_placeholder(&mut model),
        "a non-Series completion must leave the TV projection alone"
    );
}

/// Task 3.1: `clear:yes` dismisses the confirmation modal and routes the
/// clear-queue action, and the drain reports that it produced work.
#[test]
fn drain_notif_actions_clear_yes_dismisses_and_clears_queue() {
    let mut app = make_app_stub();
    app.notif_action_tx
        .send("clear:yes".into())
        .expect("notif channel");

    assert!(
        app.drain_notif_actions(),
        "a queued action must report produced=true"
    );
    assert!(
        matches!(
            app.pending_overlay,
            Some(crate::app::types_overlay::OverlayRequest::DismissConfirm)
        ),
        "clear:yes must dismiss the confirmation modal"
    );
    assert_eq!(
        app.status, "Queue cleared",
        "clear:yes must route the clear-queue action"
    );
}

/// Task 3.1: `__notif_failed__` raises the notification-failure flag.
#[test]
fn drain_notif_actions_failed_sets_the_failure_flag() {
    let mut app = make_app_stub();
    app.notif_action_tx
        .send("__notif_failed__".into())
        .expect("notif channel");

    assert!(
        app.drain_notif_actions(),
        "a queued action must report produced=true"
    );
    assert!(app.notif_failed, "__notif_failed__ must set the flag");
}

/// Task 3.1: payloads without a retained action -- the explicit no-ops and an
/// empty channel -- change no state; `produced` reflects only whether a
/// message was received, so an unrecognised payload still reports `true`
/// while the empty channel reports `false`.
#[rstest]
#[case(Some(""), true)]
#[case(Some("ignore"), true)]
#[case(Some("cancel"), true)]
#[case(Some("unrecognised"), true)]
#[case(None, false)]
fn drain_notif_actions_without_a_known_action_produce_nothing(
    #[case] payload: Option<&str>,
    #[case] expected_produced: bool,
) {
    let mut app = make_app_stub();
    if let Some(payload) = payload {
        app.notif_action_tx
            .send(payload.into())
            .expect("notif channel");
    }

    assert_eq!(
        app.drain_notif_actions(),
        expected_produced,
        "produced must reflect whether a message was received"
    );
    assert!(!app.notif_failed, "no failure flag may be raised");
    assert!(app.pending_overlay.is_none(), "no overlay may be requested");
    assert!(app.status.is_empty(), "no toast may be raised");
}

/// Task 3.2: a queued `SessionEvent` is dispatched to `handle_session_event`
/// and the drain reports that it produced work.
#[test]
fn drain_session_events_dispatches_a_queued_event() {
    let mut app = make_app_stub();
    app.sessions_loading = true;
    app.sessions_tx
        .send(SessionEvent::Loaded {
            sessions: vec![make_session("living-room", "mbv")],
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
#[test]
fn drain_session_events_empty_channel_produces_nothing() {
    let mut app = make_app_stub();
    assert!(
        !app.drain_session_events(),
        "an empty channel must report produced=false"
    );
}

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
/// The timeout only bounds a regression that fails to send; it is not a
/// synchronization sleep.
fn collect_library_events(app: &mut App, expected: usize) -> Vec<LibEvent> {
    let mut events = Vec::new();
    for _ in 0..expected {
        match app.lib_rx.recv_timeout(std::time::Duration::from_secs(5)) {
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
#[case(false)]
#[case(true)]
fn drain_audiobookshelf_events_puts_an_empty_receiver_back(#[case] is_test: bool) {
    let mut app = make_app_stub();
    let generation = app.audiobookshelf_runtime.generation();
    let (tx, rx) = std::sync::mpsc::channel();
    let receiver = AudiobookshelfStartupReceiver { generation, rx };
    if is_test {
        app.audiobookshelf_test_rx = Some(receiver);
    } else {
        app.audiobookshelf_startup_rx = Some(receiver);
    }

    assert!(
        !app.drain_audiobookshelf_events(),
        "an Empty channel must not report produced"
    );
    let put_back = if is_test {
        app.audiobookshelf_test_rx.is_some()
    } else {
        app.audiobookshelf_startup_rx.is_some()
    };
    assert!(put_back, "an Empty receiver must be put back in place");
    drop(tx);
}

/// Task 3.3: a receiver whose worker exited without a completion is
/// Disconnected; the drain reports work and drives the worker-disconnect
/// handler. With no configured Audiobookshelf setup the resolved state is
/// NotConfigured. Startup and test receivers share the handler.
#[rstest]
#[case(false)]
#[case(true)]
fn drain_audiobookshelf_events_disconnected_receiver_drives_worker_disconnect(
    #[case] is_test: bool,
) {
    let mut app = make_app_stub();
    let generation = app.audiobookshelf_runtime.begin_setup();
    let (tx, rx) = std::sync::mpsc::channel();
    drop(tx);
    let receiver = AudiobookshelfStartupReceiver { generation, rx };
    if is_test {
        app.audiobookshelf_test_rx = Some(receiver);
    } else {
        app.audiobookshelf_startup_rx = Some(receiver);
    }

    assert!(
        app.drain_audiobookshelf_events(),
        "a Disconnected receiver must report produced"
    );
    assert_eq!(
        app.audiobookshelf_runtime.state,
        ServiceState::NotConfigured,
        "a disconnect with no setup resolves NotConfigured"
    );
    let still_held = if is_test {
        app.audiobookshelf_test_rx.is_some()
    } else {
        app.audiobookshelf_startup_rx.is_some()
    };
    assert!(!still_held, "a consumed receiver is not put back");
}

/// Task 3.3: the setup receiver runs its own disconnect handler (busy=false,
/// form error, state reset to the form's previous state). With no form the
/// reset lands on NotConfigured.
#[test]
fn drain_audiobookshelf_events_setup_disconnect_reports_and_resets() {
    let mut app = make_app_stub();
    app.audiobookshelf_runtime.begin_setup();
    let (tx, rx) = std::sync::mpsc::channel::<AudiobookshelfSetupCompletion>();
    drop(tx);
    app.audiobookshelf_setup_rx = Some(rx);

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
        app.audiobookshelf_setup_rx.is_none(),
        "the disconnected setup receiver is not put back"
    );
}

/// Task 3.3: the catalog receiver has no disconnect handler — a dropped worker
/// leaves the receiver dropped and produces nothing.
#[test]
fn drain_audiobookshelf_events_catalog_disconnect_is_dropped() {
    let mut app = make_app_stub();
    let (tx, rx) = std::sync::mpsc::channel::<AudiobookshelfCatalogCompletion>();
    drop(tx);
    app.audiobookshelf_catalog_rx = Some(AudiobookshelfCatalogReceiver { rx });

    assert!(
        !app.drain_audiobookshelf_events(),
        "a dropped catalog worker produces nothing"
    );
    assert!(
        app.audiobookshelf_catalog_rx.is_none(),
        "the disconnected catalog receiver is dropped, not restored"
    );
}

/// Task 3.4: a catalog completion whose generation the runtime no longer
/// accepts is dropped without touching browse or catalog state.
#[test]
fn drain_audiobookshelf_events_drops_a_stale_catalog_completion() {
    let mut app = make_app_stub();
    let stale = SetupGeneration::new(app.audiobookshelf_runtime.generation().value() + 1);
    app.audiobookshelf_catalog_rx = Some(catalog_receiver(
        stale,
        Ok((
            vec![audiobookshelf_library("pod-1", "podcast")],
            HashMap::new(),
            HashMap::new(),
        )),
    ));

    assert!(
        !app.drain_audiobookshelf_events(),
        "a stale completion must not be reported as produced"
    );
    assert!(app.audiobookshelf_libraries.is_empty());
    assert!(app.audiobookshelf_browse.is_empty());
    assert!(app.audiobookshelf_book_browse.is_empty());
    assert!(
        !app.audiobookshelf_catalog_ready,
        "a stale completion must not mark the catalog ready"
    );
}

/// Task 3.4: an AuthenticationRejected catalog failure moves the runtime to
/// NeedsAuthentication and clears the stored credential.
#[test]
fn drain_audiobookshelf_events_auth_rejection_needs_authentication_and_clears_credential() {
    let mut app = make_app_stub();
    let generation = app.audiobookshelf_runtime.generation();
    app.audiobookshelf_runtime.state = ServiceState::Connecting;
    app.audiobookshelf_runtime.user = Some(AudiobookshelfUser {
        id: "user-1".into(),
        username: "listener".into(),
    });
    mbv_core::config::save_service_secret(
        mbv_core::config::ServiceKind::Audiobookshelf,
        "saved-token",
    )
    .expect("secret is written under the test state dir");
    app.audiobookshelf_catalog_rx = Some(catalog_receiver(
        generation,
        Err(AudiobookshelfError {
            class: AudiobookshelfFailureClass::AuthenticationRejected,
        }),
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
        mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
            .is_none(),
        "a rejection clears the saved credential"
    );
}

/// Task 3.4: a non-auth catalog failure leaves browse and catalog state
/// untouched (it only reports the failure to the log).
#[test]
fn drain_audiobookshelf_events_generic_catalog_error_leaves_browse_untouched() {
    let mut app = make_app_stub();
    let generation = app.audiobookshelf_runtime.generation();
    app.audiobookshelf_catalog_rx = Some(catalog_receiver(
        generation,
        Err(AudiobookshelfError {
            class: AudiobookshelfFailureClass::Unavailable,
        }),
    ));

    assert!(
        app.drain_audiobookshelf_events(),
        "the completion must be reported"
    );
    assert!(app.audiobookshelf_libraries.is_empty());
    assert!(app.audiobookshelf_browse.is_empty());
    assert!(app.audiobookshelf_book_browse.is_empty());
    assert!(!app.audiobookshelf_catalog_ready);
}

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
    app.audiobookshelf_catalog_rx = Some(catalog_receiver(
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

    assert!(app.audiobookshelf_catalog_ready);
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

    let events = collect_library_events(&mut app, 3);
    let mut podcast_shows = 0;
    let mut podcast_shelves = 0;
    let mut book_fetches = 0;
    for event in &events {
        match event {
            LibEvent::AudiobookshelfShowsFetched {
                generation: event_generation,
                library_id,
                ..
            } => {
                assert_eq!(*event_generation, generation);
                assert_eq!(library_id, "pod-1");
                podcast_shows += 1;
            }
            LibEvent::AudiobookshelfShelfFetched {
                generation: event_generation,
                library_id,
                ..
            } => {
                assert_eq!(*event_generation, generation);
                assert_eq!(library_id, "pod-1");
                podcast_shelves += 1;
            }
            LibEvent::AudiobookshelfBooksFetched {
                generation: event_generation,
                library_id,
                ..
            } => {
                assert_eq!(*event_generation, generation);
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
