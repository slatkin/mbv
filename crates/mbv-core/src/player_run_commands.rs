impl PlaybackRun {
    fn handle_command(
        &mut self,
        cmd: PlayerCommand,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) -> bool {
        let mut cancel_stop = false;
        match cmd {
            PlayerCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            } => {
                log::warn!(target: "player", "next-up: sending script-message mbv-next-up id={item_id} show={show_title} ep={ep_title}");
                let r = mpv.command(
                    "script-message",
                    &["mbv-next-up", &item_id, &show_title, &ep_title, &artist],
                );
                log::warn!(target: "player", "next-up: script-message result={r:?}");
            }
            PlayerCommand::TogglePause => {
                let p = self.status.lock().unwrap().paused;
                let _ = mpv.set_property("pause", !p);
            }
            PlayerCommand::JumpTo(idx) => {
                if let Some(slot_id) = self.slot_id_at(idx) {
                    // mpv playlist indices are adapter coordinates; pin the
                    // target slot identity before asking mpv to move.
                    self.forced_slot_id = Some(slot_id);
                    if let Err(e) = mpv.set_property("playlist-pos", idx as i64) {
                        self.forced_slot_id = None;
                        log::warn!(target: "player", "jump-to idx={idx} failed: {}", mpv_err_str(&e));
                    } else {
                        // Selecting a track should always start it playing, even if
                        // mpv was paused on the previous track — otherwise the new
                        // track loads silently "stuck" paused (see issue: Enter on a
                        // queue item, or a remote Next/Previous command, while paused).
                        let _ = mpv.set_property("pause", false);
                    }
                }
            }
            PlayerCommand::QueueAppend { items } => {
                self.cmd_append_queue(items, mpv);
            }
            PlayerCommand::QueueRemove(idx) => {
                if let Some(slot_id) = self.slot_id_at(idx) {
                    let active_slot_id = self.active_slot_id();
                    let _ = mpv.command("playlist-remove", &[&idx.to_string()]);
                    if active_slot_id == Some(slot_id) {
                        let _ = self.queue.remove_active_slot_confirmed(slot_id);
                    } else {
                        let _ = self.queue.remove_slot(slot_id);
                    }
                    self.refresh_current_idx_from_queue();
                    if self.forced_slot_id == Some(slot_id) {
                        self.forced_slot_id = None;
                    }
                    if active_slot_id == Some(slot_id) {
                        // Currently playing track removed — clear reporter item_id to prevent
                        // stale progress reports until on_end_file transitions to the next track.
                        let mut ids = self.reporter.ids.lock().unwrap();
                        ids.0.clear();
                    }
                }
            }
            PlayerCommand::QueueMove(from, to) => {
                if from < self.queue_len() && to < self.queue_len() && from != to {
                    // mpv's playlist-move index2 names the *pre-move* slot the
                    // entry should end up next to, not its post-move index: for
                    // from < to the entry actually lands at to - 1, not to (mpv
                    // manual's own "paradox" note, confirmed against mpv 0.41).
                    // Passing to + 1 (one past the end when to == n - 1, which
                    // mpv also accepts as "move to end") makes mpv's result
                    // match this struct's from/to bookkeeping below.
                    let mpv_to = if from < to { to + 1 } else { to };
                    let _ = mpv.command("playlist-move", &[&from.to_string(), &mpv_to.to_string()]);
                    if let Some(slot_id) = self.slot_id_at(from) {
                        let had_active_slot = self.active_slot_id().is_some();
                        let _ = self.queue.move_slot(slot_id, to);
                        if had_active_slot {
                            self.refresh_current_idx_from_queue();
                        } else {
                            self.current_idx = shift_index_for_move(self.current_idx, from, to);
                            self.sync_status_position();
                        }
                    }
                }
            }
            PlayerCommand::NextUpDismiss => {
                let _ = mpv.command("script-message", &["mbv-next-up-dismiss"]);
            }
            PlayerCommand::SkipIntroDismiss => {
                let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
            }
            PlayerCommand::ReplaceQueue {
                items: new_items,
                start_idx,
            } => {
                self.cmd_replace_queue(new_items, start_idx, mpv, progress);
                cancel_stop = true;
            }
            PlayerCommand::SetVolume(v) => {
                let vol_max = self.status.lock().unwrap().volume_max;
                let v = v.clamp(0, vol_max);
                let raw = (10.0 * (v as f64).sqrt()).round() as i64;
                let _ = mpv.set_property("volume", raw as f64);
                self.status.lock().unwrap().volume = v;
                let _ = mpv.command("show-text", &[&format!("Volume: {v}%"), "1500"]);
            }
            PlayerCommand::Seek(secs) => {
                let _ = mpv.command("seek", &[&secs.to_string(), "relative"]);
                self.last_seek_at = Some(Instant::now());
            }
            PlayerCommand::SeekAbsolute(secs) => {
                let _ = mpv.command("seek", &[&secs.to_string(), "absolute"]);
                self.last_seek_at = Some(Instant::now());
            }
            PlayerCommand::SetAudio(id) => {
                if id > 0 {
                    let _ = mpv.set_property("aid", id);
                } else {
                    let _ = mpv.set_property("aid", "no".to_string());
                }
                self.status.lock().unwrap().audio_id = id;
                refresh_tracks(mpv, &self.status);
            }
            PlayerCommand::SetSub(id) => {
                if id == 0 {
                    let _ = mpv.set_property("sid", "no".to_string());
                } else {
                    let _ = mpv.set_property("sid", id);
                }
                refresh_tracks(mpv, &self.status);
                self.status.lock().unwrap().sub_id = id;
            }
            PlayerCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            } => {
                {
                    let mut p = self.subtitle_prefs.lock().unwrap();
                    p.mode = mode;
                    p.subtitle_lang = subtitle_lang;
                    p.audio_lang = audio_lang;
                }
                let prefs = self.subtitle_prefs.lock().unwrap().clone();
                auto_select_tracks(mpv, &self.status, &prefs);
            }
            PlayerCommand::SetMute(m) => {
                let _ = mpv.set_property("mute", m);
                self.status.lock().unwrap().muted = m;
            }
            PlayerCommand::LoadNew {
                url,
                start_pos,
                item,
            } => {
                self.cmd_load_new(url, start_pos, item, mpv, progress);
                cancel_stop = true;
            }
        }
        cancel_stop
    }

    fn cmd_replace_queue(
        &mut self,
        new_items: Vec<MediaItem>,
        start_idx: usize,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        self.cancel_pending_quit();
        if new_items.is_empty() {
            self.stop_report = StopReport::mark_sent(self.reporter.report_stopped(self.last_valid_pos));
            let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
            let _ = mpv.command("playlist-clear", &[]);
            self.origin = PlaybackOrigin::Queue;
            self.set_origin(self.origin);
            self.queue = PlaybackQueue::default();
            self.current_idx = 0;
            self.sync_status_position();
            self.last_valid_pos = 0;
            self.pending_initial_jump = false;
            self.load_state = LoadState::Ready;
            self.begin_item_lifecycle();
            self.osd_title.clear();
            self.pending_resume_secs = None;
            self.series_id.clear();
            self.season = 0;
            self.episode = 0;
            return;
        }
        // report_stopped for current item; is_audio zeroing handled inside.
        self.stop_report = StopReport::mark_sent(self.reporter.report_stopped(self.last_valid_pos));
        // Replacing the playlist should always start playing it, even if mpv
        // was left paused on the previous item (reused-window fast path).
        let _ = mpv.set_property("pause", false);

        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
        // Remove all old playlist entries except the current one so that
        // the subsequent loadfile "replace" starts from a clean slate.
        // Without this, old entries remain and playlist-pos = start_idx
        // lands on a stale file instead of new_items[start_idx].
        let _ = mpv.command("playlist-clear", &[]);

        let start_idx = start_idx.min(new_items.len() - 1);
        for (i, item) in new_items.iter().enumerate() {
            let ep = if item.is_audio() { "Audio" } else { "Videos" };
            let url = format!(
                "{}/{}/{}/stream?static=true&api_key={}",
                self.server_url, ep, item.id, self.token
            );
            let mode = if i == 0 { "replace" } else { "append-play" };
            let title_opt = mpv_title_opt(&item.display_name());
            if let Err(e) = mpv.command("loadfile", &[url.as_str(), mode, "-1", title_opt.as_str()])
            {
                log::warn!(target: "player", "ReplaceQueue loadfile error: {}", mpv_err_str(&e));
            }
        }
        let active_item = new_items[start_idx].clone();
        let _ = mpv.set_property("start", "0");
        send_ep_info(mpv, &active_item);
        // loadfile "replace" displaces the current file (EndFile #1).
        // If start_idx > 0 we also set playlist-pos which displaces item[0] (EndFile #2).
        // Use = not += so a stale load_state from a prior operation never stacks.
        // Clear pending_initial_jump too since any in-flight initial jump is superseded.
        self.pending_initial_jump = false;
        self.load_state = LoadState::begin_replace(if start_idx > 0 { 2 } else { 1 });
        if start_idx > 0 {
            let _ = mpv.set_property("playlist-pos", start_idx as i64);
        }

        self.origin = PlaybackOrigin::Queue;
        self.set_origin(self.origin);
        self.queue = PlaybackQueue::from_items(new_items, Some(start_idx));
        self.current_idx = start_idx;
        self.load_active_item_state();
        // stop_report stays Sent until load_state drains to Ready in on_end_file,
        // preventing a duplicate report_stopped for the displaced file's EndFile(Quit).
        self.begin_item_lifecycle();
        log::info!(target: "player", "playlist queue-replace idx={start_idx} pending_resume={:?}s", self.pending_resume_secs);
        {
            let mut s = self.status.lock().unwrap();
            s.position_ticks = active_item.playback_position_ticks;
            s.runtime_ticks = active_item.runtime_ticks;
            s.current_idx = self.current_idx;
            s.queue_len = self.queue_len();
            s.set_current_item_metadata(&active_item);
        }

        // Stop progress reporter during transition to prevent stale reports,
        // then restart for the new item.
        progress.stop_and_join(self.progress_join_budget());
        let (urls, ok) = self.reporter.start_item(&active_item);
        self.ext_sub_urls = urls;
        if !ok {
            log::warn!(target: "player", "start_item failed for playlist replace item={}", active_item.id);
        }
        *progress = spawn_progress_reporter(self.reporter.clone());
    }

    fn append_items_to_queue(&mut self, items: Vec<MediaItem>) {
        for item in items {
            self.queue.append(item);
        }
        self.status.lock().unwrap().queue_len = self.queue_len();
    }

    fn cmd_append_queue(&mut self, new_items: Vec<MediaItem>, mpv: &Mpv) {
        if new_items.is_empty() {
            return;
        }

        for item in &new_items {
            let ep = if item.is_audio() { "Audio" } else { "Videos" };
            let url = format!(
                "{}/{}/{}/stream?static=true&api_key={}",
                self.server_url, ep, item.id, self.token
            );
            let title_opt = mpv_title_opt(&item.display_name());
            if let Err(e) = mpv.command(
                "loadfile",
                &[url.as_str(), "append-play", "-1", title_opt.as_str()],
            ) {
                log::warn!(target: "player", "QueueAppend loadfile error: {}", mpv_err_str(&e));
            }
        }

        self.append_items_to_queue(new_items);
    }

    fn cmd_load_new(
        &mut self,
        url: String,
        start_pos: f64,
        item: Box<MediaItem>,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        self.cancel_pending_quit();
        self.origin = PlaybackOrigin::Standalone;
        self.set_origin(self.origin);
        // Loading a new item should always start playing it, even if mpv
        // was left paused on the previous item (reused-window fast path).
        let _ = mpv.set_property("pause", false);

        // Stop progress reporter during transition to prevent stale reports.
        progress.stop_and_join(self.progress_join_budget());
        if self.config.audio_pipe_path.is_some() {
            self.reporter
                .transition_to_deferred(&item, self.last_valid_pos);
            self.ext_sub_urls = vec![];
        } else {
            self.ext_sub_urls = self.reporter.transition_to(&item, self.last_valid_pos);
        }
        *progress = spawn_progress_reporter(self.reporter.clone());

        self.queue = PlaybackQueue::from_items(vec![item.as_ref().clone()], Some(0));
        self.current_idx = 0;
        self.load_active_item_state();
        self.stop_report = StopReport::NotSent;
        self.load_state = LoadState::begin_single();
        self.pending_initial_jump = false;
        self.begin_item_lifecycle();
        {
            let mut st = self.status.lock().unwrap();
            st.runtime_ticks = item.runtime_ticks;
            st.position_ticks = item.playback_position_ticks;
            st.current_idx = 0;
            st.queue_len = 1;
            st.set_current_item_metadata(&item);
        }

        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
        let _ = mpv.command("script-message", &["mbv-next-up-dismiss"]);

        if start_pos > 0.0 {
            let _ = mpv.set_property("start", format!("{start_pos:.0}"));
        } else {
            let _ = mpv.set_property("start", "0");
        }
        let title_opt = mpv_title_opt(&item.display_name());
        log::info!(target: "player", "loadfile url={url} opts={title_opt:?}");
        if let Err(e) = mpv.command(
            "loadfile",
            &[url.as_str(), "replace", "-1", title_opt.as_str()],
        ) {
            log::warn!(target: "player", "loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e));
        }
        send_ep_info(mpv, &item);
    }

}
