//! An accepted Audiobookshelf mark applies locally (audiobookshelf-played-state
//! spec: "An accepted mark updates local progress", "The actively owned session
//! is not modified by a mark", "A failed mark changes nothing locally").
//! Standard-media-context-menus tasks 7.3-7.4: the completions are fed to the
//! drain directly, with no network and no worker thread.

use super::*;
use crate::app::dispatch::session::service_startup::{
    AudiobookshelfMarkCompletion, AudiobookshelfMarkReceiver,
};
use mbv_audiobookshelf::{AudiobookshelfError, AudiobookshelfFailureClass};
use mbv_queue::{AudiobookshelfItem, AudiobookshelfQueueItem, QueueItem};
use mbv_ui_model::context_menu::AudiobookshelfMenuTarget;

/// An app with the podcast catalog loaded and a stubbed local Player owner
/// whose queue commands the test can observe.
fn mark_ready_app() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let mut app = super::podcast::audiobookshelf_app();
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_answered_queue_ops_with_command_rx(Vec::new(), 0);
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);
    app.player_rx = player_rx;
    (app, cmd_rx)
}

fn episode_target(episode_id: &str) -> AudiobookshelfMenuTarget {
    AudiobookshelfMenuTarget::Episode {
        library_item_id: "show-a".into(),
        episode_id: episode_id.into(),
    }
}

fn episode_queue_item(episode_id: &str) -> QueueItem {
    QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(AudiobookshelfQueueItem {
        library_item_id: "show-a".into(),
        episode_id: episode_id.into(),
        title: format!("Episode {episode_id}"),
        show_title: None,
        author: None,
        description: None,
        duration_ticks: None,
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    }))
}

/// Install a mark completion for the drain to pick up, as the worker thread
/// would deliver it.
fn feed_mark_completion(
    app: &mut App,
    generation: mbv_core::service_runtime::SetupGeneration,
    targets: Vec<AudiobookshelfMenuTarget>,
    finished: bool,
    result: Result<(), AudiobookshelfError>,
) {
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(AudiobookshelfMarkCompletion {
        generation,
        targets,
        finished,
        result,
    })
    .expect("the receiver is held by the app");
    app.setup.audiobookshelf_mark_rx = Some(AudiobookshelfMarkReceiver { generation, rx });
}

fn seed_episode_progress(
    app: &mut App,
    episode_id: &str,
    current_time_seconds: f64,
    is_finished: bool,
) {
    app.audiobookshelf_browse[0].progress.insert(
        ("show-a".into(), episode_id.into()),
        mbv_audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: episode_id.into(),
            current_time_seconds,
            is_finished,
        },
    );
}

/// A finished episode marked unplayed resets to unplayed at 0, in browse and
/// in the queue progress sent to the local Player owner.
#[test]
fn mark_unplayed_completion_resets_a_finished_episode_to_unplayed_at_zero() {
    let (mut app, cmd_rx) = mark_ready_app();
    seed_episode_progress(&mut app, "episode-a", 2400.0, true);
    // episode-a is an inactive queue slot; the active slot is another item.
    app.local_view.adopt_queue_items_with_active(
        vec![
            episode_queue_item("episode-a"),
            episode_queue_item("episode-b"),
        ],
        0,
        1,
    );
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 1;
    });

    let generation = app.audiobookshelf_runtime.generation();
    feed_mark_completion(
        &mut app,
        generation,
        vec![episode_target("episode-a")],
        false,
        Ok(()),
    );
    assert!(
        app.drain_audiobookshelf_events(),
        "an accepted completion must be reported"
    );

    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert!(!progress.is_finished, "the episode resets to unplayed");
    assert_eq!(
        progress.current_time_seconds, 0.0,
        "the finished position resets to zero"
    );
    assert!(
        matches!(cmd_rx.try_recv(), Ok(mbv_ctrl::CtrlCmd::UnifiedQueueApplyProgress { updates, .. })
        if updates == vec![mbv_ctrl::ProgressUpdate {
            content_id: mbv_queue::QueueItemContentId::Audiobookshelf {
                library_item_id: "show-a".into(),
                episode_id: "episode-a".into(),
            },
            position_ticks: 0,
            finished: false,
        }]),
        "the inactive slot's progress resets through the local owner"
    );
}

