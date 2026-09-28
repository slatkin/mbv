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
        let decision = queue_next_up_decision(
            self.queue_next_up,
            self.current_idx,
            self.queue_len(),
            self.active_item().is_some_and(QueueItem::is_tv_episode),
            self.item_at(self.current_idx + 1)
                .is_some_and(QueueItem::is_tv_episode),
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
            tracing::info!(name: "player.queue_next_up.armed", target: "player", index = self.current_idx + 1, "queue next-up armed");
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
