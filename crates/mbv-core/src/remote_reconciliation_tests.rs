use super::*;

fn occurrence(id: u64, media: &str, runtime: i64) -> SubmittedOccurrence {
    SubmittedOccurrence::new(id, media).runtime_ticks(runtime)
}

fn tracker(items: Vec<SubmittedOccurrence>) -> ReconciliationTracker {
    ReconciliationTracker::new("session", items, 0, 0).unwrap()
}

#[test]
fn tracker_identity_is_unique_across_same_emby_session() {
    let first = tracker(vec![
        SubmittedOccurrence::new(1, "a"),
        SubmittedOccurrence::new(2, "b"),
    ]);
    let second = tracker(vec![
        SubmittedOccurrence::new(1, "a"),
        SubmittedOccurrence::new(2, "b"),
    ]);

    assert_ne!(first.tracking_id(), second.tracking_id());
    assert_eq!(first.session_id(), second.session_id());
}

#[test]
fn command_observation_remains_eligible_after_newer_poll() {
    let mut tracker = tracker(vec![
        SubmittedOccurrence::new(1, "a"),
        SubmittedOccurrence::new(2, "b"),
    ]);
    tracker.track_command_generation(2);
    tracker.observe(RemoteObservation::playing(3, "session", "stale", 0, 100, 1));

    let effects = tracker.observe(RemoteObservation::playing(2, "session", "a", 1, 100, 2));
    assert!(effects
        .iter()
        .any(|effect| matches!(effect, ReconciliationEffect::StateChanged { .. })));
    assert_eq!(tracker.state(), TrackingState::Tracking);
}

#[test]
fn pre_command_incompatible_poll_cannot_complete_or_contradict_expected_transition() {
    let mut tracker = tracker(vec![
        SubmittedOccurrence::new(1, "a"),
        SubmittedOccurrence::new(2, "b"),
    ]);
    tracker.track_command_generation(1);
    tracker.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    tracker.issue_intent(RemoteIntent::Next { target: 2 }, 2);
    tracker.track_command_generation(3);
    let effects = tracker.observe(RemoteObservation::playing(4, "session", "a", 99, 100, 3));
    assert!(effects.is_empty());
    assert_eq!(tracker.state(), TrackingState::Tracking);
    let effects = tracker.observe(RemoteObservation::playing(3, "session", "b", 1, 100, 4));
    assert!(effects.contains(&ReconciliationEffect::ExpectedConfirmed));
    assert_eq!(tracker.current_occurrence().unwrap().occurrence_id, 2);
}

#[test]
fn acknowledged_command_is_confirmed_by_later_ordinary_poll() {
    let mut tracker = tracker(vec![
        SubmittedOccurrence::new(1, "a"),
        SubmittedOccurrence::new(2, "b"),
    ]);
    tracker.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    tracker.issue_intent(RemoteIntent::Next { target: 2 }, 2);
    tracker.track_command_generation(5);
    // The command succeeded but its immediate post-command poll failed: only
    // the acknowledgment arrives. Tracking must not freeze waiting for a
    // command-scoped observation.
    tracker.acknowledge_command(5);
    let effects = tracker.observe(RemoteObservation::playing(6, "session", "b", 1, 100, 3));
    assert!(effects.contains(&ReconciliationEffect::ExpectedConfirmed));
    assert_eq!(tracker.current_occurrence().unwrap().occurrence_id, 2);
    assert_eq!(tracker.state(), TrackingState::Tracking);
}

#[test]
fn acknowledgment_does_not_expire_an_expected_that_later_ordinary_poll_confirms() {
    let mut tracker = tracker(vec![
        SubmittedOccurrence::new(1, "a"),
        SubmittedOccurrence::new(2, "b"),
    ]);
    tracker.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    tracker.issue_intent(RemoteIntent::Next { target: 2 }, 2);
    tracker.track_command_generation(5);
    tracker.acknowledge_command(5);
    // A poll issued after the acknowledgment boundary cannot contradict or
    // suppress the command's authoritative post-dispatch observation; the
    // boundary rejects the pre-command poll and admits the post-command one.
    let effects = tracker.observe(RemoteObservation::playing(3, "session", "b", 1, 100, 4));
    assert!(effects.is_empty());
    let effects = tracker.observe(RemoteObservation::playing(6, "session", "b", 1, 100, 5));
    assert!(effects.contains(&ReconciliationEffect::ExpectedConfirmed));
}