/// A bulk Mark Unplayed resets only the targets cached as finished; an
/// in-progress episode keeps its saved position and reaches no queue update.
#[test]
fn bulk_mark_unplayed_leaves_an_in_progress_episode_position_alone() {
    let (mut app, cmd_rx) = mark_ready_app();
    seed_episode_progress(&mut app, "episode-a", 2400.0, true);
    seed_episode_progress(&mut app, "episode-b", 600.0, false);

    let generation = app.audiobookshelf_runtime.generation();
    feed_mark_completion(
        &mut app,
        generation,
        vec![episode_target("episode-a"), episode_target("episode-b")],
        false,
        Ok(()),
    );
    assert!(app.drain_audiobookshelf_events());

    let progress_b = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-b".into())];
    assert!((progress_b.current_time_seconds - 600.0).abs() < f64::EPSILON);
    assert!(!progress_b.is_finished);
    assert!(
        matches!(cmd_rx.try_recv(), Ok(mbv_ctrl::CtrlCmd::UnifiedQueueApplyProgress { updates, .. })
        if updates == vec![mbv_ctrl::ProgressUpdate {
            content_id: mbv_queue::QueueItemContentId::Audiobookshelf {
                library_item_id: "show-a".into(),
                episode_id: "episode-a".into(),
            },
            position_ticks: 0,
            finished: false,
        }]),
        "only the finished target resets; the in-progress target sends no update"
    );
}

/// Unit 7 review: zip misalignment. A bulk Mark Unplayed whose first target
/// is the unfinished one must reconcile each remaining target with its own
/// apply: the finished episode resets to 0/unplayed in browse, the in-progress
/// episode keeps its position, and the single queue update names only the
/// finished episode.
#[test]
fn bulk_mark_unplayed_with_a_leading_in_progress_target_reconciles_each_target_with_its_own_apply()
{
    let (mut app, cmd_rx) = mark_ready_app();
    seed_episode_progress(&mut app, "episode-b", 600.0, false);
    seed_episode_progress(&mut app, "episode-a", 2400.0, true);

    let generation = app.audiobookshelf_runtime.generation();
    feed_mark_completion(
        &mut app,
        generation,
        vec![episode_target("episode-b"), episode_target("episode-a")],
        false,
        Ok(()),
    );
    assert!(app.drain_audiobookshelf_events());

    let progress_b = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-b".into())];
    assert!(
        (progress_b.current_time_seconds - 600.0).abs() < f64::EPSILON,
        "the in-progress episode keeps its saved position"
    );
    assert!(
        !progress_b.is_finished,
        "the in-progress episode stays unplayed"
    );

    let progress_a = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert!(
        !progress_a.is_finished,
        "the finished episode resets to unplayed"
    );
    assert!(
        progress_a.current_time_seconds == 0.0,
        "the finished episode's position resets to zero"
    );

    assert!(
        matches!(cmd_rx.try_recv(), Ok(mbv_ctrl::CtrlCmd::UnifiedQueueApplyProgress { updates, .. })
        if updates == vec![mbv_ctrl::ProgressUpdate {
            content_id: mbv_queue::QueueItemContentId::Audiobookshelf {
                library_item_id: "show-a".into(),
                episode_id: "episode-a".into(),
            },
            position_ticks: 0,
            finished: false,
        }]),
        "the single queue update names only the finished episode"
    );
    assert!(cmd_rx.try_recv().is_err(), "only one update batch is sent");
}

