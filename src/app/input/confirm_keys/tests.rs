use super::*;
use crate::app::state::types::playback::{ReplacementExecutor, RoutedReplacementPrep};
use crate::app::tests::*;
use crossterm::event::KeyModifiers;
use mbv_core::api::EmbyItem;

/// Design D6: one `PlayItems` payload as the grouped-track resolver produces
/// it, so the gate tests exercise the same executable shape the tree path
/// stores.
fn play_action(ids: &[&str]) -> PendingQueueAction {
    PendingQueueAction::PlayItems {
        items: ids.iter().map(|id| audio(id)).collect(),
        start_idx: 0,
        source: crate::config::QueueSource::Album,
        autostart: true,
    }
}

fn audio(id: &str) -> EmbyItem {
    let mut item = make_item(id, "Audio");
    item.id = id.into();
    item.media_type = "Audio".into();
    item
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Whether the shared confirm modal is the current pending overlay.
fn confirm_pending(app: &App) -> bool {
    matches!(
        app.pending_overlay,
        Some(crate::app::state::types::overlay::OverlayRequest::Confirm(
            _
        ))
    )
}

/// The pending overlay's confirm action, when a confirm modal is up.
fn pending_confirm_action(app: &App) -> Option<ConfirmAction> {
    match &app.pending_overlay {
        Some(crate::app::state::types::overlay::OverlayRequest::Confirm(modal)) => {
            Some(modal.on_confirm.clone())
        }
        _ => None,
    }
}

fn queue_ids(app: &App) -> Vec<String> {
    app.playback_queue()
        .emby_items()
        .iter()
        .map(|item| item.id.clone())
        .collect()
}

/// The shell's sync pass takes the pending overlay when it mounts the modal;
/// a direct `apply_confirm_action` caller closes that loop the same way so an
/// answered request never lingers as if it were still live.
fn mount_confirmation(app: &mut App) {
    app.pending_overlay = None;
}

/// A saved-playlist queue with unsaved edits: the second-step protection the
/// confirmed replacement must still run through `replace_queue_or_prompt`.
fn dirty_saved_playlist_app() -> App {
    let mut app = make_app_stub();
    app.player_tab.set_items(vec![audio("existing")], 0);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Saved".into(),
    };
    app.queue_dirty = true;
    app
}

/// D6: an empty local playback-target queue executes the replacement
/// immediately, with no confirmation and no stored payload.
#[test]
fn empty_local_target_queue_executes_the_replacement_immediately() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    assert_eq!(app.playback_queue().total_queue_len(), 0);

    app.request_queue_replacement(play_action(&["track-1"]), ReplacementExecutor::Pending);

    assert!(!confirm_pending(&app), "an empty queue asks nothing");
    assert!(app.pending_queue_replacement.is_none());
    assert_eq!(queue_ids(&app), ["track-1"]);
}

/// D6: an empty directly-controlled remote target queue also executes
/// immediately, staging the replacement in that owner's canonical queue.
#[test]
fn populated_local_target_queue_stores_the_action_and_prompts() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(vec![audio("existing")], 0);

    app.request_queue_replacement(play_action(&["track-1"]), ReplacementExecutor::Pending);

    assert_eq!(
        pending_confirm_action(&app),
        Some(ConfirmAction::ReplacePopulatedQueue)
    );
    assert!(
        matches!(
            app.pending_queue_replacement,
            Some((
                PendingQueueAction::PlayItems { .. },
                ReplacementExecutor::Pending
            ))
        ),
        "the complete payload is stored, not re-derived at confirmation time"
    );
    assert!(
        app.pending_queue_action.is_none(),
        "the gated payload never occupies the save-deferral slot"
    );
    assert_eq!(queue_ids(&app), ["existing"], "the prompt changes no queue");
    assert_eq!(app.playback_queue().queue_cursor, 0);
}

/// D6: a populated directly-controlled remote queue prompts; the remote
/// canonical queue is not staged before the confirmation.
#[test]
fn confirming_a_populated_local_queue_executes_the_stored_action() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(vec![audio("existing")], 0);
    app.request_queue_replacement(play_action(&["track-1"]), ReplacementExecutor::Pending);

    app.apply_confirm_action(
        ConfirmAction::ReplacePopulatedQueue,
        key(KeyCode::Char('y')),
    );

    assert!(app.pending_queue_replacement.is_none());
    assert_eq!(queue_ids(&app), ["track-1"]);
}

/// D6 cancellation: Esc at the first prompt changes neither queue nor
/// playback and leaves no executable payload behind.
#[test]
fn cancelling_the_replace_queue_prompt_changes_neither_queue_nor_playback() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(vec![audio("existing")], 0);
    app.request_queue_replacement(play_action(&["track-1"]), ReplacementExecutor::Pending);

    app.apply_confirm_action(ConfirmAction::ReplacePopulatedQueue, key(KeyCode::Esc));

    assert!(
        app.pending_queue_replacement.is_none(),
        "cancellation clears the stored payload"
    );
    assert!(
        app.pending_queue_action.is_none(),
        "and the save-deferral slot was never touched"
    );
    assert_eq!(queue_ids(&app), ["existing"]);
    assert_eq!(app.playback_queue().queue_cursor, 0);
    assert!(!app.player.status.lock().unwrap().active);
}

