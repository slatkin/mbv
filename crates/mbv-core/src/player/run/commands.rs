use super::*;

impl PlaybackRun {
    pub(in crate::player) fn handle_command(
        &mut self,
        cmd: PlayerCommand,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) -> bool {
        let mut cancel_stop = false;
        match cmd {
            PlayerCommand::JumpTo {
                slot_id,
                request_id,
                generation,
                resume_ticks,
            } => {
                self.cmd_jump_to(slot_id, request_id, generation, resume_ticks, mpv);
            }
            PlayerCommand::QueueAppend { items } => {
                self.cmd_append_queue(items, mpv);
            }
            PlayerCommand::QueueRemove(slot_id) => {
                self.cmd_queue_remove(slot_id, mpv);
            }
            PlayerCommand::QueueMove(slot_id, to) => {
                self.cmd_queue_move(slot_id, to, mpv);
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
            PlayerCommand::SubmitQueue { items, start_idx } => {
                if !items.is_empty() {
                    self.run_identity = self.status.lock().unwrap().sequence_generation;
                }
                self.cmd_submit_queue(items, start_idx, mpv, progress);
                cancel_stop = true;
            }
            command => {
                let _ = self.handle_simple_command(command, mpv);
            }
        }
        cancel_stop
    }

    fn handle_simple_command(
        &mut self,
        cmd: PlayerCommand,
        mpv: &Mpv,
    ) -> Result<(), PlayerCommand> {
        match cmd {
            PlayerCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            } => {
                log::warn!(target: "player", "next-up: sending script-message mbv-next-up id={item_id} show={show_title} ep={ep_title}");
                let result = mpv.command(
                    "script-message",
                    &["mbv-next-up", &item_id, &show_title, &ep_title, &artist],
                );
                log::warn!(target: "player", "next-up: script-message result={result:?}");
            }
            PlayerCommand::TogglePause => {
                let paused = self.status.lock().unwrap().paused;
                let _ = mpv.set_property("pause", !paused);
            }
            PlayerCommand::Next => {
                let target = self.relative_step_base() + 1;
                if target < self.queue_len() {
                    self.step_to_index(target, mpv);
                }
            }
            PlayerCommand::Previous => {
                if let Some(target) = self.relative_step_base().checked_sub(1) {
                    self.step_to_index(target, mpv);
                }
            }
            command @ (PlayerCommand::NextUpDismiss | PlayerCommand::SkipIntroDismiss) => {
                let message = if matches!(command, PlayerCommand::NextUpDismiss) {
                    "mbv-next-up-dismiss"
                } else {
                    "mbv-skip-intro-dismiss"
                };
                let _ = mpv.command("script-message", &[message]);
            }
            PlayerCommand::SetVolume(volume) => {
                let vol_max = self.status.lock().unwrap().volume_max;
                let (volume, raw) = volume_decision(volume, vol_max);
                let _ = mpv.set_property("volume", raw as f64);
                self.status.lock().unwrap().volume = volume;
                let _ = mpv.command("show-text", &[&format!("Volume: {volume}%"), "1500"]);
            }
            command @ (PlayerCommand::Seek(secs) | PlayerCommand::SeekAbsolute(secs)) => {
                let absolute = matches!(command, PlayerCommand::SeekAbsolute(_));
                let (mode, seconds) = seek_decision(secs, absolute);
                let _ = mpv.command("seek", &[&seconds, mode]);
                self.last_seek_at = Some(Instant::now());
            }
            command => return Err(command),
        }
        Ok(())
    }

