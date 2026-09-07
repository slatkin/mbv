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
            PlayerCommand::JumpTo {
                slot_id,
                request_id,
                generation,
            } => {
                // Resolve the owner-assigned slot to this run's mpv-local
                // ordinal; a stale slot (gone here) is rejected, never
                // repaired by position (design D6).
                let Some(idx) = self.queue.slot_index(slot_id) else {
                    log::debug!(target: "player", "jump-to: stale slot {slot_id:?} absent; discarded");
                    return cancel_stop;
                };
                self.forced_transition =
                    Some(crate::playback_transition::Transition::new(request_id, generation, slot_id));
                if self.active_file {
                    if let Err(error) = self.select_active_slot(slot_id, mpv) {
                        log::warn!(target: "player", "active-file selection failed: {error}");
                    } else {
                        let _ = mpv.set_property("pause", false);
                    }
                    return cancel_stop;
                }
                // mpv playlist indices are adapter coordinates; pin the
                // target slot identity before asking mpv to move.
                self.forced_slot_id = Some(slot_id);
                if let Err(e) = mpv.set_property("playlist-pos", idx as i64) {
                    self.forced_slot_id = None;
                    self.forced_transition = None;
                    log::warn!(target: "player", "jump-to idx={idx} failed: {}", mpv_err_str(&e));
                } else {
                    // Selecting a track should always start it playing, even if
                    // mpv was paused on the previous track — otherwise the new
                    // track loads silently "stuck" paused (see issue: Enter on a
                    // queue item, or a remote Next/Previous command, while paused).
                    let _ = mpv.set_property("pause", false);
                }
            }
            PlayerCommand::Next => {
                let target = self.current_idx + 1;
                if target < self.queue_len() {
                    self.step_to_index(target, mpv);
                }
            }
            PlayerCommand::Previous => {
                if let Some(target) = self.current_idx.checked_sub(1) {
                    self.step_to_index(target, mpv);
                }
            }
            PlayerCommand::QueueAppend { items } => {
                self.cmd_append_queue(items, mpv);
            }
            PlayerCommand::QueueRemove(slot_id) => {
                // Resolve the owner-assigned slot to this run's mpv-local
                // ordinal; a stale slot (already gone here) is discarded.
                if let Some(idx) = self.queue.slot_index(slot_id) {
                    let active_slot_id = self.active_slot_id();
                    if self.active_file {
                        let active = active_slot_id == Some(slot_id);
                        if active {
                            let next = if idx + 1 < self.queue_len() {
                                self.slot_id_at(idx + 1)
                            } else if idx > 0 {
                                self.slot_id_at(idx - 1)
                            } else {
                                None
                            };
                            if let Some(next) = next {
                                if self.select_active_slot(next, mpv).is_err() {
                                    return cancel_stop;
                                }
                                self.queue.remove_slot(slot_id);
                            } else {
                                self.close_prepared_source();
                                self.queue.remove_active_slot_confirmed(slot_id);
                                let _ = mpv.command("playlist-clear", &[]);
                            }
                            self.sync_status_position();
                        } else {
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
                        }
                        return cancel_stop;
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
                    }
                    if active_slot_id == Some(slot_id) {
                        // Currently playing track removed — clear reporter item_id to prevent
                        // stale progress reports until on_end_file transitions to the next track.
                        let mut ids = self.reporter.ids.lock().unwrap();
                        ids.0.clear();
                    }
                }
            }
            PlayerCommand::QueueMove(slot_id, to) => {
                let from = match self.queue.slot_index(slot_id) {
                    Some(from) => from,
                    None => return cancel_stop,
                };
                if from < self.queue_len() && to < self.queue_len() && from != to {
                    // mpv's playlist-move index2 names the *pre-move* slot the
                    // entry should end up next to, not its post-move index: for
                    // from < to the entry actually lands at to - 1, not to (mpv
                    // manual's own "paradox" note, confirmed against mpv 0.41).
                    // Passing to + 1 (one past the end when to == n - 1, which
                    // mpv also accepts as "move to end") makes mpv's result
                    // match this struct's from/to bookkeeping below.
                    if !self.active_file {
                        let mpv_to = if from < to { to + 1 } else { to };
                        let _ =
                            mpv.command("playlist-move", &[&from.to_string(), &mpv_to.to_string()]);
                    }
                    let _ = self.queue.move_slot(slot_id, to);
                    // `current_idx` is this run's mpv-local coordinate; adjust it
                    // for the move directly (design D2) rather than recomputing
                    // it from the observed active slot.
                    self.current_idx = shift_index_for_move(self.current_idx, from, to);
                    self.sync_status_position();
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
            PlayerCommand::SubmitQueue { items, start_idx } => {
                self.cmd_submit_queue(items, start_idx, mpv, progress);
                cancel_stop = true;
            }
        }
        cancel_stop
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
        self.forced_slot_id = Some(slot_id);
        if let Err(e) = mpv.set_property("playlist-pos", idx as i64) {
            self.forced_slot_id = None;
            log::warn!(target: "player", "step to idx={idx} failed: {}", mpv_err_str(&e));
        } else {
            let _ = mpv.set_property("pause", false);
        }
    }

    fn cmd_replace_queue(
        &mut self,
        new_items: Vec<EmbyItem>,
        start_idx: usize,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        if self.active_file {
            // ponytail: run-side slot-id minting (1..=len). Still reachable via
            // the in-process PlayerCommand::ReplaceQueue callers in src/app/;
            // removed with the ReplaceQueue variant itself in task 5.1.
            let paired = new_items
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    (
                        QueueSlotId::from_raw(i as u64 + 1),
                        QueueItem::Emby(Box::new(item)),
                    )
                })
                .collect();
            self.replace_with_queue_items(paired, start_idx, mpv, progress);
            return;
        }
        self.cancel_pending_quit();
        if new_items.is_empty() {
            self.close_prepared_source();
            self.stop_report =
                StopReport::mark_sent(self.reporter.report_stopped(self.last_valid_pos));
            let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
            let _ = mpv.command("playlist-clear", &[]);
            self.origin = PlaybackOrigin::Queue;
            self.queue = ExecutionSequence::empty();
            self.current_idx = 0;
            self.sync_status_position();
            self.last_valid_pos = 0;
            self.pending_initial_playlist_layout = false;
            self.load_state = LoadState::Ready;
            self.begin_item_lifecycle();
            self.osd_title.clear();
            self.series_id.clear();
            self.season = 0;
            self.episode = 0;
            return;
        }
        self.close_prepared_source();
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
        let active_item = new_items[start_idx].clone();
        for i in queue_load_indices(new_items.len(), start_idx) {
            let item = &new_items[i];
            let queue_item = QueueItem::Emby(Box::new(item.clone()));
            let url = mpv_url_for_queue_item(&queue_item, &self.server_url, &self.token);
            let (mode, index) = queue_load_location(i, start_idx);
            let opts = mpv_load_opts(&queue_item);
            if let Err(e) = mpv.command("loadfile", &[url.as_str(), mode, &index, &opts]) {
                log::warn!(target: "player", "ReplaceQueue loadfile error: {}", mpv_err_str(&e));
            }
        }
        send_ep_info(mpv, &active_item);
        // loadfile "replace" displaces the current file (EndFile #1).
        self.load_state = LoadState::begin_single();
        self.pending_initial_playlist_layout = false;

        self.origin = PlaybackOrigin::Queue;
        // ponytail: run-side slot-id minting (1..=len); removed with the
        // PlayerCommand::ReplaceQueue variant in task 5.1.
        let paired: Vec<(QueueSlotId, QueueItem)> = new_items
            .into_iter()
            .enumerate()
            .map(|(i, item)| {
                (
                    QueueSlotId::from_raw(i as u64 + 1),
                    QueueItem::Emby(Box::new(item)),
                )
            })
            .collect();
        let active_slot_id = paired.get(start_idx).map(|(id, _)| *id);
        self.queue = ExecutionSequence::from_slot_items(paired, active_slot_id);
        self.current_idx = start_idx;
        self.load_active_item_state();
        // stop_report stays Sent until load_state drains to Ready in on_end_file,
        // preventing a duplicate report_stopped for the displaced file's EndFile(Quit).
        self.begin_item_lifecycle();
        log::info!(target: "player", "playlist queue-replace idx={start_idx}");
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

    fn append_items_to_queue(&mut self, items: Vec<(QueueSlotId, QueueItem)>) {
        for (slot_id, item) in items {
            self.queue.append_with_id(slot_id, item);
        }
        self.status.lock().unwrap().queue_len = self.queue_len();
    }

    fn cmd_append_queue(&mut self, new_items: Vec<(QueueSlotId, QueueItem)>, mpv: &Mpv) {
        if new_items.is_empty() {
            return;
        }

        if self.active_file {
            self.append_items_to_queue(new_items);
            return;
        }
        if new_items.iter().any(|(_, i)| i.is_audiobookshelf_any()) {
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
        for (_, item) in &new_items {
            let url = mpv_url_for_queue_item(item, &self.server_url, &self.token);
            let opts = mpv_load_opts(item);
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
        items: Vec<(QueueSlotId, QueueItem)>,
        start_idx: usize,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        if self.active_file || items.iter().any(|(_, i)| i.is_audiobookshelf_any()) {
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
        let _ = mpv.command("playlist-clear", &[]);
        for i in queue_load_indices(items.len(), start_idx) {
            let item = &items[i].1;
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

        let active_item = &items[start_idx].1;
        if let Some(emby) = active_item.as_emby() {
            send_ep_info(mpv, emby);
        }
        self.origin = origin;
        let active_runtime = active_item.runtime_ticks();
        let active_pos = active_item.playback_position_ticks();
        let active_as_emby = active_item.as_emby().cloned();
        let active_guid = active_item.id().to_string();
        let active_title = active_item.title().to_string();
        let active_slot_id = items.get(start_idx).map(|(id, _)| *id);
        self.queue = ExecutionSequence::from_slot_items(items, active_slot_id);
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
        }
        if initialize_load_state {
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
        items: Vec<(QueueSlotId, QueueItem)>,
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
        let active_item = items[start_idx].1.clone();
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
        let active_slot_id = items.get(start_idx).map(|(id, _)| *id);
        self.queue = ExecutionSequence::from_slot_items(items, active_slot_id);
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
        items: Vec<(QueueSlotId, QueueItem)>,
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
            let active_slot_id = items.get(start_idx).map(|(id, _)| *id);
            self.queue = ExecutionSequence::from_slot_items(items, active_slot_id);
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
            position_ticks,
            played: false,
            consume: false,
            progress_report_accepted: false,
            error: Some(error),
        });
    }
}

/// Constructs the mpv loadfile URL for a `QueueItem`.
/// - Emby: the standard Emby streaming URL.
/// - Feed: the enclosure/link URL handed directly to mpv.
/// - Audiobookshelf: not yet playable; returns empty (will fail visibly
///   rather than crash; owner admission will reject before this path).
fn mpv_url_for_queue_item(item: &QueueItem, server_url: &str, token: &str) -> String {
    match item {
        QueueItem::Emby(emby) => {
            let ep = if emby.is_audio() { "Audio" } else { "Videos" };
            format!(
                "{}/{}/{}/stream?static=true&api_key={}",
                server_url, ep, emby.id, token
            )
        }
        QueueItem::Feed(entry) => entry.primary_source().unwrap_or("").to_string(),
        QueueItem::Audiobookshelf(_) => {
            unreachable!("Audiobookshelf admission must precede URL resolution")
        }
        QueueItem::AudiobookshelfBook(_) => {
            unreachable!("Audiobookshelf book admission must precede URL resolution")
        }
    }
}
