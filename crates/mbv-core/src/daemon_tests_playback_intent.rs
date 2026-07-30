#[test]
fn playback_intent_state_coalesces_startup_and_supersedes_new_play() {
    let mut state = PlaybackIntentState::default();
    let first = PlaybackIntent {
        request_id: 1,
        generation: 1,
        action: PlaybackIntentAction::Play {
            item_ids: vec!["a".into()],
            start_idx: 0,
            start_ticks: 0,
            source: QueueSource::Album,
        },
    };
    assert!(matches!(
        state.accept(7, first.clone(), true)[0].outcome,
        PlaybackIntentOutcome::Accepted
    ));
    state.mark_resolving(1);

    let duplicate = PlaybackIntent {
        request_id: 2,
        generation: 2,
        ..first.clone()
    };
    assert!(matches!(
        state.accept(7, duplicate, true)[0].outcome,
        PlaybackIntentOutcome::Coalesced {
            canonical_request_id: 1
        }
    ));

    let newer = PlaybackIntent {
        request_id: 3,
        generation: 3,
        action: PlaybackIntentAction::Play {
            item_ids: vec!["b".into()],
            start_idx: 0,
            start_ticks: 0,
            source: QueueSource::Album,
        },
    };
    let events = state.accept(7, newer, true);
    assert!(matches!(
        events[0].outcome,
        PlaybackIntentOutcome::Superseded
    ));
    assert!(matches!(events[1].outcome, PlaybackIntentOutcome::Accepted));
}

#[test]
fn stale_playback_resolution_cannot_apply_after_disconnect() {
    let mut state = PlaybackIntentState::default();
    let intent = PlaybackIntent {
        request_id: 9,
        generation: 4,
        action: PlaybackIntentAction::Stop,
    };
    state.accept(11, intent, true);
    state.invalidate_connection(11);
    assert!(state.applied_if_current(11, 9, 4).is_none());
}

#[test]
fn pipe_buffering_coalesces_until_its_generation_bound_deadline_settles() {
    let mut state = PlaybackIntentState::default();
    let first = PlaybackIntent {
        request_id: 1,
        generation: 11,
        action: PlaybackIntentAction::Play {
            item_ids: vec!["a".into()],
            start_idx: 0,
            start_ticks: 0,
            source: QueueSource::Album,
        },
    };
    state.accept(7, first.clone(), true);
    state.mark_resolving(first.request_id);
    state.mark_starting(first.request_id);
    let (_, status) = state
        .output_started_if_current(Some(Duration::ZERO))
        .expect("pipe intent should report buffering");
    assert!(matches!(
        status.phase,
        crate::ctrl::PipePlaybackPhase::OutputBuffering
    ));

    let duplicate = PlaybackIntent {
        request_id: 2,
        generation: 12,
        ..first.clone()
    };
    assert!(matches!(
        state.accept(7, duplicate, true)[0].outcome,
        PlaybackIntentOutcome::Coalesced {
            canonical_request_id: 1
        }
    ));
    assert!(matches!(
        state
            .settle_buffering_if_due()
            .map(|(_, event)| event.outcome),
        Some(PlaybackIntentOutcome::Applied)
    ));
    assert!(matches!(
        state.accept(
            7,
            PlaybackIntent {
                request_id: 3,
                generation: 13,
                ..first
            },
            true
        )[0]
        .outcome,
        PlaybackIntentOutcome::Accepted
    ));
}

#[test]
fn unconfigured_pipe_delay_settles_at_output_started_and_old_deadline_cannot_settle_replacement() {
    let mut state = PlaybackIntentState::default();
    let first = PlaybackIntent {
        request_id: 1,
        generation: 1,
        action: PlaybackIntentAction::Play {
            item_ids: vec!["a".into()],
            start_idx: 0,
            start_ticks: 0,
            source: QueueSource::Album,
        },
    };
    state.accept(7, first.clone(), true);
    let (_, status) = state
        .output_started_if_current(None)
        .expect("pipe intent should report observed output start");
    assert!(matches!(
        status.phase,
        crate::ctrl::PipePlaybackPhase::OutputStarted
    ));
    assert!(matches!(
        state.accept(
            7,
            PlaybackIntent {
                request_id: 2,
                generation: 2,
                ..first.clone()
            },
            true
        )[0]
        .outcome,
        PlaybackIntentOutcome::Accepted
    ));

    state.output_started_if_current(Some(Duration::ZERO));
    let replacement = PlaybackIntent {
        request_id: 3,
        generation: 3,
        action: PlaybackIntentAction::Play {
            item_ids: vec!["b".into()],
            start_idx: 0,
            start_ticks: 0,
            source: QueueSource::Album,
        },
    };
    state.accept(7, replacement, true);
    assert!(state.settle_buffering_if_due().is_none());
}