    /// Explicit jump to an owner-assigned slot. Resolves the slot to this
    /// run's mpv-local ordinal; a stale slot (gone here) is rejected, never
    /// repaired by position (design D6).
    fn cmd_jump_to(
        &mut self,
        slot_id: QueueSlotId,
        request_id: crate::ctrl::PlaybackRequestId,
        generation: crate::ctrl::PlaybackGeneration,
        resume_ticks: Option<i64>,
        mpv: &Mpv,
    ) {
        let slot_ids = self
            .queue
            .slots()
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>();
        let Some(idx) = resolve_jump_target(&slot_ids, slot_id) else {
            log::info!(
                target: "transition",
                "jump-to reject_stale_jump: slot_id={:?} unresolvable",
                slot_id,
            );
            reject_stale_jump(&self.event_tx, slot_id);
            return;
        };
        self.forced_transition = Some(crate::playback_transition::Transition::new(
            request_id, generation, slot_id,
        ));
        if self.active_file {
            self.cmd_jump_to_active_file(slot_id, mpv);
        } else {
            self.cmd_jump_to_playlist(slot_id, idx, resume_ticks, mpv);
        }
    }

    fn cmd_jump_to_active_file(&mut self, slot_id: QueueSlotId, mpv: &Mpv) {
        match self.select_active_slot(slot_id, mpv) {
            Ok(()) => {
                let _ = mpv.set_property("pause", false);
                // Active-file projection has no mpv playlist move to
                // observe, so the JumpTo emits its TrackChanged
                // observation here, shaped like the on_end_file settle
                // (design D1). The tag stays on `forced_transition` so
                // a duplicate settle path still carries it; the
                // pipeline treats the second attempt as Ignored.
                self.emit_track_changed(slot_id, self.forced_transition);
            }
            Err(error) => {
                log::warn!(target: "player", "active-file selection failed: {error}");
            }
        }
    }

    fn cmd_jump_to_playlist(
        &mut self,
        slot_id: QueueSlotId,
        idx: usize,
        resume_ticks: Option<i64>,
        mpv: &Mpv,
    ) {
        // mpv playlist indices are adapter coordinates; pin the
        // target slot identity before asking mpv to move. Idle jumps
        // settle on PlaybackRestart because no outgoing EndFile exists.
        self.forced_jump_from_idle = !self.status.lock().unwrap().active;
        if self.forced_jump_from_idle {
            self.tracks_initialized = false;
        }
        self.forced_slot_id = Some(slot_id);
        self.forced_resume_ticks = resume_ticks;
        if let Err(e) = mpv.set_property("playlist-pos", idx as i64) {
            self.forced_slot_id = None;
            self.forced_jump_from_idle = false;
            self.forced_transition = None;
            self.forced_resume_ticks = None;
            log::warn!(target: "player", "jump-to idx={idx} failed: {}", mpv_err_str(&e));
            return;
        }
        log::info!(
            target: "transition",
            "jump-to: playlist-pos ok slot_id={:?} idx={} current_idx={} queue_len={}",
            slot_id,
            idx,
            self.current_idx,
            self.queue_len(),
        );
        if self.forced_jump_from_idle {
            self.play_from_idle_playlist(idx, mpv);
        }
        // Selecting a track should always start it playing, even if
        // mpv was paused on the previous track — otherwise the new
        // track loads silently "stuck" paused (see issue: Enter on a
        // queue item, or a remote Next/Previous command, while paused).
        let _ = mpv.set_property("pause", false);
    }

    fn play_from_idle_playlist(&self, idx: usize, mpv: &Mpv) {
        if let Err(error) = mpv.command("playlist-play-index", &[&idx.to_string()]) {
            log::warn!(target: "player", "jump-to playlist-play-index={idx} failed: {}", mpv_err_str(&error));
        }
    }

    /// Remove an owner-assigned slot; a stale slot (already gone here) is
    /// discarded.
    fn cmd_queue_remove(&mut self, slot_id: QueueSlotId, mpv: &Mpv) {
        let Some(idx) = self.queue.slot_index(slot_id) else {
            return;
        };
        let active_slot_id = self.active_slot_id();
        if self.active_file {
            self.remove_active_file_slot(slot_id, idx, active_slot_id, mpv);
            return;
        }
        let _ = mpv.command("playlist-remove", &[&idx.to_string()]);
        if active_slot_id == Some(slot_id) {
            self.close_prepared_source();
            self.queue.remove_active_slot_confirmed(slot_id);
        } else {
            self.queue.remove_slot(slot_id);
        }
        self.sync_status_position();
        if self.forced_slot_id == Some(slot_id) {
            self.forced_slot_id = None;
            self.forced_transition = None;
            self.forced_resume_ticks = None;
        }
        if active_slot_id != Some(slot_id) {
            return;
        }
        // Currently playing track removed — clear reporter item_id to prevent
        // stale progress reports until on_end_file transitions to the next track.
        self.reporter.ids.lock().unwrap().0.clear();
    }