#[test]
fn stale_command_acknowledgment_and_observation_cannot_clear_a_newer_expected() {
    let mut tracker = tracker(vec![
        SubmittedOccurrence::new(1, "a"),
        SubmittedOccurrence::new(2, "b"),
        SubmittedOccurrence::new(3, "c"),
    ]);
    tracker.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    // Command A targets b; command B supersedes it and targets c.
    tracker.issue_intent(RemoteIntent::Next { target: 2 }, 2);
    tracker.track_command_generation(5);
    tracker.issue_intent(RemoteIntent::Next { target: 3 }, 3);
    tracker.track_command_generation(7);
    // Command A's delayed acknowledgment must not advance command B's boundary.
    tracker.acknowledge_command(5);
    // Command A's delayed observation cannot confirm, contradict, or clear
    // command B's Expected transition while command B is pending.
    let effects = tracker.observe(RemoteObservation::playing(6, "session", "b", 1, 100, 4));
    assert!(effects.is_empty());
    assert_eq!(
        tracker.expected().unwrap().intent,
        RemoteIntent::Next { target: 3 }
    );
    assert_eq!(tracker.current_occurrence().unwrap().occurrence_id, 1);
    // Command B's own acknowledgment advances the boundary, and its
    // authoritative post-dispatch observation confirms.
    tracker.acknowledge_command(7);
    let effects = tracker.observe(RemoteObservation::playing(6, "session", "b", 1, 100, 5));
    assert!(effects.is_empty(), "pre-boundary poll stays stale");
    let effects = tracker.observe(RemoteObservation::playing(8, "session", "c", 1, 100, 6));
    assert!(effects.contains(&ReconciliationEffect::ExpectedConfirmed));
    assert_eq!(tracker.current_occurrence().unwrap().occurrence_id, 3);
}

#[test]
fn duplicate_target_move_is_not_confirmed_by_media_alone() {
    let mut tracker = tracker(vec![
        SubmittedOccurrence::new(1, "a"),
        SubmittedOccurrence::new(2, "a"),
        SubmittedOccurrence::new(3, "b"),
    ]);
    tracker.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    // Next onto the duplicate A2 cannot be proven by observing media "a".
    tracker.issue_intent(RemoteIntent::Next { target: 2 }, 2);
    tracker.track_command_generation(5);
    tracker.acknowledge_command(5);
    // A stale near-end observation of A1 (same media as the target) must not
    // confirm the move onto A2.
    let effects = tracker.observe(RemoteObservation::playing(6, "session", "a", 95, 100, 3));
    assert!(!effects.contains(&ReconciliationEffect::ExpectedConfirmed));
    assert!(!effects
        .iter()
        .any(|effect| matches!(effect, ReconciliationEffect::Completion(_))));
    // A subsequent observation of B must not complete the never-observed A2
    // from the stale near-end observation.
    let effects = tracker.observe(RemoteObservation::playing(7, "session", "b", 1, 100, 4));
    assert!(!effects
        .iter()
        .any(|effect| matches!(effect, ReconciliationEffect::Completion(_))));
    assert!(tracker.completed().all(|occurrence_id| occurrence_id != 2));
    assert_eq!(tracker.current_occurrence().unwrap().occurrence_id, 3);
}

#[test]
fn distinct_target_move_is_confirmed_by_observed_media() {
    let mut tracker = tracker(vec![
        SubmittedOccurrence::new(1, "a"),
        SubmittedOccurrence::new(2, "a"),
        SubmittedOccurrence::new(3, "b"),
    ]);
    tracker.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    // A move onto B is provable because its media differs from the source.
    tracker.issue_intent(RemoteIntent::Next { target: 3 }, 2);
    tracker.track_command_generation(5);
    tracker.acknowledge_command(5);
    let effects = tracker.observe(RemoteObservation::playing(6, "session", "b", 1, 100, 3));
    assert!(effects.contains(&ReconciliationEffect::ExpectedConfirmed));
    assert_eq!(tracker.current_occurrence().unwrap().occurrence_id, 3);
}

