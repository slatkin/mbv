use super::super::*;

impl PlaybackRun {
    /// Emit `TrackChanged` for `slot_id`, tagging it with `transition` only if
    /// the transition's target matches (design D1/D4 settle shape: an
    /// active-file `JumpTo` and an `on_end_file` settle both confirm a
    /// transition through this same emit).
    pub(in crate::player) fn emit_track_changed(
        &mut self,
        slot_id: QueueSlotId,
        transition: Option<crate::playback_transition::Transition>,
    ) {
        let tag = transition
            .filter(|t| t.target == slot_id)
            .map(|t| (t.request_id, t.generation));
        log::info!(
            target: "transition",
            "track_changed: slot_id={:?} transition_tag={}",
            slot_id,
            tag.is_some(),
        );
        let _ = self.event_tx.send(PlayerEvent::TrackChanged {
            slot_id,
            transition: tag,
        });
    }

    pub(in crate::player) fn on_time_pos(&mut self, pos_secs: f64, mpv: &Mpv) {
        let ticks = (pos_secs * TICKS_PER_SECOND as f64) as i64;
        {
            let mut st = self.status.lock().unwrap();
            st.position_ticks = ticks;
            if pos_secs > 0.0 {
                if self.last_valid_pos == 0 {
                    log::info!(target: "player", "playlist last_valid_pos first non-zero: {}s idx={}", ticks / TICKS_PER_SECOND, self.current_idx);
                }
                self.last_valid_pos = ticks;
                st.last_valid_pos = ticks;
            }
        }

        if self.origin == PlaybackOrigin::Queue {
            // Playlist next-up: match Emby Web's timing from videoosd.js.
            // 60 s before end. Minimum episode: 10 min. Minimum remaining when shown: 20 s.
            const MIN_RUNTIME_TICKS: i64 = 600 * TICKS_PER_SECOND;
            const MIN_REMAIN_TICKS: i64 = 20 * TICKS_PER_SECOND;
            if self.current_idx + 1 < self.queue_len()
                && self.active_item().is_some_and(QueueItem::is_tv_episode)
                && self
                    .item_at(self.current_idx + 1)
                    .is_some_and(QueueItem::is_tv_episode)
            {
                let runtime = self.status.lock().unwrap().runtime_ticks;
                if runtime > 0 {
                    let show_at = runtime - 60 * TICKS_PER_SECOND;
                    let remaining = runtime - ticks;
                    if self.queue_next_up.is_fired() && ticks < show_at {
                        self.queue_next_up.reset();
                    }
                    if !self.queue_next_up.is_fired() && runtime >= MIN_RUNTIME_TICKS {
                        if remaining >= MIN_REMAIN_TICKS && ticks >= show_at {
                            self.queue_next_up.fire();
                            let _ = self.event_tx.send(PlayerEvent::QueueNextUp {
                                next_idx: self.current_idx + 1,
                            });
                        } else if self.queue_next_up == NextUp::Idle
                            && ticks > 0
                            && ticks < TICKS_PER_SECOND * 5
                        {
                            self.queue_next_up.arm();
                            log::info!(target: "player", "queue next-up armed idx={}", self.current_idx + 1);
                        }
                    }
                }
            }
        } else if !self.next_up.is_fired() {
            const NEXT_UP_TICKS: i64 = 60 * TICKS_PER_SECOND;
            if self.series_id.as_str().is_empty() {
                if self.next_up == NextUp::Idle && ticks > 0 && ticks < TICKS_PER_SECOND * 5 {
                    self.next_up.arm();
                    log::warn!(target: "player", "next-up disabled: no series_id (Episode item without SeriesId in fetch)");
                }
            } else {
                let runtime = self.status.lock().unwrap().runtime_ticks;
                if runtime > NEXT_UP_TICKS && ticks > runtime - NEXT_UP_TICKS {
                    self.next_up.fire();
                    log::warn!(target: "player", "next-up: threshold reached series={}", self.series_id);
                    let _ = self.event_tx.send(PlayerEvent::NextUpThreshold {
                        series_id: self.series_id.clone(),
                        season: self.season,
                        episode: self.episode,
                    });
                } else if self.next_up == NextUp::Idle && ticks > 0 && ticks < TICKS_PER_SECOND * 5
                {
                    self.next_up.arm();
                    log::info!(target: "player", "next-up: armed series={} runtime={}s", self.series_id, runtime / TICKS_PER_SECOND);
                }
            }
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
