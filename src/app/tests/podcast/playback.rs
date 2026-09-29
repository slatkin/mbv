use super::*;
use mbv_audiobookshelf::socket::AudiobookshelfProgress;
use mbv_audiobookshelf::socket::SocketEvent;
use mbv_emby_model::TICKS_PER_SECOND;
use mbv_queue::QueueItem;
use rstest::{fixture, rstest};

// Task 3.1(a)(b)(c): daemon route via PlayerEvent::AudiobookshelfProgress
// updates queue slots, browse progress map, and Unplayed filter.
// Task 3.4: Socket-merge tests — matching inactive/browsed, skipped
// active slot, unmatched episode, superseded generation.
//
// These test `handle_audiobookshelf_socket_event` with `SocketEvent::ProgressUpdated`
// directly, bypassing the network-connected socket thread.

/// Set up the app with a known socket generation and a matching
/// browse progress entry so the merge will recognise the episode.
#[fixture]
fn make_socket_merge_ready_app() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let mut app = super::podcast::audiobookshelf_app();
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_answered_queue_ops_with_command_rx(Vec::new(), 0);
    app.player = mbv_player::PlayerProxy::remote(remote, false);
    app.player_rx = player_rx;
    app.audiobookshelf_socket_generation = Some(app.audiobookshelf_runtime.generation());
    app.audiobookshelf_browse[0].progress.insert(
        ("show-a".into(), "episode-a".into()),
        mbv_audiobookshelf::AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 0.0,
            is_finished: false,
        },
    );
    (app, cmd_rx)
}

#[rstest]
fn clients_hold_no_editable_queue_socket_progress_relays_and_updates_browse_state(
    make_socket_merge_ready_app: (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>),
) {
    let (mut app, cmd_rx) = make_socket_merge_ready_app;
    // Seed the known episode as an inactive slot directly: this test owns
    // the socket merge, not the enqueue (row 5.3 made the enqueue an owner
    // op whose result only reaches the view through the owner's answer).
    app.player_tab.queue.append(QueueItem::Audiobookshelf(
        mbv_queue::AudiobookshelfItem::Episode(mbv_queue::AudiobookshelfQueueItem {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        }),
    ));

    // Activate a different slot so episode-a is inactive.
    let other = QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
        mbv_queue::AudiobookshelfQueueItem {
            library_item_id: "show-b".into(),
            episode_id: "ep-b".into(),
            title: "Other".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        },
    ));
    app.player_tab.queue.append(other);
    let other_slot = app.player_tab.queue.slots()[1].slot_id;
    let _ = app.player_tab.queue.set_active_slot(other_slot);

    assert!(
        app.playback_queue()
            .queue
            .active_slot()
            .and_then(|s| s.item.as_audiobookshelf())
            .is_some_and(|e| e.episode_id != "episode-a")
    );

    // Fire the socket progress event.
    app.handle_audiobookshelf_socket_event(SocketEvent::ProgressUpdated(AudiobookshelfProgress {
        library_item_id: "show-a".into(),
        episode_id: "episode-a".into(),
        current_time_seconds: 42.5,
        is_finished: true,
    }));

    let slot = app
        .player_tab
        .queue
        .slots()
        .iter()
        .find(|s| {
            s.item
                .as_audiobookshelf()
                .is_some_and(|e| e.episode_id == "episode-a")
        })
        .expect("episode-a slot");
    let episode = slot.item.as_audiobookshelf().unwrap();
    assert_eq!(episode.position_ticks, 0);
    assert!(!episode.is_finished);

    assert!(
        matches!(cmd_rx.try_recv(), Ok(mbv_ctrl::CtrlCmd::UnifiedQueueApplyProgress { updates, .. })
        if updates == vec![mbv_ctrl::ProgressUpdate {
            content_id: mbv_queue::QueueItemContentId::Audiobookshelf {
                library_item_id: "show-a".into(),
                episode_id: "episode-a".into(),
            },
            position_ticks: 85 * TICKS_PER_SECOND / 2,
            finished: true,
        }])
    );

    // Browse map updated directly while the queue awaits its owner's snapshot.
    let progress = &app.audiobookshelf_browse[0].progress[&("show-a".into(), "episode-a".into())];
    assert!((progress.current_time_seconds - 42.5).abs() < f64::EPSILON);
    assert!(progress.is_finished);
}

// Task 3.1(d): a no-match daemon-route event leaves the queue unchanged but
// still writes the browse progress map for the episode.
