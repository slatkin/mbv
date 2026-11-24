use super::lifecycle_launch_restore::{assert_expired_launch_restores_saved_focus, launch_state};
use super::*;
use mbv_audiobookshelf::{AudiobookshelfError, AudiobookshelfFailureClass};

fn pending_audiobookshelf_launch() -> App {
    let mut app = crate::app::tests::make_app_stub();
    app.set_panel_focus_for_test(PanelFocus::Library);
    // The saved focus is Queue, so the displayed queue must not be empty:
    // with the empty-queue column hidden (change `hide-empty-queue-column`,
    // D2) a sync moves the stored focus to Library and the expiry restore
    // would be asserted against a state that no longer holds.
    app.seed_local_queue_emby_items_for_test(
        vec![mbv_emby_model::test_support::make_item(
            "Queue Item",
            "Movie",
        )],
        0,
    );
    app.seed_launch_restore_for_test(crate::app::state::app_struct::LaunchRestore::Pending(
        launch_state(
            ServiceKind::Audiobookshelf,
            "abs-books",
            mbv_config::LaunchPanelFocus::Queue,
        ),
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
        app.apply_audiobookshelf_result_for_test(Err(AudiobookshelfError::from_class(
            AudiobookshelfFailureClass::Unavailable,
        )));
    });
    // Catalog Err.
    expire(|app| {
        let generation = app.audiobookshelf_setup_generation_for_test();
        app.seed_audiobookshelf_catalog_result_for_test(
            generation,
            Err(AudiobookshelfError::from_class(
                AudiobookshelfFailureClass::Unavailable,
            )),
        );
        assert!(app.drain_audiobookshelf_events_for_test());
    });
    // Validation-worker disconnect.
    expire(|app| {
        let generation = app.audiobookshelf_setup_generation_for_test();
        app.report_audiobookshelf_worker_disconnect_for_test(generation);
    });
    // Catalog-worker disconnect.
    expire(|app| {
        let generation = app.audiobookshelf_setup_generation_for_test();
        app.seed_audiobookshelf_catalog_worker_gone_for_test(generation);
        assert!(app.drain_audiobookshelf_events_for_test());
    });
}

/// #810: a stale catalog-worker disconnect cannot settle or move the launch.
#[test]
fn stale_audiobookshelf_catalog_worker_disconnect_keeps_launch_pending() {
    let mut app = pending_audiobookshelf_launch();
    let stale_generation = app.audiobookshelf_setup_generation_for_test();
    app.begin_audiobookshelf_validation_for_test();
    app.seed_audiobookshelf_catalog_worker_gone_for_test(stale_generation);

    assert!(app.drain_audiobookshelf_events_for_test());

    assert_eq!(app.tab_for_test(), TabSelection::Home);
    assert!(matches!(
        app.launch_restore_for_test(),
        crate::app::state::app_struct::LaunchRestore::Pending(_)
    ));
}
