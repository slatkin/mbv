use super::lifecycle_launch_restore::{assert_expired_launch_restores_saved_focus, launch_state};
use super::*;
use crate::app::dispatch::session::service_startup::{
    AudiobookshelfCatalogReceiver, AudiobookshelfCompletion, AudiobookshelfCompletionKind,
};
use crate::app::tests::render_fixtures::catalog_receiver;
use mbv_audiobookshelf::{AudiobookshelfError, AudiobookshelfFailureClass};
use std::collections::HashMap;

fn pending_audiobookshelf_launch() -> App {
    let mut app = crate::app::tests::make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.launch_restore = crate::app::state::app_struct::LaunchRestore::Pending(launch_state(
        ServiceKind::Audiobookshelf,
        "abs-books",
        mbv_config::LaunchPanelFocus::Queue,
    ));
    app
}

/// #810: validation failure expires the snapshot before catalog startup exists.
#[test]
fn audiobookshelf_validation_error_expires_launch_and_restores_saved_focus() {
    let mut app = pending_audiobookshelf_launch();
    let generation = app.audiobookshelf_runtime.generation();

    app.apply_audiobookshelf_completion(AudiobookshelfCompletion {
        generation,
        kind: AudiobookshelfCompletionKind::Startup,
        result: Err(AudiobookshelfError::from_class(
            AudiobookshelfFailureClass::Unavailable,
        )),
    });

    assert_expired_launch_restores_saved_focus(app);
}

/// #810: non-auth catalog failure expires the launch snapshot too.
#[test]
fn audiobookshelf_catalog_error_expires_launch_and_restores_saved_focus() {
    let mut app = pending_audiobookshelf_launch();
    let generation = app.audiobookshelf_runtime.generation();
    app.setup.audiobookshelf_catalog_rx = Some(catalog_receiver(
        generation,
        Err(AudiobookshelfError::from_class(
            AudiobookshelfFailureClass::Unavailable,
        )),
    ));

    assert!(app.drain_audiobookshelf_events());
    assert_expired_launch_restores_saved_focus(app);
}

/// #810: validation-worker disconnect expires only its accepted generation.
#[test]
fn audiobookshelf_startup_worker_disconnect_expires_launch_and_restores_saved_focus() {
    let mut app = pending_audiobookshelf_launch();
    let generation = app.audiobookshelf_runtime.generation();

    app.handle_audiobookshelf_worker_disconnect(generation);

    assert_expired_launch_restores_saved_focus(app);
}

/// #810: a current catalog-worker disconnect settles Home and applies saved focus.
#[test]
fn audiobookshelf_catalog_worker_disconnect_expires_launch_and_restores_saved_focus() {
    let mut app = pending_audiobookshelf_launch();
    let generation = app.audiobookshelf_runtime.generation();
    let (tx, rx) = std::sync::mpsc::channel();
    drop(tx);
    app.setup.audiobookshelf_catalog_rx = Some(AudiobookshelfCatalogReceiver { generation, rx });

    assert!(app.drain_audiobookshelf_events());
    assert_expired_launch_restores_saved_focus(app);
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

/// #810: after a catalog disconnect expires the snapshot, later success cannot move the tab.
#[test]
fn audiobookshelf_catalog_disconnect_then_success_keeps_tab_unchanged() {
    let mut app = pending_audiobookshelf_launch();
    let generation = app.audiobookshelf_runtime.generation();
    let (tx, rx) = std::sync::mpsc::channel();
    drop(tx);
    app.setup.audiobookshelf_catalog_rx = Some(AudiobookshelfCatalogReceiver { generation, rx });

    assert!(app.drain_audiobookshelf_events());
    assert_eq!(app.tab, TabSelection::Home);

    app.setup.audiobookshelf_catalog_rx = Some(catalog_receiver(
        generation,
        Ok((
            vec![mbv_audiobookshelf::AudiobookshelfLibrary {
                id: "abs-books".into(),
                name: "Books".into(),
                media_type: "book".into(),
            }],
            HashMap::new(),
            HashMap::new(),
        )),
    ));
    assert!(app.drain_audiobookshelf_events());
    assert_eq!(app.tab, TabSelection::Home);
}
