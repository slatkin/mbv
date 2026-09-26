use super::*;

impl PlaybackRun {
    pub(super) fn cmd_load_new(
        &mut self,
        url: &str,
        start_pos: f64,
        item: &EmbyItem,
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
        progress.stop_and_join(Self::progress_join_budget());
        if self.config.audio_pipe_path.is_some() {
            self.reporter
                .transition_to_deferred(item, self.last_valid_pos);
            self.ext_sub_urls = vec![];
        } else {
            self.ext_sub_urls = self.reporter.transition_to(item, self.last_valid_pos);
        }
        *progress = spawn_progress_reporter(self.reporter.clone());

        // ponytail: run-side slot-id minting. PlayerCommand::LoadNew has had no
        // in-process constructor since task 1.5 dropped its wire command; the
        // variant and this dead path go together in task 5.1.
        let slot_id = QueueSlotId::from_raw(1);
        self.queue = ExecutionSequence::from_slot_items(
            vec![(slot_id, QueueItem::Emby(Box::new(item.clone())))],
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
            st.set_current_item_metadata(item);
        };

        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
        let _ = mpv.command("script-message", &["mbv-next-up-dismiss"]);

        if start_pos > 0.0 {
            let _ = mpv.set_property("start", format!("{start_pos:.0}"));
        } else {
            let _ = mpv.set_property("start", "0");
        }
        let title_opt = mpv_title_opt(&item.display_name());
        log::info!(target: "player", "loadfile url={url} opts={title_opt:?}");
        if let Err(e) = mpv.command("loadfile", &[url, "replace", "-1", title_opt.as_str()]) {
            log::warn!(target: "player", "loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e));
        }
        send_ep_info(mpv, item);
    }

    /// Item-generic queue submission: replace the current queue with `items`
    /// and start playback from `start_idx`. Handles both Emby and Feed items
    /// through the same lifecycle — source URL and reporting branch on
    /// `QueueItem` variant; everything else is shared.
    ///
    /// Single-item sets `PlaybackOrigin::Standalone`; multi-item sets `Queue`.
    pub(super) fn cmd_submit_queue(
        &mut self,
        items: Vec<ExecSlot>,
        start_idx: usize,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        if self.active_file || items.iter().any(|slot| slot.item.is_audiobookshelf()) {
            self.replace_with_queue_items(items, start_idx, mpv, progress);
            return;
        }
        let Some(sources) = items
            .iter()
            .map(|slot| slot.item.mpv_url_source())
            .collect::<Option<Vec<_>>>()
        else {
            let reason = "Queue submission rejected: item has no direct mpv URL source".to_string();
            log::warn!(target: "player", "{reason}");
            let _ = self.event_tx.send(PlayerEvent::CommandRejected(reason));
            return;
        };
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
            let url = mpv_url_for_queue_item(sources[i], &self.server_url, &self.token);
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
            progress.stop_and_join(Self::progress_join_budget());
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
        progress.stop_and_join(Self::progress_join_budget());
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
        progress.stop_and_join(Self::progress_join_budget());
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