    fn remove_active_file_slot(
        &mut self,
        slot_id: QueueSlotId,
        idx: usize,
        active_slot_id: Option<QueueSlotId>,
        mpv: &Mpv,
    ) {
        if active_slot_id != Some(slot_id) {
            self.queue.remove_slot(slot_id);
            // active_file mode gets no mpv playlist-pos event to
            // self-correct the coordinate, so re-derive the
            // ordinal from the still-active slot's identity
            // (identity -> ordinal is permitted, design D2).
            self.current_idx = self
                .active_slot_id()
                .and_then(|s| self.queue.slot_index(s))
                .unwrap_or(self.current_idx);
            self.sync_status_position();
            return;
        }
        let Some(next) = self.remove_neighbor_slot(idx) else {
            self.close_prepared_source();
            self.queue.remove_active_slot_confirmed(slot_id);
            let _ = mpv.command("playlist-clear", &[]);
            self.sync_status_position();
            return;
        };
        if self.select_active_slot(next, mpv).is_ok() {
            self.queue.remove_slot(slot_id);
        }
        self.sync_status_position();
    }

    fn remove_neighbor_slot(&self, idx: usize) -> Option<QueueSlotId> {
        if idx + 1 < self.queue_len() {
            return self.slot_id_at(idx + 1);
        }
        if idx > 0 {
            return self.slot_id_at(idx - 1);
        }
        None
    }

    fn cmd_queue_move(&mut self, slot_id: QueueSlotId, to: usize, mpv: &Mpv) {
        let Some(from) = self.queue.slot_index(slot_id) else {
            return;
        };
        if from >= self.queue_len() || to >= self.queue_len() || from == to {
            return;
        }
        // mpv's playlist-move index2 names the *pre-move* slot the
        // entry should end up next to, not its post-move index: for
        // from < to the entry actually lands at to - 1, not to (mpv
        // manual's own "paradox" note, confirmed against mpv 0.41).
        // Passing to + 1 (one past the end when to == n - 1, which
        // mpv also accepts as "move to end") makes mpv's result
        // match this struct's from/to bookkeeping below.
        if !self.active_file {
            let mpv_to = if from < to { to + 1 } else { to };
            let _ = mpv.command("playlist-move", &[&from.to_string(), &mpv_to.to_string()]);
        }
        let _ = self.queue.move_slot(slot_id, to);
        // `current_idx` is this run's mpv-local coordinate; adjust it
        // for the move directly (design D2) rather than recomputing
        // it from the observed active slot.
        self.current_idx = shift_index_for_move(self.current_idx, from, to);
        self.sync_status_position();
    }

    /// Relative single-step nav (`PlayerCommand::Next`/`Previous`) for an
    /// already-bounds-checked target ordinal. Mirrors the `JumpTo` move minus
    /// request identity: no `forced_transition` (design D4 — relative nav
    /// correlates like natural advancement). Still pins `forced_slot_id` so the
    /// resulting mpv observation is attributed to the right slot.
    fn step_to_index(&mut self, idx: usize, mpv: &Mpv) {
        let Some(slot_id) = self.slot_id_at(idx) else {
            return;
        };
        if self.active_file {
            if let Err(error) = self.select_active_slot(slot_id, mpv) {
                log::warn!(target: "player", "active-file step to idx={idx} failed: {error}");
            } else {
                let _ = mpv.set_property("pause", false);
            }
            return;
        }
        // Unlike JumpTo (which gets the target's resume position from the
        // owner's canonical queue via the command itself), relative nav is
        // resolved entirely locally — this run's own queue mirror is now kept
        // current for exactly this (see `on_end_file`'s `apply_progress`
        // call), so the same per-kind gate can be evaluated straight off it.
        let resume_ticks = self
            .queue
            .slot(slot_id)
            .and_then(|slot| crate::player::resume_ticks_for_item(&slot.item));
        self.forced_slot_id = Some(slot_id);
        self.forced_resume_ticks = resume_ticks;
        if let Err(e) = mpv.set_property("playlist-pos", idx as i64) {
            self.forced_slot_id = None;
            self.forced_resume_ticks = None;
            log::warn!(target: "player", "step to idx={idx} failed: {}", mpv_err_str(&e));
        } else {
            let _ = mpv.set_property("pause", false);
        }
    }

