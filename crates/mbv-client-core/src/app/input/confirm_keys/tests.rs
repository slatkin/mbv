use super::*;
use crate::app::state::playback::{ReplacementExecutor, RoutedReplacementPrep};
use crate::app::tests::*;
use mbv_emby_model::EmbyItem;
use mbv_emby_model::test_support::make_item;
use mbv_ui_msg::KeyModifiers;

/// Design D6: one `PlayItems` payload as the grouped-track resolver produces
/// it, so the replacement tests exercise the same executable shape the tree
/// path submits.
fn play_action(ids: &[&str]) -> PendingQueueAction {
    PendingQueueAction::PlayItems {
        items: ids.iter().map(|id| audio(id)).collect(),
        start_idx: 0,
        source: mbv_queue::QueueSource::Album,
    }
}

fn audio(id: &str) -> EmbyItem {
    let mut item = make_item(id, "Audio");
    item.id = id.into();
    item.media_type = "Audio".into();
    item
}

/// A neutral key with no modifiers (`extract-client-core` task 2.2: confirm
/// keys are neutral, so the tests spell them in the neutral vocabulary).
fn key(code: Key) -> mbv_ui_msg::KeyEvent {
    mbv_ui_msg::KeyEvent::new(code, KeyModifiers::NONE)
}

/// Whether the shared confirm modal is the current pending overlay.
fn confirm_pending(app: &App) -> bool {
    matches!(
        app.pending_overlay,
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(_))
    )
}

/// The pending overlay's confirm action, when a confirm modal is up.
fn pending_confirm_action(app: &App) -> Option<ConfirmAction> {
    match &app.pending_overlay {
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(modal)) => {
            Some(modal.on_confirm.clone())
        }
        _ => None,
    }
}

/// A local-daemon stub whose command channel exposes the owner-bound queue
/// ops, so a replacement's execution is observed on the wire (row 5.3: the
/// Client's view follows the owner's answer, not an optimistic write).
fn gate_app() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    make_local_daemon_app_stub_with_cmd_rx(Vec::new())
}

/// The item ids of the most recent `UnifiedQueueReplace` sent to the owner;
/// drains the channel, so call once per assertion point.
fn sent_replace_ids(cmd_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) -> Vec<String> {
    let mut ids = Vec::new();
    for command in cmd_rx.try_iter() {
        if let mbv_ctrl::CtrlCmd::UnifiedQueueReplace { items, .. } = command {
            ids = items.iter().map(|item| item.id().to_string()).collect();
        }
    }
    ids
}

fn queue_ids(app: &App) -> Vec<String> {
    app.playback_queue()
        .emby_items()
        .iter()
        .map(|item| item.id.clone())
        .collect()
}

/// User request 2026-11-27: every queue replacement executes at once — an
/// empty local target queue included, which never had a prompt.
#[test]
fn empty_local_target_queue_executes_the_replacement_immediately() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = gate_app();
    assert_eq!(app.playback_queue().total_queue_len(), 0);

    app.request_queue_replacement(play_action(&["track-1"]), ReplacementExecutor::Pending);

    assert!(!confirm_pending(&app));
    assert_eq!(
        sent_replace_ids(&cmd_rx),
        ["track-1"],
        "the replacement reached the owner as one answered op"
    );
}

/// User request 2026-11-27: a populated target queue is replaced without a
/// confirmation prompt.
#[test]
fn a_populated_target_queue_is_replaced_without_a_prompt() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = gate_app();
    app.local_view.adopt_items(vec![audio("existing")], 0);

    app.request_queue_replacement(play_action(&["track-1"]), ReplacementExecutor::Pending);

    assert!(
        !confirm_pending(&app),
        "no prompt stands before the replacement"
    );
    assert_eq!(
        sent_replace_ids(&cmd_rx),
        ["track-1"],
        "the replacement reached the owner as one answered op"
    );
}

/// User request 2026-11-27: a populated local stub queue is replaced without
/// a prompt, and the queue view follows the owner's answer (row 5.3).
#[test]
fn a_populated_local_stub_queue_is_replaced_without_a_prompt() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.local_view.adopt_items(vec![audio("existing")], 0);

    app.request_queue_replacement(play_action(&["track-1"]), ReplacementExecutor::Pending);

    assert!(!confirm_pending(&app));
    assert_eq!(queue_ids(&app), ["existing"]);
    assert_eq!(app.playback_queue().cursor(), 0);
}