/// Mark Played on the actively owned session's episode updates browse progress
/// but leaves that slot out of the `ApplyProgress` updates.
#[test]
fn mark_played_on_the_active_slot_updates_browse_and_leaves_that_slot_out() {
    let (mut app, cmd_rx) = mark_ready_app();
    seed_episode_progress(&mut app, "episode-a", 300.0, false);
    // episode-a IS the active Player-owned slot.
    app.local_view.adopt_queue_items_with_active(
        vec![
            episode_queue_item("episode-a"),
            episode_queue_item("episode-b"),
        ],
        0,
        0,
    );
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
    });

    let generation = app.audiobookshelf_runtime.generation();
    feed_mark_completion(
        &mut app,
        generation,
        vec![episode_target("episode-a")],
        true,
        Ok(()),
    );
    assert!(app.drain_audiobookshelf_events());

    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert!(progress.is_finished, "browse progress still updates");
    assert!((progress.current_time_seconds - 300.0).abs() < f64::EPSILON);
    assert!(
        cmd_rx.try_recv().is_err(),
        "the actively owned session's slot must send no queue update"
    );
}

/// A completion from a superseded setup generation is dropped whole: no browse
/// change and no queue update.
#[test]
fn mark_completion_from_a_superseded_generation_changes_nothing() {
    let (mut app, cmd_rx) = mark_ready_app();
    seed_episode_progress(&mut app, "episode-a", 2400.0, true);
    let stale = app.audiobookshelf_runtime.generation();
    app.audiobookshelf_runtime.begin_setup();

    feed_mark_completion(
        &mut app,
        stale,
        vec![episode_target("episode-a")],
        false,
        Ok(()),
    );
    assert!(
        !app.drain_audiobookshelf_events(),
        "a superseded completion produces no work"
    );

    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert!(progress.is_finished);
    assert!((progress.current_time_seconds - 2400.0).abs() < f64::EPSILON);
    assert!(
        cmd_rx.try_recv().is_err(),
        "a dropped completion sends no update"
    );
}

/// A book mark goes through the book reconcile and names the book content
/// identity in its queue update.
#[test]
fn mark_played_completion_applies_a_book_through_the_book_path() {
    let (mut app, cmd_rx) = mark_ready_app();
    let library = mbv_audiobookshelf::AudiobookshelfLibrary {
        id: "abs-books".into(),
        name: "ABS Books".into(),
        media_type: "book".into(),
    };
    app.audiobookshelf_book_browse
        .push(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState::new(library));
    app.audiobookshelf_book_browse[0].progress.insert(
        "book-a".into(),
        mbv_audiobookshelf::AudiobookshelfBookProgress {
            library_item_id: "book-a".into(),
            current_time_seconds: 120.0,
            is_finished: false,
        },
    );

    let generation = app.audiobookshelf_runtime.generation();
    feed_mark_completion(
        &mut app,
        generation,
        vec![AudiobookshelfMenuTarget::Book {
            library_item_id: "book-a".into(),
        }],
        true,
        Ok(()),
    );
    assert!(app.drain_audiobookshelf_events());

    let progress = &app.audiobookshelf_book_browse[0].progress["book-a"];
    assert!(progress.is_finished);
    assert!((progress.current_time_seconds - 120.0).abs() < f64::EPSILON);
    assert!(
        matches!(cmd_rx.try_recv(), Ok(mbv_ctrl::CtrlCmd::UnifiedQueueApplyProgress { updates, .. })
        if updates == vec![mbv_ctrl::ProgressUpdate {
            content_id: mbv_queue::QueueItemContentId::AudiobookshelfBook {
                library_item_id: "book-a".into(),
            },
            position_ticks: 120 * mbv_emby_model::TICKS_PER_SECOND,
            finished: true,
        }]),
        "the book slot's progress updates under the book content identity"
    );
}

