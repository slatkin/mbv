use super::lifecycle_launch_restore::{assert_expired_launch_restores_saved_focus, launch_state};
use super::*;
use crate::app::dispatch::session::service_startup::{
    AudiobookshelfCatalogReceiver, AudiobookshelfCompletion, AudiobookshelfCompletionKind,
};
use crate::app::tests::render_fixtures::catalog_receiver;
use mbv_audiobookshelf::{AudiobookshelfError, AudiobookshelfFailureClass};

fn pending_audiobookshelf_launch() -> App {
    let mut app = crate::app::tests::make_app_stub();
    app.panel_focus = PanelFocus::Library;
    // The saved focus is Queue, so the displayed queue must not be empty:
    // with the empty-queue column hidden (change `hide-empty-queue-column`,
    // D2) a sync moves the stored focus to Library and the expiry restore
    // would be asserted against a state that no longer holds.
    app.local_view.adopt_items(
        vec![mbv_emby_model::test_support::make_item(
            "Queue Item",
            "Movie",
        )],
        0,
    );
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::Pending(launch_state(
        ServiceKind::Audiobookshelf,
        "abs-books",
        mbv_config::LaunchPanelFocus::Queue,
    ));
    app
}

/// #859: every Audiobookshelf Service failure outcome expires the pending
/// launch the same way — Home tab, settled restore, saved focus on sync.
#[test]
fn audiobookshelf_service_outcome_expiry_restores_saved_focus() {
    fn expire(trigger: impl FnOnce(&mut App)) {
        let mut app = pending_audiobookshelf_launch();
        trigger(&mut app);
        assert_expired_launch_restores_saved_focus(app);
    }
    // Startup (validation) Err.
    expire(|app| {
        let generation = app.audiobookshelf_runtime.generation();
        app.apply_audiobookshelf_completion(AudiobookshelfCompletion {
            generation,
            kind: AudiobookshelfCompletionKind::Startup,
            result: Err(AudiobookshelfError::from_class(
                AudiobookshelfFailureClass::Unavailable,
            )),
        });
    });
    // Catalog Err.
    expire(|app| {
        let generation = app.audiobookshelf_runtime.generation();
        app.setup.audiobookshelf_catalog_rx = Some(catalog_receiver(
            generation,
            Err(AudiobookshelfError::from_class(
                AudiobookshelfFailureClass::Unavailable,
            )),
        ));
        assert!(app.drain_audiobookshelf_events());
    });
    // Validation-worker disconnect.
    expire(|app| {
        let generation = app.audiobookshelf_runtime.generation();
        app.handle_audiobookshelf_worker_disconnect(generation);
    });
    // Catalog-worker disconnect.
    expire(|app| {
        let generation = app.audiobookshelf_runtime.generation();
        let (tx, rx) = std::sync::mpsc::channel();
        drop(tx);
        app.setup.audiobookshelf_catalog_rx =
            Some(AudiobookshelfCatalogReceiver { generation, rx });
        assert!(app.drain_audiobookshelf_events());
    });
}

/// #810: a stale catalog-worker disconnect cannot settle or move the launch.
#[test]
fn stale_audiobookshelf_catalog_worker_disconnect_keeps_launch_pending() {
    let mut app = pending_audiobookshelf_launch();
    let stale_generation = app.audiobookshelf_runtime.generation();
    app.audiobookshelf_runtime.begin_validation();
    let (tx, rx) = std::sync::mpsc::channel();
    drop(tx);
    app.setup.audiobookshelf_catalog_rx = Some(AudiobookshelfCatalogReceiver {
        generation: stale_generation,
        rx,
    });

    assert!(app.drain_audiobookshelf_events());

    assert_eq!(app.tab, TabSelection::Home);
    assert!(matches!(
        app.launch_restore,
        crate::app::state::app_struct::LaunchRestore::Pending(_)
    ));
}
