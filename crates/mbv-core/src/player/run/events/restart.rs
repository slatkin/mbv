use super::super::*;

impl PlaybackRun {
    pub(in crate::player) fn settle_idle_jump_on_restart(
        &mut self,
        position_ticks: i64,
    ) -> Option<(QueueSlotId, Option<crate::playback_transition::Transition>)> {
        if !self.forced_jump_from_idle {
            return None;
        }
        let Some(slot_id) = self.forced_slot_id.take() else {
            self.forced_jump_from_idle = false;
            self.forced_transition = None;
            return None;
        };
        let Some(index) = self.queue.slot_index(slot_id) else {
            self.forced_jump_from_idle = false;
            self.forced_transition = None;
            return None;
        };
        let transition = self.forced_transition.take();
        self.forced_jump_from_idle = false;
        if !self.set_active_index(index) {
            return None;
        }
        self.load_active_item_state();
        self.last_valid_pos = position_ticks;
        self.report_active_item();
        self.stop_report = StopReport::NotSent;
        {
            let item = self.active_item();
            let mut status = self.status.lock().unwrap();
            status.active = true;
            status.position_ticks = self.last_valid_pos;
            status.runtime_ticks = item.map_or(0, QueueItem::runtime_ticks);
            if let Some(emby) = item.and_then(QueueItem::as_emby) {
                status.set_current_item_metadata(emby);
            } else if let Some(item) = item {
                status.title = item.title().to_string();
                status.art_item_id = item.id().to_string();
            }
        }
        self.observe_reporting(true);
        Some((slot_id, transition))
    }

    fn init_tracks_once(&mut self, mpv: &Mpv) {
        let prefs = self.subtitle_prefs.lock().unwrap().clone();
        for url in &self.ext_sub_urls {
            if let Err(e) = mpv.command("sub-add", &[url.as_str()]) {
                log::warn!(target: "player", "sub-add failed: {url}: {e:?}");
            }
        }
        auto_select_tracks(mpv, &self.status, &prefs);
        self.tracks_initialized = true;
        self.apply_forced_resume(mpv);
        if let Some(item) = self.active_item().cloned() {
            if let Some(emby) = item.as_emby() {
                send_ep_info(mpv, emby);
            }
        }
        if self.config.use_mpv_config {
            let _ = mpv.command("show-text", &[&self.osd_title, "3000"]);
        }
    }

    /// A re-visited playlist entry only honors its baked `start=` option the
    /// first time it ever loads; mpv reopens it from scratch on a later
    /// `playlist-pos` jump, discarding whatever was watched in this session.
    /// `forced_resume_ticks` (armed by the JumpTo handler from the canonical
    /// queue's current position) repairs that with an explicit seek now that
    /// the entry is actually loaded and seekable.
    fn apply_forced_resume(&mut self, mpv: &Mpv) {
        let Some(ticks) = self.forced_resume_ticks.take() else {
            return;
        };
        let seconds = ticks as f64 / TICKS_PER_SECOND as f64;
        if let Err(e) = mpv.command("seek", &[&seconds.to_string(), "absolute"]) {
            log::warn!(target: "player", "resume re-seek to {seconds}s failed: {}", mpv_err_str(&e));
            return;
        }
        // Arms the same seek-settle guard below, so this restart
        // (still reporting position 0, since status hasn't caught
        // up to the seek yet) does not report progress to Emby
        // before the seek actually lands.
        self.last_seek_at = Some(Instant::now());
    }

    fn settle_seek_osd(&mut self, mpv: &Mpv, event_name: &'static str) -> &'static str {
        if self.origin == PlaybackOrigin::Standalone {
            self.next_up.reset();
            self.show_seek_osd(mpv);
            return "Seek";
        }
        self.show_seek_osd(mpv);
        event_name
    }

    fn show_seek_osd(&mut self, mpv: &Mpv) {
        if self.last_seek_at.take().is_none() || !self.config.use_mpv_config {
            return;
        }
        let _ = mpv.command("show-text", &[&self.osd_title, "2000"]);
    }

    fn report_restart_progress(&mut self, event_name: &str) {
        let seek_settled = self
            .last_seek_at
            .is_none_or(|t| t.elapsed() > Duration::from_millis(500));
        if self.quit_at.is_some() || !seek_settled {
            return;
        }
        self.last_seek_at = None;
        if self.origin == PlaybackOrigin::Standalone {
            self.reporter.report_progress(event_name);
        } else if !self.reporter.is_audio.load(Ordering::Relaxed) {
            self.reporter.report_progress("TimeUpdate");
        }
    }

    pub(in crate::player) fn on_playback_restart(&mut self, mpv: &Mpv) {
        let settled_idle_jump = self.settle_idle_jump_on_restart(mpv_position_ticks(mpv));
        let was_seek = self.last_seek_at.is_some();
        self.active_file_starting = false;
        // `PlaybackRestart` is the concrete mpv-owned event used by mbvd as
        // its output-started boundary. It says nothing about downstream pipe
        // buffers or actual audibility.
        let _ = self.event_tx.send(PlayerEvent::OutputStarted);
        self.pending_initial_playlist_layout = false;
        {
            let h: i64 = mpv.get_property("video-params/h").unwrap_or(0);
            let is_img: bool = mpv
                .get_property("current-tracks/video/image")
                .unwrap_or(false);
            let codec: String = mpv.get_property("audio-codec-name").unwrap_or_default();
            let mut st = self.status.lock().unwrap();
            st.video_height = h;
            st.audio_codec = codec.to_lowercase();
            st.video_is_image = is_img;
        }
        if self.startup_pause.is_holding() {
            self.startup_pause.clear();
            log::info!(
                target: "player",
                "audio pipe: startup gate cleared on PlaybackRestart (playlist)"
            );
            let _ = mpv.set_property("pause", false);
        }
        let mut event_name = "TimeUpdate";
        if !self.tracks_initialized {
            self.init_tracks_once(mpv);
        } else {
            event_name = self.settle_seek_osd(mpv, event_name);
        }
        self.report_restart_progress(event_name);
        if was_seek {
            self.observe_reporting(true);
        }
        if let Some((slot_id, transition)) = settled_idle_jump {
            self.emit_track_changed(slot_id, transition);
        }
    }
}