/// Row 3.4: an empty target queue runs a routed shuffle and a playlist replace
/// immediately.
#[test]
fn empty_queue_runs_a_shuffle_and_a_playlist_replace_without_a_modal() {
    let _guard = crate::app::config::TestStateDirGuard::new();

    let (mut app, cmd_rx) = gate_app();
    assert_eq!(app.playback_queue().total_queue_len(), 0);
    app.request_queue_replacement(
        PendingQueueAction::PlayItems {
            items: vec![audio("shuffle-1")],
            start_idx: 0,
            source: mbv_queue::QueueSource::Shuffle,
        },
        ReplacementExecutor::Routed(RoutedReplacementPrep::ShuffleFolder),
    );
    assert!(!confirm_pending(&app));
    assert_eq!(
        sent_replace_ids(&cmd_rx),
        ["shuffle-1"],
        "the routed shuffle reached the owner as one answered op"
    );

    let (mut app, cmd_rx) = gate_app();
    app.request_queue_replacement(
        PendingQueueAction::PlayItems {
            items: vec![audio("playlist-1")],
            start_idx: 0,
            source: mbv_queue::QueueSource::Playlist {
                id: Some("playlist-1".into()),
                name: "Playlist".into(),
            },
        },
        ReplacementExecutor::PlaylistsSidebar,
    );
    assert!(!confirm_pending(&app));
    assert_eq!(
        sent_replace_ids(&cmd_rx),
        ["playlist-1"],
        "the playlist replace reached the owner as one answered op"
    );
}

/// Row 3.5: a routed replacement whose items the current owner cannot play
/// raises the local fall-through prompt directly — no replace-queue
/// confirmation stands before it.
#[test]
fn a_wholly_unplayable_replacement_raises_the_local_play_prompt() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    // The audio-only owner cannot play a Video item; its queue is populated
    // so the routed executor defers to local play.
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
            source: mbv_queue::QueueSource::Album,
        },
        ReplacementExecutor::Routed(RoutedReplacementPrep::Album),
    );

    assert_eq!(
        pending_confirm_action(&app),
        Some(ConfirmAction::PlayLocallyInstead),
        "the fall-through prompt is the only prompt"
    );
}

/// Row 3.6: an explicit `play_item` on a populated queue is a direct-play
/// path, not a user-initiated replacement, so it never asks.
#[test]
fn play_item_on_a_populated_queue_does_not_raise_the_replace_modal() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = gate_app();
    app.local_view.adopt_items(vec![audio("existing")], 0);

    let mut item = audio("movie-1");
    item.media_type = "Video".into();
    app.play_item(item);

    assert!(!confirm_pending(&app), "play_item is never gated");
    assert_eq!(
        sent_replace_ids(&cmd_rx),
        ["movie-1"],
        "the play reached the owner as one answered op"
    );
}

/// User request 2026-11-27: a context-menu Play selection on a populated
/// queue replaces it without a prompt, and playback state changes as the
/// play executor leaves it.
#[test]
fn a_context_menu_play_selection_replaces_a_populated_queue_without_a_prompt() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = gate_app();
    app.local_view.adopt_items(vec![audio("existing")], 0);

    app.execute_context_action(
        Some(crate::app::ContextAction::PlaySelection(vec![
            audio("track-1"),
            audio("track-2"),
        ])),
        None,
    );

    assert!(!confirm_pending(&app));
    let ids = sent_replace_ids(&cmd_rx);
    assert_eq!(ids.len(), 2, "the whole selection replaced the queue");
    assert!(ids.contains(&"track-1".to_string()));
    assert!(ids.contains(&"track-2".to_string()));
}

/// The Enter answer of the one confirm the clear flow still raises runs the
/// clear through the shared executor, which resets the local queue's dirty
/// flag with the clear.
#[test]
fn the_clear_confirmation_executes_the_clear() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let (mut app, _cmd_rx) = gate_app();
    app.local_view.adopt_items(vec![audio("existing")], 0);
    app.queue_dirty = true;

    app.apply_confirm_action(ConfirmAction::ClearQueue, key(Key::Enter));

    assert!(
        !confirm_pending(&app),
        "the answered clear raises no second prompt"
    );
    assert!(!app.queue_dirty, "the clear resets the dirty flag");
}