#[test]
fn submitted_occurrences_retain_immutable_playlist_identity() {
    let source = SequenceSource::Playlist {
        id: "playlist-a".into(),
    };
    let first = SubmittedOccurrence::new(11, "media").source(source.clone());
    let second = SubmittedOccurrence::new(12, "media").source(source);

    assert_eq!(first.playlist_id(), Some("playlist-a"));
    assert_eq!(second.playlist_id(), Some("playlist-a"));
    assert_ne!(first.occurrence_id, second.occurrence_id);
}

#[test]
fn intents_resolve_non_index_occurrence_ids() {
    let mut t = tracker(vec![occurrence(41, "a", 100), occurrence(9001, "b", 100)]);
    t.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    t.issue_intent(RemoteIntent::Next { target: 9001 }, 2);

    let effects = t.observe(RemoteObservation::playing(2, "session", "b", 1, 100, 3));
    assert!(effects.contains(&ReconciliationEffect::ExpectedConfirmed));
    assert_eq!(t.current_occurrence().unwrap().occurrence_id, 9001);
}

#[test]
fn near_end_is_inclusive_and_requires_runtime() {
    assert!(!ReconciliationTracker::near_end(94, 100));
    assert!(ReconciliationTracker::near_end(95, 100));
    assert!(ReconciliationTracker::near_end(96, 100));
    assert!(!ReconciliationTracker::near_end(1, 0));
}

#[test]
fn startup_ignores_stale_prior_item_until_meaningful_progress() {
    let mut t = tracker(vec![occurrence(1, "a", 100), occurrence(2, "b", 100)]);
    assert!(t
        .observe(RemoteObservation::playing(1, "session", "b", 0, 100, 1))
        .is_empty());
    assert_eq!(t.state(), TrackingState::Starting);
    assert_eq!(
        t.observe(RemoteObservation::playing(2, "session", "b", 1, 100, 2))
            .last(),
        Some(&ReconciliationEffect::StateChanged {
            state: TrackingState::Invalid,
            reason: TrackingReason::StartContradicted,
        })
    );
}

#[test]
fn startup_observation_at_deadline_expires_without_confirmation() {
    let mut t = tracker(vec![occurrence(1, "a", 100), occurrence(2, "b", 100)]);

    let effects = t.observe(RemoteObservation::playing(
        1, "session", "a", 1, 100, 15_000,
    ));

    assert_eq!(t.state(), TrackingState::Invalid);
    assert!(effects.contains(&ReconciliationEffect::ExpectedExpired));
    assert!(effects.contains(&ReconciliationEffect::StateChanged {
        state: TrackingState::Invalid,
        reason: TrackingReason::StartExpired,
    }));
}

#[test]
fn adjacent_near_end_emits_completion_once() {
    let mut t = tracker(vec![occurrence(1, "a", 100), occurrence(2, "b", 100)]);
    t.observe(RemoteObservation::playing(1, "session", "a", 95, 100, 1));
    let effects = t.observe(RemoteObservation::playing(2, "session", "b", 1, 100, 2));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        ReconciliationEffect::Completion(item) if item.occurrence_id == 1
    )));
    assert!(t
        .observe(RemoteObservation::playing(3, "session", "b", 2, 100, 3))
        .iter()
        .all(|effect| !matches!(effect, ReconciliationEffect::Completion(_))));
}

#[test]
fn stale_generation_is_inert() {
    let mut t = tracker(vec![occurrence(1, "a", 100), occurrence(2, "b", 100)]);
    t.observe(RemoteObservation::playing(2, "session", "a", 10, 100, 2));
    assert!(t
        .observe(RemoteObservation::playing(1, "session", "b", 1, 100, 3))
        .is_empty());
    assert_eq!(t.current_occurrence().unwrap().media_id, "a");
}

#[test]
fn duplicate_reset_is_ambiguous() {
    let mut t = tracker(vec![
        occurrence(1, "a", 100),
        occurrence(2, "a", 100),
        occurrence(3, "b", 100),
    ]);
    t.observe(RemoteObservation::playing(1, "session", "a", 80, 100, 1));
    t.observe(RemoteObservation::playing(2, "session", "a", 1, 100, 2));
    assert_eq!(t.state(), TrackingState::Ambiguous);
    assert_eq!(t.candidates(), vec![1, 2]);
}