/// D6 cancellation: a dismiss key at the first prompt also clears the stored
/// payload, so no later pass can fire it.
#[test]
fn confirmed_dirty_saved_playlist_replacement_raises_the_save_discard_prompt() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = dirty_saved_playlist_app();
    app.request_queue_replacement(play_action(&["track-1"]), ReplacementExecutor::Pending);

    mount_confirmation(&mut app);
    app.apply_confirm_action(
        ConfirmAction::ReplacePopulatedQueue,
        key(KeyCode::Char('y')),
    );

    assert_eq!(
        pending_confirm_action(&app),
        Some(ConfirmAction::DiscardOrSaveDirtyPlaylist),
        "the second step is the existing save/discard prompt"
    );
    assert!(
        app.pending_queue_action.is_some(),
        "the replacement still waits behind the save/discard answer"
    );
    assert_eq!(queue_ids(&app), ["existing"]);
}

/// D6 two-step: discarding the dirty-playlist prompt executes exactly the
/// stored replacement.
#[test]
fn discarding_the_dirty_prompt_executes_the_stored_replacement() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = dirty_saved_playlist_app();
    app.request_queue_replacement(play_action(&["track-1"]), ReplacementExecutor::Pending);
    mount_confirmation(&mut app);
    app.apply_confirm_action(
        ConfirmAction::ReplacePopulatedQueue,
        key(KeyCode::Char('y')),
    );
    mount_confirmation(&mut app);

    app.apply_confirm_action(
        ConfirmAction::DiscardOrSaveDirtyPlaylist,
        key(KeyCode::Char('d')),
    );

    assert!(app.pending_queue_action.is_none());
    assert_eq!(queue_ids(&app), ["track-1"]);
}

/// D6 two-step: saving the dirty playlist defers the replacement until the
/// save completes; the payload survives and the queue is unchanged.
#[test]
fn an_in_flight_save_completion_never_executes_an_unconfirmed_gated_replacement() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = dirty_saved_playlist_app();

    // First activation: confirmed at the populated-queue gate, then deferred
    // behind the dirty-playlist save prompt's `s` answer.
    app.request_queue_replacement(play_action(&["track-1"]), ReplacementExecutor::Pending);
    mount_confirmation(&mut app);
    app.apply_confirm_action(
        ConfirmAction::ReplacePopulatedQueue,
        key(KeyCode::Char('y')),
    );
    mount_confirmation(&mut app);
    app.apply_confirm_action(
        ConfirmAction::DiscardOrSaveDirtyPlaylist,
        key(KeyCode::Char('s')),
    );
    assert_eq!(queue_ids(&app), ["existing"], "the save defers execution");
    assert!(
        app.pending_queue_action.is_some(),
        "the confirmed intent waits for the save boundary"
    );
    let save_mutation_id = app
        .playlist_mutations
        .get("playlist-1")
        .and_then(|state| state.active.as_ref())
        .map(crate::app::state::types::playback::PlaylistMutation::mutation_id)
        .expect("the save is tracked as an in-flight mutation");

    // Second activation while that save is still in flight: it gates on the
    // populated queue instead of overwriting the deferred intent.
    app.request_queue_replacement(play_action(&["track-2"]), ReplacementExecutor::Pending);
    assert_eq!(
        pending_confirm_action(&app),
        Some(ConfirmAction::ReplacePopulatedQueue)
    );
    assert!(app.pending_queue_replacement.is_some());
    assert_eq!(queue_ids(&app), ["existing"]);

    // The save completes with matching lineage and playlist identity.
    app.handle_session_event(crate::app::SessionEvent::PlaylistMutationComplete {
        mutation_id: save_mutation_id,
        playlist_id: "playlist-1".into(),
        origin: crate::app::state::queue_owner::QueueOrigin::ThisProcess {
            epoch: app.queue_epoch,
        },
        source_playlist_id: "playlist-1".into(),
        result: Ok(()),
    });

    assert_eq!(
        queue_ids(&app),
        ["track-1"],
        "the boundary executes the confirmed deferred intent, not the gated one"
    );
    assert!(
        app.pending_queue_action.is_none(),
        "the deferral was consumed"
    );
    assert!(
        app.pending_queue_replacement.is_some(),
        "the unconfirmed payload survives, still awaiting its own confirmation"
    );

    // Only the gated payload's own confirmation executes it: the save
    // completion did not leave the mounted prompt inert.
    mount_confirmation(&mut app);
    app.apply_confirm_action(
        ConfirmAction::ReplacePopulatedQueue,
        key(KeyCode::Char('y')),
    );
    assert_eq!(queue_ids(&app), ["track-2"]);
    assert!(app.pending_queue_replacement.is_none());
}

