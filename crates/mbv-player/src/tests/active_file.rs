use super::*;

/// playback-run-lifecycle row 4.1 / defect 3 (spec requirement 4, scenario
/// "An active-file jump fails"): a selection failure during a jump into the
/// active-file projection must end the run with one `Stopped` that carries
/// the error and names the slot the run tried to select; before the fix the
/// error arm sent nothing.
#[test]
fn failed_active_file_jump_reports_one_stopped_naming_the_target_slot() {
    let (mut session, status, events, http) = make_queue_session_for_pos_tests_with_mock(0);
    let (holder, target, last) = (
        session.slot_id_at(0).unwrap(),
        session.slot_id_at(1).unwrap(),
        session.slot_id_at(2).unwrap(),
    );
    // Lift the active-file projection onto Audiobookshelf slots with no
    // active setup context, so selecting a slot beside the holder fails
    // preparation and never touches mpv.
    session.queue = mbv_queue::ExecutionSequence::from_slot_items(
        vec![
            (holder, abs_item()),
            (target, abs_item()),
            (last, abs_book_item()),
        ],
        Some(holder),
    );
    session.current_idx = 0;
    session.run_lifecycle.enter_active_file();
    // The outgoing item's stop report is the only HTTP call the failure makes.
    http.respond(200, "");
    http.respond(200, "");
    let (ours, _peer) = std::os::unix::net::UnixStream::pair().expect("socket pair");
    let mpv =
        Mpv::for_test(crate::mpv_session::ipc::IpcSession::from_stream(ours).expect("session"));
    let mut progress = noop_progress();

    session.handle_command(
        PlayerCommand::JumpTo {
            slot_id: target,
            request_id: 42,
            generation: 1,
            resume_ticks: None,
        },
        &mpv,
        &mut progress,
    );

    let event = events
        .try_recv()
        .expect("a failed active-file jump reports one Stopped");
    let PlayerEvent::Stopped { slot_id, error, .. } = event else {
        panic!("expected a Stopped event, got {event:?}");
    };
    assert_eq!(
        slot_id,
        Some(target),
        "the stop names the failed selection target"
    );
    assert!(error.is_some(), "the stop carries the selection error");
    assert!(
        events.try_recv().is_err(),
        "a failed active-file jump ends the run exactly once"
    );
    assert!(
        !status.lock().unwrap().active,
        "the run must not stay marked active with nothing selected"
    );
}

pub(crate) fn noop_progress() -> ProgressGuard {
    let (stop_tx, _) = mpsc::channel();
    ProgressGuard {
        stop_tx,
        handle: None,
    }
}

pub(crate) fn abs_item() -> QueueItem {
    QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
        mbv_queue::AudiobookshelfQueueItem {
            library_item_id: "show".into(),
            episode_id: "episode".into(),
            title: "Episode".into(),
            show_title: None,
            author: None,
            description: None,
            duration_ticks: Some(100_u64),
            position_ticks: 0,
            played: false,
            pub_date_secs: None,
            is_finished: false,
            cover_path: None,
        },
    ))
}

pub(crate) fn abs_book_item() -> QueueItem {
    QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(
        mbv_queue::AudiobookshelfBookQueueItem {
            library_item_id: "book".into(),
            title: "Book".into(),
            author: None,
            duration_ticks: Some(100_u64),
            position_ticks: 0,
            played: false,
            is_finished: false,
            cover_path: None,
        },
    ))
}