    pub(in crate::player) fn append_items_to_queue(&mut self, items: Vec<ExecSlot>) {
        for slot in items {
            self.queue.append_with_id(slot.slot_id, slot.item);
        }
        self.status.lock().unwrap().queue_len = self.queue_len();
    }

    fn cmd_append_queue(&mut self, new_items: Vec<ExecSlot>, mpv: &Mpv) {
        if new_items.is_empty() {
            return;
        }

        if self.active_file {
            self.append_items_to_queue(new_items);
            return;
        }
        if new_items
            .iter()
            .any(|slot| slot.item.is_audiobookshelf_any())
        {
            let Some(active_item) = self.active_item().cloned() else {
                return;
            };
            let prepared = match self.prepare_item(&active_item) {
                Ok(prepared) => prepared,
                Err(error) => {
                    log::warn!(target: "player", "active-file transition failed: {error}");
                    return;
                }
            };
            if let Err(error) = self.install_active_projection(mpv, prepared, &active_item) {
                log::warn!(target: "player", "active-file transition failed: {error}");
                return;
            }
            self.append_items_to_queue(new_items);
            self.active_file = true;
            return;
        }
        for slot in &new_items {
            let url = mpv_url_for_queue_item(&slot.item, &self.server_url, &self.token);
            let opts = mpv_load_opts(&slot.item);
            if let Err(e) = mpv.command(
                "loadfile",
                &[url.as_str(), "append-play", "-1", opts.as_str()],
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
        item: Box<EmbyItem>,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        self.cancel_pending_quit();
        self.origin = PlaybackOrigin::Standalone;
        self.close_prepared_source();
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

        // ponytail: run-side slot-id minting. PlayerCommand::LoadNew has had no
        // in-process constructor since task 1.5 dropped its wire command; the
        // variant and this dead path go together in task 5.1.
        let slot_id = QueueSlotId::from_raw(1);
        self.queue = ExecutionSequence::from_slot_items(
            vec![(slot_id, QueueItem::Emby(Box::new(item.as_ref().clone())))],
            Some(slot_id),
        );
        self.current_idx = 0;
        self.load_active_item_state();
        self.stop_report = StopReport::NotSent;
        self.load_state = LoadState::begin_single();
        self.pending_initial_playlist_layout = false;
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

    /// Item-generic queue submission: replace the current queue with `items`
    /// and start playback from `start_idx`. Handles both Emby and Feed items
    /// through the same lifecycle — source URL and reporting branch on
    /// `QueueItem` variant; everything else is shared.
    ///
    /// Single-item sets `PlaybackOrigin::Standalone`; multi-item sets `Queue`.
    fn cmd_submit_queue(
        &mut self,
        items: Vec<ExecSlot>,
        start_idx: usize,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        if self.active_file || items.iter().any(|slot| slot.item.is_audiobookshelf_any()) {
            self.replace_with_queue_items(items, start_idx, mpv, progress);
            return;
        }
        self.cancel_pending_quit();
        if items.is_empty() {
            return;
        }
        let start_idx = start_idx.min(items.len() - 1);
        self.close_prepared_source();

        // Loading new items should always start playing, even if mpv was
        // left paused on the previous item (reused-window fast path).
        let _ = mpv.set_property("pause", false);

        // Determine origin from queue size: single item = Standalone, multi = Queue.
        let origin = if items.len() == 1 {
            PlaybackOrigin::Standalone
        } else {
            PlaybackOrigin::Queue
        };

        let had_previous_queue = self.queue_len() > 0;
        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
        let _ = mpv.command("script-message", &["mbv-next-up-dismiss"]);
        // Design D3 load-then-play: stop the previous playback and clear the
        // whole playlist first — `playlist-clear` keeps the currently played
        // file, which would survive a no-play load plan as a stray entry.
        let _ = mpv.command("stop", &[]);
        for i in queue_load_indices(items.len(), start_idx) {
            let item = &items[i].item;
            let url = mpv_url_for_queue_item(item, &self.server_url, &self.token);
            let (mode, index) = queue_load_location(i, start_idx);
            let opts = mpv_load_opts(item);
            if let Err(e) = mpv.command("loadfile", &[url.as_str(), mode, &index, &opts]) {
                log::warn!(
                    target: "player",
                    "SubmitQueue loadfile error: {e} | mode={mode} opts={opts:?}",
                );
            }
        }
        // Design D3: every load above was no-play, so playback starts here,
        // at the fully built playlist's start slot — reassert below must now
        // observe Ok (a mismatch log means the no-play plan drifted).
        start_queue_playback(mpv, start_idx);
        reassert_queue_layout(mpv, start_idx, items.len());

        let active_item = &items[start_idx].item;
        if let Some(emby) = active_item.as_emby() {
            send_ep_info(mpv, emby);
        }
        self.origin = origin;
        let active_runtime = active_item.runtime_ticks();
        let active_pos = active_item.playback_position_ticks();
        let active_as_emby = active_item.as_emby().cloned();
        let active_guid = active_item.id().to_string();
        let active_title = active_item.title().to_string();
        let active_slot_id = items.get(start_idx).map(|slot| slot.slot_id);
        self.queue = ExecutionSequence::from_slot_items(
            items
                .into_iter()
                .map(|slot| (slot.slot_id, slot.item))
                .collect(),
            active_slot_id,
        );
        self.current_idx = start_idx;
        self.load_active_item_state();
        self.begin_item_lifecycle();
        self.initialize_queue_start(
            had_previous_queue,
            active_as_emby.as_ref(),
            active_pos,
            active_runtime,
            active_title,
            active_guid,
            progress,
            true,
        );

        log::info!(
            target: "player",
            "SubmitQueue origin={origin:?} idx={start_idx} items={}",
            self.queue_len(),
        );
    }

    fn initialize_queue_start(
        &mut self,
        had_previous_queue: bool,
        active_as_emby: Option<&EmbyItem>,
        active_pos: i64,
        active_runtime: i64,
        active_title: String,
        active_guid: String,
        progress: &mut ProgressGuard,
        initialize_load_state: bool,
    ) {
        self.stop_report = if had_previous_queue {
            StopReport::mark_sent(self.reporter.report_stopped(self.last_valid_pos))
        } else {
            StopReport::NotSent
        };
        if initialize_load_state {
            self.load_state = LoadState::begin_single();
            self.pending_initial_playlist_layout = false;
            progress.stop_and_join(self.progress_join_budget());
        }
        if let Some(emby) = active_as_emby {
            let (urls, ok) = self.reporter.start_item(emby);
            self.ext_sub_urls = urls;
            if !ok {
                log::warn!(
                    target: "player",
                    "start_item failed for SubmitQueue item={}",
                    emby.id,
                );
            }
        } else {
            self.ext_sub_urls = vec![];
            self.reporter.clear_session();
        }
        *progress = spawn_progress_reporter(self.reporter.clone());
        let mut status = self.status.lock().unwrap();
        status.position_ticks = active_pos;
        status.runtime_ticks = active_runtime;
        status.current_idx = self.current_idx;
        status.queue_len = self.queue_len();
        if let Some(emby) = active_as_emby {
            status.set_current_item_metadata(emby);
        } else {
            status.title = active_title;
            status.art_item_id = active_guid;
        }
    }

    fn replace_with_queue_items(
        &mut self,
        items: Vec<ExecSlot>,
        start_idx: usize,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        self.cancel_pending_quit();
        if items.is_empty() {
            self.close_prepared_source();
            let _ = mpv.command("playlist-clear", &[]);
            self.queue.clear();
            self.current_idx = 0;
            self.sync_status_position();
            return;
        }
        let start_idx = start_idx.min(items.len() - 1);
        let active_item = items[start_idx].item.clone();
        let prepared = match self.prepare_item(&active_item) {
            Ok(prepared) => prepared,
            Err(error) => {
                log::warn!(target: "player", "active-file replacement preparation failed: {error}");
                self.accept_stopped_replacement(
                    items,
                    start_idx,
                    &active_item,
                    mpv,
                    progress,
                    format!("failed to prepare media: {error}"),
                );
                return;
            }
        };

        let had_previous_queue = self.queue_len() > 0;
        progress.stop_and_join(self.progress_join_budget());
        let active_slot_id = items.get(start_idx).map(|slot| slot.slot_id);
        self.queue = ExecutionSequence::from_slot_items(
            items
                .into_iter()
                .map(|slot| (slot.slot_id, slot.item))
                .collect(),
            active_slot_id,
        );
        self.current_idx = start_idx;
        self.active_file = true;
        if let Err(error) = self.install_active_projection(mpv, prepared, &active_item) {
            log::warn!(target: "player", "active-file replacement failed: {error}");
            self.accept_stopped_replacement(
                Vec::new(),
                start_idx,
                &active_item,
                mpv,
                progress,
                format!("failed to load media: {error}"),
            );
            return;
        }
        self.origin = if self.queue_len() == 1 {
            PlaybackOrigin::Standalone
        } else {
            PlaybackOrigin::Queue
        };
        self.load_active_item_state();
        self.begin_item_lifecycle();
        self.initialize_queue_start(
            had_previous_queue,
            active_item.as_emby(),
            active_item.playback_position_ticks(),
            active_item.runtime_ticks(),
            active_item.title().to_string(),
            active_item.id().to_string(),
            progress,
            false,
        );
        self.status.lock().unwrap().active = true;
    }

    fn accept_stopped_replacement(
        &mut self,
        items: Vec<ExecSlot>,
        start_idx: usize,
        active_item: &QueueItem,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
        error: String,
    ) {
        let old_pos = self.last_valid_pos;
        progress.stop_and_join(self.progress_join_budget());
        if self.stop_report == StopReport::NotSent {
            self.stop_report = StopReport::mark_sent(self.reporter.report_stopped(old_pos));
        }
        self.close_prepared_source_at(old_pos);
        let _ = mpv.command("stop", &[]);
        let _ = mpv.command("playlist-clear", &[]);

        if !items.is_empty() {
            let active_slot_id = items.get(start_idx).map(|slot| slot.slot_id);
            self.queue = ExecutionSequence::from_slot_items(
                items
                    .into_iter()
                    .map(|slot| (slot.slot_id, slot.item))
                    .collect(),
                active_slot_id,
            );
        }
        self.current_idx = start_idx;
        self.active_file = true;
        self.origin = if self.queue_len() == 1 {
            PlaybackOrigin::Standalone
        } else {
            PlaybackOrigin::Queue
        };
        self.load_active_item_state();
        self.begin_item_lifecycle();
        self.active_file_starting = false;
        self.load_state = LoadState::begin_single();
        self.pending_initial_playlist_layout = false;
        self.ext_sub_urls.clear();
        self.reporter.clear_session();

        let position_ticks = active_item.playback_position_ticks();
        let mut status = self.status.lock().unwrap();
        status.position_ticks = position_ticks;
        status.last_valid_pos = position_ticks;
        status.runtime_ticks = active_item.runtime_ticks();
        status.current_idx = start_idx;
        status.queue_len = self.queue_len();
        if let Some(emby) = active_item.as_emby() {
            status.set_current_item_metadata(emby);
        } else {
            status.clear_current_item_metadata();
            status.title = active_item.title().to_string();
            status.art_item_id = active_item.id().to_string();
        }
        status.active = false;
        drop(status);

        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: self.active_slot_id(),
            run_identity: self.run_identity,
            position_ticks,
            played: false,
            consume: false,
            progress_report_accepted: false,
            error: Some(error),
        });
    }
}