/// D6 predicate: only a `PlayItems` payload over a non-empty playback-target
/// queue needs the gate; a bare clear owns its own confirmation flow.
#[test]
fn cancelling_context_menu_play_leaves_the_populated_queue_unchanged() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(vec![audio("existing")], 0);

    app.execute_context_action(
        Some(crate::app::ContextAction::PlaySelection(vec![
            audio("track-1"),
            audio("track-2"),
        ])),
        None,
    );

    assert_eq!(
        pending_confirm_action(&app),
        Some(ConfirmAction::ReplacePopulatedQueue)
    );
    assert!(
        matches!(
            app.pending_queue_replacement,
            Some((
                PendingQueueAction::PlayItems { .. },
                ReplacementExecutor::Routed(RoutedReplacementPrep::Selection)
            ))
        ),
        "the selection replacement stores its routed prep"
    );
    assert_eq!(
        queue_ids(&app),
        ["existing"],
        "the selection rebuild is deferred past the gate"
    );
    assert_eq!(app.playback_queue().queue_cursor, 0);

    app.apply_confirm_action(ConfirmAction::ReplacePopulatedQueue, key(KeyCode::Esc));

    assert!(app.pending_queue_replacement.is_none());
    assert_eq!(queue_ids(&app), ["existing"]);
    assert_eq!(app.playback_queue().queue_cursor, 0);
    assert!(!app.player.status.lock().unwrap().active);
}

/// Row 3.4: an empty target queue runs a routed shuffle and a playlist load
/// immediately, with no confirmation and no stored payload.
#[test]
fn empty_queue_runs_a_shuffle_and_a_playlist_load_without_a_modal() {
    let _guard = crate::config::TestStateDirGuard::new();

    let mut app = make_app_stub();
    assert_eq!(app.playback_queue().total_queue_len(), 0);
    app.request_queue_replacement(
        PendingQueueAction::PlayItems {
            items: vec![audio("shuffle-1")],
            start_idx: 0,
            source: crate::config::QueueSource::Shuffle,
            autostart: true,
        },
        ReplacementExecutor::Routed(RoutedReplacementPrep::ShuffleFolder),
    );
    assert!(!confirm_pending(&app), "an empty queue asks nothing");
    assert!(app.pending_queue_replacement.is_none());
    assert_eq!(queue_ids(&app), ["shuffle-1"]);

    let mut app = make_app_stub();
    app.request_queue_replacement(
        PendingQueueAction::PlayItems {
            items: vec![audio("playlist-1")],
            start_idx: 0,
            source: crate::config::QueueSource::Playlist {
                id: Some("playlist-1".into()),
                name: "Playlist".into(),
            },
            autostart: false,
        },
        ReplacementExecutor::Pending,
    );
    assert!(!confirm_pending(&app), "an empty queue asks nothing");
    assert!(app.pending_queue_replacement.is_none());
    assert_eq!(queue_ids(&app), ["playlist-1"]);
}

/// Row 3.5 / design D5: a routed replacement whose items the current owner
/// cannot play asks the populated-queue question first; only after that
/// confirmation does `play_items_routed` raise the local fall-through prompt.
#[test]
fn confirming_a_wholly_unplayable_replacement_then_raises_the_local_play_prompt() {
    let _guard = crate::config::TestStateDirGuard::new();
    // The audio-only owner cannot play a Video item; its queue is populated so
    // the gate must ask before the routed executor can defer to local play.
    let (mut app, _cmd_rx) =
        make_audio_only_remote_app_stub_with_cmd_rx(Vec::new(), vec![audio("existing")]);
    assert_eq!(queue_ids(&app), ["existing"]);

    let mut video = make_item("Movie", "Movie");
    video.id = "video-1".into();
    video.media_type = "Video".into();
    app.request_queue_replacement(
        PendingQueueAction::PlayItems {
            items: vec![video],
            start_idx: 0,
            source: crate::config::QueueSource::Album,
            autostart: true,
        },
        ReplacementExecutor::Routed(RoutedReplacementPrep::Album),
    );

    assert_eq!(
        pending_confirm_action(&app),
        Some(ConfirmAction::ReplacePopulatedQueue)
    );
    assert_eq!(queue_ids(&app), ["existing"], "the gate changes no queue");

    mount_confirmation(&mut app);
    app.apply_confirm_action(
        ConfirmAction::ReplacePopulatedQueue,
        key(KeyCode::Char('y')),
    );

    assert_eq!(
        pending_confirm_action(&app),
        Some(ConfirmAction::PlayLocallyInstead),
        "the fall-through prompt is the second step"
    );
    assert!(app.pending_queue_replacement.is_none());
}

/// Row 3.6: an explicit `play_item` on a populated queue is a direct-play
/// path, not a user-initiated replacement, so it never asks.
#[test]
fn play_item_on_a_populated_queue_does_not_raise_the_replace_modal() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(vec![audio("existing")], 0);

    let mut item = audio("movie-1");
    item.media_type = "Video".into();
    app.play_item(item);

    assert!(!confirm_pending(&app), "play_item is never gated");
    assert!(app.pending_queue_replacement.is_none());
    assert_eq!(queue_ids(&app), ["movie-1"]);
}