/// A failed mark changes nothing locally: browse progress keeps its cached
/// value and no queue update is sent.
#[test]
fn failed_mark_completion_leaves_progress_unchanged() {
    let (mut app, cmd_rx) = mark_ready_app();
    seed_episode_progress(&mut app, "episode-a", 2400.0, true);

    let generation = app.audiobookshelf_runtime.generation();
    feed_mark_completion(
        &mut app,
        generation,
        vec![episode_target("episode-a")],
        false,
        Err(AudiobookshelfError::from_class(
            AudiobookshelfFailureClass::Protocol,
        )),
    );
    assert!(app.drain_audiobookshelf_events());

    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert!(
        progress.is_finished,
        "a failed mark changes no played state"
    );
    assert!(
        (progress.current_time_seconds - 2400.0).abs() < f64::EPSILON,
        "a failed mark changes no position"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "a failed mark sends no queue update"
    );
}

/// A second mark while one is in flight is refused; PR 914 review. With a
/// completion channel already pending, the refusal flash shows a Warning
/// toast, no worker starts (the pending receiver is untouched, so its
/// completion still drains and applies), and the queue owner receives only
/// the original mark's update.
#[test]
fn a_second_mark_while_one_is_in_flight_is_refused() {
    let (mut app, cmd_rx) = mark_ready_app();
    app.audiobookshelf_runtime.state = mbv_core::service_runtime::ServiceState::Ready;
    app.config.lock().unwrap().audiobookshelf_setup = Some(mbv_config::AudiobookshelfSetup::new(
        "https://podcasts.example",
    ));
    mbv_config::save_service_secret(mbv_queue::ServiceKind::Audiobookshelf, "saved-token")
        .expect("secret is written under the test state dir");
    seed_episode_progress(&mut app, "episode-a", 2400.0, true);

    // An in-flight mark whose completion the drain has not read yet.
    let generation = app.audiobookshelf_runtime.generation();
    feed_mark_completion(
        &mut app,
        generation,
        vec![episode_target("episode-a")],
        false,
        Ok(()),
    );

    app.execute_context_action(
        Some(ContextAction::AudiobookshelfMarkUnplayed(vec![
            episode_target("episode-b"),
        ])),
        None,
    );

    assert_eq!(app.status, "Audiobookshelf mark already in progress");
    assert_eq!(app.status_severity, ToastSeverity::Warning);
    assert!(
        app.setup.audiobookshelf_mark_rx.is_some(),
        "the pending receiver is untouched"
    );

    assert!(
        app.drain_audiobookshelf_events(),
        "the original mark's completion still applies"
    );
    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert!(!progress.is_finished, "the original mark was applied");
    assert!(
        matches!(cmd_rx.try_recv(), Ok(mbv_ctrl::CtrlCmd::UnifiedQueueApplyProgress { updates, .. })
        if updates == vec![mbv_ctrl::ProgressUpdate {
            content_id: mbv_queue::QueueItemContentId::Audiobookshelf {
                library_item_id: "show-a".into(),
                episode_id: "episode-a".into(),
            },
            position_ticks: 0,
            finished: false,
        }]),
        "the owner receives only the original mark's update"
    );
    assert!(cmd_rx.try_recv().is_err(), "the refused mark sends nothing");
}

/// A credential rejection routes through the existing Audiobookshelf
/// authentication classification: the Service fails into `NeedsAuthentication`
/// and the saved credential is cleared (progress is covered by the plain
/// failure test above).
#[test]
fn mark_credential_rejection_uses_the_authentication_classification() {
    let mut app = super::podcast::audiobookshelf_app();
    let generation = app.audiobookshelf_runtime.generation();
    mbv_config::save_service_secret(mbv_queue::ServiceKind::Audiobookshelf, "saved-token")
        .expect("secret is written under the test state dir");

    feed_mark_completion(
        &mut app,
        generation,
        vec![episode_target("episode-a")],
        true,
        Err(AudiobookshelfError::from_class(
            AudiobookshelfFailureClass::AuthenticationRejected,
        )),
    );
    assert!(app.drain_audiobookshelf_events());

    assert_eq!(
        app.audiobookshelf_runtime.state,
        mbv_core::service_runtime::ServiceState::NeedsAuthentication
    );
    assert!(
        mbv_config::load_service_secret(mbv_queue::ServiceKind::Audiobookshelf).is_none(),
        "a rejection clears the saved credential"
    );
}