#[test]
fn stopped_final_near_end_completes_but_disappearance_does_not() {
    let mut t = tracker(vec![occurrence(1, "a", 100), occurrence(2, "b", 100)]);
    t.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    t.observe(RemoteObservation::playing(2, "session", "b", 95, 100, 2));
    let effects = t.observe(RemoteObservation::stopped(3, "session", 3));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        ReconciliationEffect::Completion(item) if item.occurrence_id == 2
    )));
    let mut t = tracker(vec![occurrence(1, "a", 100), occurrence(2, "b", 100)]);
    t.observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1));
    t.observe(RemoteObservation::playing(2, "session", "b", 95, 100, 2));
    assert!(t
        .session_disappeared()
        .iter()
        .all(|effect| !matches!(effect, ReconciliationEffect::Completion(_))));
    assert_eq!(t.state(), TrackingState::Suspended);
}

#[test]
fn representative_traces_are_table_driven() {
    let traces: Vec<(&str, Vec<RemoteObservation>, TrackingState)> = vec![
        (
            "startup",
            vec![RemoteObservation::playing(1, "session", "a", 1, 100, 1)],
            TrackingState::Tracking,
        ),
        (
            "adjacent",
            vec![
                RemoteObservation::playing(1, "session", "a", 95, 100, 1),
                RemoteObservation::playing(2, "session", "b", 1, 100, 2),
            ],
            TrackingState::Tracking,
        ),
        (
            "poll gap invalidation",
            vec![
                RemoteObservation::playing(1, "session", "a", 1, 100, 1),
                RemoteObservation::playing(2, "session", "c", 1, 100, 2),
            ],
            TrackingState::Invalid,
        ),
        (
            "duplicate ambiguity",
            vec![
                RemoteObservation::playing(1, "session", "a", 80, 100, 1),
                RemoteObservation::playing(2, "session", "a", 1, 100, 2),
            ],
            TrackingState::Ambiguous,
        ),
        (
            "present stopped",
            vec![
                RemoteObservation::playing(1, "session", "a", 1, 100, 1),
                RemoteObservation::stopped(2, "session", 2),
            ],
            TrackingState::Tracking,
        ),
    ];
    for (name, observations, expected) in traces {
        let items = if name == "duplicate ambiguity" {
            vec![
                occurrence(1, "a", 100),
                occurrence(2, "a", 100),
                occurrence(3, "b", 100),
            ]
        } else {
            vec![
                occurrence(1, "a", 100),
                occurrence(2, "b", 100),
                occurrence(3, "c", 100),
            ]
        };
        let mut tracker = tracker(items);
        for observation in observations {
            tracker.observe(observation);
        }
        assert_eq!(tracker.state(), expected, "trace {name}");
    }
}

#[derive(Clone)]
enum TraceStep {
    Observe(RemoteObservation),
    Disappear,
    Expire(u64),
    Reanchor(usize),
}

struct Trace {
    name: &'static str,
    media: &'static [&'static str],
    steps: Vec<TraceStep>,
    expected_state: TrackingState,
    expected_completion: Option<OccurrenceId>,
}

