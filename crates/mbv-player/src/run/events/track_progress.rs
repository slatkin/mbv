use super::super::{
    Mpv, NextUpFire, PlaybackOrigin, PlaybackRun, PlayerEvent, QueueItem, QueueSlotId,
    TICKS_PER_SECOND, handle_intro, queue_next_up_decision, standalone_next_up_decision,
};

impl PlaybackRun {
    /// Emit `TrackChanged` for `slot_id`, tagging it with `transition` only if
    /// the transition's target matches (design D1/D4 settle shape: an
    /// active-file `JumpTo` and an `on_end_file` settle both confirm a
    /// transition through this same emit).
    pub(crate) fn emit_track_changed(
        &mut self,
        slot_id: QueueSlotId,
        transition: Option<crate::transition::Transition>,
    ) {
        let tag = transition
            .filter(|t| t.target == slot_id)
            .map(|t| (t.request_id, t.generation));
        tracing::info!(name: "player.track.changed", target: "transition", slot = ?slot_id, has_transition = tag.is_some(), "track changed");
        let _ = self.event_tx.send(PlayerEvent::TrackChanged {
            slot_id,
            transition: tag,
        });
    }

    /// Playlist next-up: match Emby Web's timing from videoosd.js.
    /// 60 s before end. Minimum episode: 10 min. Minimum remaining when shown: 20 s.
    fn maybe_fire_queue_next_up(&mut self, ticks: i64) {
        let runtime = self.status.lock().unwrap().runtime_ticks;
        // The queue neighbor decides the next slot; an index past the end
        // means there is no Next Up.
        let current_is_episode = self.active_item().is_some_and(QueueItem::is_tv_episode);
        // Next Up needs an episode on both sides, so skip the lookup otherwise.
        let next_idx = current_is_episode
            .then(|| self.current_idx + 1)
            .filter(|next_idx| *next_idx < self.queue_len());
        let next_is_episode = next_idx
            .is_some_and(|next_idx| self.item_at(next_idx).is_some_and(QueueItem::is_tv_episode));
        let decision = queue_next_up_decision(
            self.queue_next_up,
            next_idx,
            current_is_episode,
            next_is_episode,
            runtime,
            ticks,
        );
        if decision.reset {
            self.queue_next_up.reset();
        }
        if let Some(NextUpFire::Queue(next_idx)) = decision.fire {
            self.queue_next_up.fire();
            let _ = self.event_tx.send(PlayerEvent::QueueNextUp { next_idx });
        } else if decision.arm {
            self.queue_next_up.arm();
            tracing::info!(name: "player.queue_next_up.armed", target: "player", index = ?next_idx, "queue next-up armed");
        }
    }

    fn maybe_fire_standalone_next_up(&mut self, ticks: i64) {
        let runtime = self.status.lock().unwrap().runtime_ticks;
        let has_series = !self.series_id.as_str().is_empty();
        let decision = standalone_next_up_decision(self.next_up, has_series, runtime, ticks);
        if let Some(NextUpFire::Standalone) = decision.fire {
            self.next_up.fire();
            tracing::warn!(name: "player.next_up.threshold_reached", target: "player", series = %self.series_id, "next-up threshold reached");
            let _ = self.event_tx.send(PlayerEvent::NextUpThreshold {
                series_id: self.series_id.clone(),
                season: self.season,
                episode: self.episode,
            });
        } else if decision.arm {
            self.next_up.arm();
            if has_series {
                tracing::info!(name: "player.next_up.armed", target: "player", series = %self.series_id, runtime_seconds = runtime / TICKS_PER_SECOND, "next-up armed");
            } else {
                tracing::warn!(name: "player.next_up.disabled", target: "player", "next-up disabled because episode has no series id");
            }
        }
    }

    pub(crate) fn on_time_pos(&mut self, pos_secs: f64, mpv: &Mpv) {
        let ticks = mbv_emby_model::seconds_to_ticks(pos_secs);
        // mpv's own duration is the loaded file's truth. The server's
        // RunTimeTicks can be absent on some items (unprobed home-video
        // entries) and is omitted by trimmed payloads (Latest, /Sessions),
        // which stranded the seekbar at 0:00 for an otherwise playing file.
        let mpv_runtime_ticks = adopted_runtime(
            self.status.lock().unwrap().runtime_ticks,
            mpv.get_property::<f64>("duration").ok(),
        );
        if let Some(runtime) = mpv_runtime_ticks {
            tracing::info!(
                name: "player.runtime.adopted_from_mpv",
                target: "player",
                runtime_ticks = runtime,
                "adopted mpv duration as the active file's runtime"
            );
            self.status.lock().unwrap().runtime_ticks = runtime;
        }
        {
            let mut st = self.status.lock().unwrap();
            st.position_ticks = ticks;
            if pos_secs > 0.0 {
                if self.last_valid_pos == 0 {
                    tracing::info!(name: "player.playback_position.initialized", target: "player", position_seconds = ticks / TICKS_PER_SECOND, index = self.current_idx, "first non-zero playback position");
                }
                self.last_valid_pos = ticks;
                st.last_valid_pos = ticks;
            }
        }

        if self.origin == PlaybackOrigin::Queue {
            self.maybe_fire_queue_next_up(ticks);
        } else {
            self.maybe_fire_standalone_next_up(ticks);
        }

        handle_intro(
            ticks,
            self.intro_start,
            self.intro_end,
            &mut self.intro_state,
            self.config.always_skip_intro,
            mpv,
            &self.event_tx,
        );
    }
}

/// The runtime the active file should report, given the queue item's stored
/// runtime and mpv's own `duration` property. mpv's measurement wins: the
/// server's `RunTimeTicks` is missing outright on some items (unprobed
/// home-video entries) and is dropped by trimmed payloads (`Latest`,
/// `/Sessions`), and a stranded zeroRuntime strands the seekbar at 0:00.
/// Returns the adopted tick count, or `None` when nothing should change.
fn adopted_runtime(current_ticks: i64, mpv_duration_secs: Option<f64>) -> Option<i64> {
    let ticks = mpv_duration_secs
        .filter(|secs| *secs > 0.0)
        .map(mbv_emby_model::seconds_to_ticks)
        .filter(|ticks| *ticks > 0)
        .filter(|ticks| *ticks != current_ticks)?;
    Some(ticks)
}

#[cfg(test)]
mod runtime_adoption_tests {
    use super::adopted_runtime;
    use rstest::rstest;

    /// The contract: mpv's duration, when mpv reports a positive one, becomes
    /// the active file's runtime — and nothing else writes. The first case is
    /// the reported bug: the server omitted `RunTimeTicks`, the queue item
    /// carried runtime 0, and the seekbar stayed stranded at 0:00.
    #[rstest]
    #[case::repairs_zero_runtime(0, Some(1845.9), Some(18_459_000_000))]
    #[case::absent_duration_writes_nothing(0, None, None)]
    #[case::non_positive_duration_writes_nothing(0, Some(0.0), None)]
    #[case::matching_runtime_writes_nothing(18_459_000_000, Some(1845.9), None)]
    fn only_a_positive_mpv_duration_is_adopted(
        #[case] current_ticks: i64,
        #[case] mpv_duration_secs: Option<f64>,
        #[case] expected: Option<i64>,
    ) {
        assert_eq!(adopted_runtime(current_ticks, mpv_duration_secs), expected);
    }
}
