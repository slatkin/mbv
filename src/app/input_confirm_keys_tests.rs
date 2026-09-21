#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::tests::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
        Some(super::super::types_overlay::OverlayRequest::Confirm(_))
    )
}

/// The pending overlay's confirm action, when a confirm modal is up.
fn pending_confirm_action(app: &App) -> Option<ConfirmAction> {
    match &app.pending_overlay {
        Some(super::super::types_overlay::OverlayRequest::Confirm(modal)) => {
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

    app.request_queue_replacement(play_action(&["track-1"]));

    assert!(!confirm_pending(&app), "an empty queue asks nothing");
    assert!(app.pending_queue_action.is_none());
    assert_eq!(queue_ids(&app), ["track-1"]);
}

/// D6: an empty directly-controlled remote target queue also executes
/// immediately, staging the replacement in that owner's canonical queue.
#[test]
fn empty_direct_remote_target_queue_executes_the_replacement_immediately() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(Vec::new(), Vec::new());
    assert!(
        app.has_direct_remote_queue(),
        "the fixture must exercise the directly-controlled owner"
    );
    assert_eq!(app.playback_queue().total_queue_len(), 0);

    app.request_queue_replacement(play_action(&["track-1"]));

    assert!(!confirm_pending(&app));
    assert_eq!(queue_ids(&app), ["track-1"]);
    assert!(
        matches!(app.queue_source, crate::config::QueueSource::Album),
        "the staged replacement carries its own source label"
    );
}

/// D6: a populated local queue stores the complete action and prompts; the
/// queue itself is untouched until the confirmation is answered.
#[test]
fn populated_local_target_queue_stores_the_action_and_prompts() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(vec![audio("existing")], 0);

    app.request_queue_replacement(play_action(&["track-1"]));

    assert_eq!(
        pending_confirm_action(&app),
        Some(ConfirmAction::ReplacePopulatedQueue)
    );
    assert!(
        matches!(
            app.pending_queue_action,
            Some(PendingQueueAction::PlayItems { .. })
        ),
        "the complete payload is stored, not re-derived at confirmation time"
    );
    assert_eq!(queue_ids(&app), ["existing"], "the prompt changes no queue");
    assert_eq!(app.playback_queue().queue_cursor, 0);
}

/// D6: a populated directly-controlled remote queue prompts; the remote
/// canonical queue is not staged before the confirmation.
#[test]
fn populated_direct_remote_target_queue_prompts_before_staging() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(Vec::new(), vec![audio("existing")]);
    assert!(app.has_direct_remote_queue());

    app.request_queue_replacement(play_action(&["track-1"]));

    assert_eq!(
        pending_confirm_action(&app),
        Some(ConfirmAction::ReplacePopulatedQueue)
    );
    assert_eq!(
        queue_ids(&app),
        ["existing"],
        "the remote queue is untouched before the confirmation"
    );

    mount_confirmation(&mut app);
    app.apply_confirm_action(
        ConfirmAction::ReplacePopulatedQueue,
        key(KeyCode::Char('y')),
    );

    assert!(!confirm_pending(&app));
    assert!(app.pending_queue_action.is_none());
    assert_eq!(
        queue_ids(&app),
        ["track-1"],
        "confirmation stages and executes the stored action"
    );
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Album
    ));
}

/// D6: confirming a populated local queue executes the stored action when no
/// saved-playlist protection applies.
#[test]
fn confirming_a_populated_local_queue_executes_the_stored_action() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(vec![audio("existing")], 0);
    app.request_queue_replacement(play_action(&["track-1"]));

    app.apply_confirm_action(
        ConfirmAction::ReplacePopulatedQueue,
        key(KeyCode::Char('y')),
    );

    assert!(app.pending_queue_action.is_none());
    assert_eq!(queue_ids(&app), ["track-1"]);
}

/// D6 cancellation: Esc at the first prompt changes neither queue nor
/// playback and leaves no executable payload behind.
#[test]
fn cancelling_the_replace_queue_prompt_changes_neither_queue_nor_playback() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(vec![audio("existing")], 0);
    app.request_queue_replacement(play_action(&["track-1"]));

    app.apply_confirm_action(ConfirmAction::ReplacePopulatedQueue, key(KeyCode::Esc));

    assert!(
        app.pending_queue_action.is_none(),
        "cancellation clears the stored payload"
    );
    assert_eq!(queue_ids(&app), ["existing"]);
    assert_eq!(app.playback_queue().queue_cursor, 0);
    assert!(!app.player.status.lock().unwrap().active);
}

/// D6 cancellation: a dismiss key at the first prompt also clears the stored
/// payload, so no later pass can fire it.
#[test]
fn dismissing_the_replace_queue_prompt_clears_the_stored_action() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(vec![audio("existing")], 0);
    app.request_queue_replacement(play_action(&["track-1"]));

    app.apply_confirm_action(
        ConfirmAction::ReplacePopulatedQueue,
        key(KeyCode::Char('x')),
    );

    assert!(app.pending_queue_action.is_none());
    assert_eq!(queue_ids(&app), ["existing"]);
}

/// D6 two-step: confirming a populated, dirty saved-playlist queue runs the
/// existing save/discard prompt as a second step before anything executes.
#[test]
fn confirmed_dirty_saved_playlist_replacement_raises_the_save_discard_prompt() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = dirty_saved_playlist_app();
    app.request_queue_replacement(play_action(&["track-1"]));

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
    app.request_queue_replacement(play_action(&["track-1"]));
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
fn saving_the_dirty_playlist_defers_the_stored_replacement() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = dirty_saved_playlist_app();
    app.request_queue_replacement(play_action(&["track-1"]));
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

    assert!(!confirm_pending(&app));
    assert_eq!(queue_ids(&app), ["existing"], "the save defers execution");
    assert!(
        app.pending_queue_action.is_some(),
        "the payload waits for the save completion"
    );
}

/// D6 two-step: cancelling the second prompt changes neither queue nor
/// playback and clears the stored payload.
#[test]
fn cancelling_the_dirty_prompt_changes_neither_queue_nor_playback() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = dirty_saved_playlist_app();
    app.request_queue_replacement(play_action(&["track-1"]));
    mount_confirmation(&mut app);
    app.apply_confirm_action(
        ConfirmAction::ReplacePopulatedQueue,
        key(KeyCode::Char('y')),
    );

    app.apply_confirm_action(ConfirmAction::DiscardOrSaveDirtyPlaylist, key(KeyCode::Esc));

    assert!(app.pending_queue_action.is_none());
    assert_eq!(queue_ids(&app), ["existing"]);
    assert_eq!(app.playback_queue().queue_cursor, 0);
    assert!(!app.player.status.lock().unwrap().active);
}

/// D6 predicate: only a `PlayItems` payload over a non-empty playback-target
/// queue needs the gate; a bare clear owns its own confirmation flow.
#[test]
fn only_play_items_over_a_populated_target_queue_needs_confirmation() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    assert!(!app.queue_replacement_needs_confirmation(&play_action(&["track-1"])));
    assert!(!app.queue_replacement_needs_confirmation(&PendingQueueAction::ClearQueue));

    app.player_tab.set_items(vec![audio("existing")], 0);
    assert!(app.queue_replacement_needs_confirmation(&play_action(&["track-1"])));
    assert!(!app.queue_replacement_needs_confirmation(&PendingQueueAction::ClearQueue));
}