#[test]
fn reconciliation_paths_are_covered_by_table_driven_traces() {
    let traces = vec![
        Trace {
            name: "startup confirmation",
            media: &["a", "b", "c"],
            steps: vec![TraceStep::Observe(RemoteObservation::playing(
                1, "session", "a", 1, 100, 1,
            ))],
            expected_state: TrackingState::Tracking,
            expected_completion: None,
        },
        Trace {
            name: "near-end adjacent completion",
            media: &["a", "b"],
            steps: vec![
                TraceStep::Observe(RemoteObservation::playing(1, "session", "a", 95, 100, 1)),
                TraceStep::Observe(RemoteObservation::playing(2, "session", "b", 1, 100, 2)),
            ],
            expected_state: TrackingState::Tracking,
            expected_completion: Some(1),
        },
        Trace {
            name: "poll gap invalidation",
            media: &["a", "b", "c"],
            steps: vec![
                TraceStep::Observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1)),
                TraceStep::Observe(RemoteObservation::playing(2, "session", "c", 1, 100, 2)),
            ],
            expected_state: TrackingState::Invalid,
            expected_completion: None,
        },
        Trace {
            name: "stale generation",
            media: &["a", "b"],
            steps: vec![
                TraceStep::Observe(RemoteObservation::playing(2, "session", "a", 10, 100, 2)),
                TraceStep::Observe(RemoteObservation::playing(1, "session", "b", 1, 100, 3)),
            ],
            expected_state: TrackingState::Tracking,
            expected_completion: None,
        },
        Trace {
            name: "duplicate ambiguity",
            media: &["a", "a", "b"],
            steps: vec![
                TraceStep::Observe(RemoteObservation::playing(1, "session", "a", 80, 100, 1)),
                TraceStep::Observe(RemoteObservation::playing(2, "session", "a", 1, 100, 2)),
            ],
            expected_state: TrackingState::Ambiguous,
            expected_completion: None,
        },
        Trace {
            name: "backward invalidation",
            media: &["a", "b", "c"],
            steps: vec![
                TraceStep::Observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1)),
                TraceStep::Observe(RemoteObservation::playing(2, "session", "b", 1, 100, 2)),
                TraceStep::Observe(RemoteObservation::playing(3, "session", "a", 1, 100, 3)),
            ],
            expected_state: TrackingState::Invalid,
            expected_completion: None,
        },
        Trace {
            name: "suspension exact return",
            media: &["a", "b"],
            steps: vec![
                TraceStep::Observe(RemoteObservation::playing(1, "session", "a", 10, 100, 1)),
                TraceStep::Disappear,
                TraceStep::Observe(RemoteObservation::playing(2, "session", "a", 11, 100, 2)),
            ],
            expected_state: TrackingState::Tracking,
            expected_completion: None,
        },
        Trace {
            name: "re-anchor after invalidation",
            media: &["a", "b", "c"],
            steps: vec![
                TraceStep::Observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1)),
                TraceStep::Observe(RemoteObservation::playing(2, "session", "c", 1, 100, 2)),
                TraceStep::Reanchor(2),
            ],
            expected_state: TrackingState::Tracking,
            expected_completion: None,
        },
        Trace {
            name: "stopped playback remains tracked",
            media: &["a", "b"],
            steps: vec![
                TraceStep::Observe(RemoteObservation::playing(1, "session", "a", 10, 100, 1)),
                TraceStep::Observe(RemoteObservation::stopped(2, "session", 2)),
            ],
            expected_state: TrackingState::Tracking,
            expected_completion: None,
        },
        Trace {
            name: "final stopped completion",
            media: &["a", "b"],
            steps: vec![
                TraceStep::Observe(RemoteObservation::playing(1, "session", "a", 1, 100, 1)),
                TraceStep::Observe(RemoteObservation::playing(2, "session", "b", 95, 100, 2)),
                TraceStep::Observe(RemoteObservation::stopped(3, "session", 3)),
            ],
            expected_state: TrackingState::Tracking,
            expected_completion: Some(2),
        },
        Trace {
            name: "startup expiry",
            media: &["a", "b"],
            steps: vec![TraceStep::Expire(15_000)],
            expected_state: TrackingState::Invalid,
            expected_completion: None,
        },
    ];

    for trace in traces {
        let items = trace
            .media
            .iter()
            .enumerate()
            .map(|(index, media)| occurrence(index as u64 + 1, media, 100))
            .collect();
        let mut tracker = tracker(items);
        let mut completions = Vec::new();
        for step in trace.steps {
            let effects = match step {
                TraceStep::Observe(observation) => tracker.observe(observation),
                TraceStep::Disappear => tracker.session_disappeared(),
                TraceStep::Expire(now_ms) => tracker.expire(now_ms),
                TraceStep::Reanchor(index) => tracker.reanchor(index),
            };
            completions.extend(effects.into_iter().filter_map(|effect| match effect {
                ReconciliationEffect::Completion(item) => Some(item.occurrence_id),
                _ => None,
            }));
        }
        assert_eq!(
            tracker.state(),
            trace.expected_state,
            "trace {}",
            trace.name
        );
        assert_eq!(
            completions.into_iter().next(),
            trace.expected_completion,
            "trace {} completion",
            trace.name
        );
    }
}
