impl PlaybackRun {
    /// The ordinal a relative step (Next/Previous) advances from: the active
    /// slot's identity resolved to this run's ordinal (the permitted
    /// identity -> ordinal direction, design D2), falling back to the run
    /// coordinate when the sequence holds no active marker. Deriving the
    /// neighbor from `current_idx` alone re-used a coordinate that a
    /// settling observation can briefly outrun.
    fn relative_step_base(&self) -> usize {
        self.active_slot_id()
            .and_then(|id| self.queue.slot_index(id))
            .unwrap_or(self.current_idx)
    }

    fn queue_len(&self) -> usize {
        self.queue.slots().len()
    }

    fn slot_id_at(&self, idx: usize) -> Option<QueueSlotId> {
        self.queue.slots().get(idx).map(|slot| slot.slot_id)
    }

    fn item_at(&self, idx: usize) -> Option<&QueueItem> {
        self.queue.slots().get(idx).map(|slot| &slot.item)
    }

    fn active_item(&self) -> Option<&QueueItem> {
        self.queue.active_slot().map(|slot| &slot.item)
    }

    fn active_slot_id(&self) -> Option<QueueSlotId> {
        self.queue.active_slot_id()
    }

    fn report_stopped_for_current_context(&self) -> bool {
        if let Some(timeout) = *self.shutdown_report_timeout.lock().unwrap() {
            self.reporter
                .report_stopped_for_shutdown(self.last_valid_pos, timeout)
        } else {
            self.reporter.report_stopped(self.last_valid_pos)
        }
    }

    /// True once `Player::stop_for_shutdown` has armed a deadline — a real
    /// quit (app close or daemon teardown), as opposed to an ordinary track
    /// transition. Gates whether `report_stop_now_or_background` can afford
    /// to fire the stop report on a background thread: mid-playback there's
    /// no rush, but once the process is on its way out a detached thread
    /// might never get to run before exit.
    fn is_quit_shutdown(&self) -> bool {
        self.shutdown_report_timeout.lock().unwrap().is_some()
    }

    /// Reports the current item as stopped either synchronously (real quit,
    /// where the report must complete before the process exits) or on a
    /// background thread (ordinary stop, kept off the critical path so the
    /// UI/mpv can proceed immediately). Callers must still guard on
    /// `self.stop_report` before calling this.
    fn report_stop_now_or_background(&mut self, progress: &mut ProgressGuard) {
        let _ = progress.stop_tx.send(());
        if self.is_quit_shutdown() {
            self.stop_report = StopReport::mark_sent(self.report_stopped_for_current_context());
        } else {
            let handle = progress.handle.take();
            let budget = self.progress_join_budget();
            let stopped = self.reporter.stopped_report_data(self.last_valid_pos);
            let _ = self.reporter.job_tx.send(ReportJob::ProgressJoinThenStopped {
                handle,
                budget,
                stopped,
            });
            // Fire-and-forget: we can't know synchronously whether Emby accepted
            // this. Treat it as accepted anyway so mark_progress_sync_pending
            // still protects the just-saved local position from being overwritten
            // by a queue refresh that lands before the background call completes.
            // If the call *does* fail, the slot's pending_sync just never gets
            // confirmed and stays protected — the safe failure mode.
            self.stop_report = StopReport::Accepted;
        }
    }

    fn report_stopped_for_end_file(&self, reason: EndFileReason) -> bool {
        match end_file_stop_report_context(reason) {
            StopReportContext::Ordinary => self.reporter.report_stopped(self.last_valid_pos),
            StopReportContext::ShutdownAware => self.report_stopped_for_current_context(),
        }
    }

    /// Budget for `ProgressGuard::stop_and_join` during ordinary track
    /// transitions. There is no time pressure, so a generous fixed budget
    /// just guards against a truly stuck thread without adding latency to
    /// the common fast case.
    fn progress_join_budget(&self) -> Duration {
        Duration::from_secs(30)
    }

    /// Clears any pending-quit state so a `LoadNew`/`SubmitQueue` command
    /// that arrives while a quit is in flight fully cancels it — not just
    /// `quit_at`, but also the shutdown-scoped report context set by
    /// `Player::stop_for_shutdown`. Without resetting
    /// `shutdown_report_timeout` here too, a cancelled quit would leave it
    /// `Some` for the rest of this `PlaybackRun`'s lifetime (nothing
    /// else clears it once set), so every subsequent track transition
    /// would silently keep using the shutdown report behavior from
    /// `report_stopped_for_current_context` instead of the ordinary one —
    /// no crash, just quietly degraded reliability for the rest of the
    /// session.
    fn cancel_pending_quit(&mut self) {
        self.quit_at = None;
        self.stop_slot = None;
        self.stop_runtime = None;
        *self.shutdown_report_timeout.lock().unwrap() = None;
    }

    fn sync_status_position(&self) {
        let mut s = self.status.lock().unwrap();
        s.current_idx = self.current_idx;
        s.queue_len = self.queue_len();
    }

    fn observe_reporting(&mut self, force_sync: bool) {
        let (active, paused, position_ticks) = {
            let status = self.status.lock().unwrap();
            (status.active, status.paused, status.position_ticks)
        };
        let now = Instant::now();
        self.active_lifecycle.observe(now, active && !paused);
        self.active_lifecycle.sync(position_ticks, now, force_sync);
    }

    fn set_active_index(&mut self, idx: usize) -> bool {
        let Some(slot_id) = self.slot_id_at(idx) else {
            return false;
        };
        if !self.queue.set_active_slot(slot_id) {
            return false;
        }
        self.current_idx = idx;
        self.sync_status_position();
        true
    }

    /// mpv's current entry, when it is not the one the run believes in — the
    /// only evidence that mpv navigated itself, since nothing asked it to.
    /// `None` for an `active_file` run: that projection keeps one entry in
    /// mpv's playlist, so `playlist-pos` never names the run's ordinal.
    /// mpv's raw `playlist-pos`, alongside the entry it names when that is
    /// not the run's active one — see `divergent_entry`. Returning the raw
    /// position too lets a caller log it without a second IPC round-trip.
    fn mpv_divergent_entry(&self, mpv: &Mpv) -> (i64, Option<usize>) {
        let pos = mpv.get_property::<i64>("playlist-pos").unwrap_or(-1);
        if self.active_file {
            return (pos, None);
        }
        (pos, divergent_entry(pos, self.current_idx, self.queue_len()))
    }

    /// Report the outgoing item stopped, then adopt the entry mpv moved to on
    /// its own — folded into one step so a caller can't tell Emby "stopped"
    /// without a completed adoption to back it up. Announces the observation
    /// only when adoption succeeds.
    fn report_stopped_and_adopt_mpv_entry(
        &mut self,
        abandoned_slot: Option<QueueSlotId>,
        abandoned_pos: i64,
        index: usize,
        mpv_pos_ticks: i64,
    ) -> bool {
        self.stop_report = StopReport::mark_sent(self.reporter.report_stopped(abandoned_pos));
        if !self.adopt_mpv_entry(index, mpv_pos_ticks) {
            return false;
        }
        let stop_accepted = self.stop_report.is_accepted();
        self.announce_adopted_entry(abandoned_slot, abandoned_pos, stop_accepted);
        true
    }

    /// Adopt an entry mpv moved to on its own as the active one, bringing the
    /// run's per-item state with it and re-pointing Emby reporting at the item
    /// actually playing, so the reported identity follows playback rather than
    /// the ordinal the run asked for. `mpv_pos_ticks` is mpv's position in the
    /// adopted entry (it is already playing, so the item's stored resume
    /// would misreport progress).
    fn adopt_mpv_entry(&mut self, index: usize, mpv_pos_ticks: i64) -> bool {
        if !self.set_active_index(index) {
            return false;
        }
        self.load_active_item_state();
        if mpv_pos_ticks > 0 {
            self.last_valid_pos = mpv_pos_ticks;
        }
        let Some(item) = self.active_item().cloned() else {
            return true;
        };
        {
            let mut s = self.status.lock().unwrap();
            s.position_ticks = mpv_pos_ticks.max(0);
            s.runtime_ticks = item.runtime_ticks();
            s.current_idx = self.current_idx;
            s.queue_len = self.queue_len();
            if let Some(emby) = item.as_emby() {
                s.set_current_item_metadata(emby);
            } else {
                s.title = item.title().to_string();
                s.art_item_id = item.id().to_string();
            }
        }
        self.report_active_item();
        // Tracks and next-up belong to the entry that just started.
        self.tracks_initialized = false;
        self.queue_next_up.reset();
        true
    }

    /// Announce an entry the run adopted because mpv moved to it on its own:
    /// the abandoned slot completed without finishing, and the adopted slot is
    /// the observation (no request identity — nothing asked for the move).
    fn announce_adopted_entry(
        &mut self,
        abandoned_slot: Option<QueueSlotId>,
        abandoned_pos: i64,
        stop_accepted: bool,
    ) {
        if let Some(slot_id) = abandoned_slot {
            let _ = self.event_tx.send(PlayerEvent::TrackCompleted {
                slot_id,
                position_ticks: abandoned_pos,
                played: false,
                consume: false,
                progress_report_accepted: stop_accepted,
            });
        }
        if let Some(slot_id) = self.active_slot_id() {
            let _ = self.event_tx.send(PlayerEvent::TrackChanged {
                slot_id,
                transition: None,
            });
        }
    }

    /// Report the active slot's item as started on Emby — or clear the Emby
    /// session for a non-Emby item — the same switch a track transition makes,
    /// so reporting identity always names the item playback is on.
    fn report_active_item(&mut self) {
        match self.active_item().cloned() {
            Some(QueueItem::Emby(emby)) => {
                let (urls, ok) = self.reporter.start_item(&emby);
                self.ext_sub_urls = urls;
                if !ok {
                    log::warn!(
                        target: "player",
                        "start_item failed for adopted item={}",
                        emby.id
                    );
                }
            }
            _ => {
                self.ext_sub_urls = vec![];
                self.reporter.clear_session();
            }
        }
    }

    fn prepare_item(&mut self, item: &QueueItem) -> Result<PreparedSource, AudiobookshelfError> {
        // A new Audiobookshelf source opens a server session during preparation.
        // Finalize the current lifecycle first so normal transitions cannot
        // overlap the outgoing and incoming sessions.
        self.close_prepared_source();
        prepare_source(
            item,
            &self.server_url,
            &self.token,
            self.audiobookshelf_context.as_ref(),
        )
    }

    fn install_active_projection(
        &mut self,
        mpv: &Mpv,
        mut prepared: PreparedSource,
        item: &QueueItem,
    ) -> Result<(), AudiobookshelfError> {
        let _ = mpv.command("playlist-clear", &[]);
        // One continuous timeline across the book's audio files, so chapter
        // rows can issue absolute seeks against the whole book. mpv merges the
        // playlisted files into a single edl:// entry only while this property
        // is set, so it is reset to no for every non-book load below.
        let _ = mpv.set_property(
            "merge-files",
            if prepared.merged_timeline {
                "yes"
            } else {
                "no"
            }
            .to_string(),
        );
        let options = prepared.mpv_load_options(item);
        if mpv
            .command(
                "loadfile",
                &[prepared.url.as_str(), "replace", "-1", options.as_str()],
            )
            .is_err()
        {
            self.close_prepared_source();
            prepared.close(0.0);
            self.status.lock().unwrap().active = false;
            return Err(AudiobookshelfError::from_class(
                AudiobookshelfFailureClass::Unavailable,
            ));
        }
        if prepared.merged_timeline {
            let extra_options = prepared.extra_source_options();
            for source in &prepared.book_extra_sources {
                if mpv
                    .command(
                        "loadfile",
                        &[
                            source.url.as_str(),
                            "append-play",
                            "-1",
                            extra_options.as_str(),
                        ],
                    )
                    .is_err()
                {
                    self.close_prepared_source();
                    prepared.close(0.0);
                    self.status.lock().unwrap().active = false;
                    return Err(AudiobookshelfError::from_class(
                        AudiobookshelfFailureClass::Unavailable,
                    ));
                }
            }
            // Resume position is an absolute seek on the merged timeline, so
            // it is unambiguous across file boundaries.
            if prepared.start_seconds > 0.0 {
                let _ = mpv.command("seek", &[&prepared.start_seconds.to_string(), "absolute"]);
            }
        }
        self.close_prepared_source();
        let prepared_lifecycle = prepared.take_lifecycle();
        self.prepared_source = Some(prepared);
        self.active_lifecycle = ActiveItemLifecycle::for_item(item, prepared_lifecycle);
        self.active_file_starting = true;
        self.load_state = LoadState::begin_single();
        self.pending_initial_playlist_layout = false;
        Ok(())
    }

    fn select_active_slot(
        &mut self,
        slot_id: QueueSlotId,
        mpv: &Mpv,
    ) -> Result<(), AudiobookshelfError> {
        let item = self
            .queue
            .slot(slot_id)
            .map(|slot| slot.item.clone())
            .ok_or_else(|| AudiobookshelfError::from_class(AudiobookshelfFailureClass::Protocol))?;
        let prepared = self.prepare_item(&item)?;
        self.install_active_projection(mpv, prepared, &item)?;
        let _ = self.queue.set_active_slot(slot_id);
        // Resolve the just-selected slot to this run's mpv-local coordinate
        // (command target -> ordinal is the permitted direction, design D2);
        // never recompute the coordinate from the observed active slot.
        self.current_idx = self.queue.slot_index(slot_id).unwrap_or(self.current_idx);
        self.sync_status_position();
        self.load_active_item_state();
        Ok(())
    }

    fn close_prepared_source(&mut self) {
        self.close_prepared_source_at(self.last_valid_pos);
    }

    fn close_prepared_source_at(&mut self, position_ticks: i64) {
        self.active_lifecycle.close(position_ticks);
        self.active_lifecycle = ActiveItemLifecycle::None;
        if let Some(mut prepared) = self.prepared_source.take() {
            prepared.close(position_ticks as f64 / TICKS_PER_SECOND as f64);
        }
    }

    fn reset_next_up_state(&mut self) {
        self.next_up.reset();
        self.queue_next_up.reset();
        self.next_up_jump = false;
    }

    /// Reset per-item lifecycle flags shared by all three reset sites in
    /// `player_run_commands.rs` (`cmd_submit_queue` empty, non-empty,
    /// and `cmd_load_new`). The caller must set `stop_report` and
    /// `load_state` itself because those differ per call site.
    fn begin_item_lifecycle(&mut self) {
        self.tracks_initialized = false;
        self.forced_slot_id = None;
        self.forced_transition = None;
        self.reset_next_up_state();
        self.stopped_event_sent = false;
        self.mark_played_id = None;
        self.stopped_near_end = false;
    }

    fn load_active_item_state(&mut self) {
        let Some(item) = self.active_item().cloned() else {
            self.osd_title.clear();
            self.last_valid_pos = 0;
            self.series_id.clear();
            self.season = 0;
            self.episode = 0;
            self.intro_start = 0;
            self.intro_end = 0;
            self.intro_state = IntroState::Pending;
            return;
        };

        match &item {
            QueueItem::Emby(emby) => {
                self.osd_title = emby.display_name();
                self.last_valid_pos = if emby.is_audio() {
                    0
                } else {
                    emby.playback_position_ticks
                };
                if emby.item_type == "Episode" {
                    self.series_id = ItemId::new(emby.series_id.clone());
                    self.season = emby.parent_index_number;
                    self.episode = emby.index_number;
                } else {
                    self.series_id.clear();
                    self.season = 0;
                    self.episode = 0;
                }
                self.set_intro(0, 0, emby.playback_position_ticks);
            }
            QueueItem::Feed(entry) => {
                self.osd_title = entry.title.clone();
                let runtime = entry.duration_ticks.unwrap_or(0) as i64;
                self.last_valid_pos = if crate::api::should_resume(entry.position_ticks, runtime) {
                    entry.position_ticks
                } else {
                    0
                };
                self.series_id.clear();
                self.season = 0;
                self.episode = 0;
                self.intro_start = 0;
                self.intro_end = 0;
                self.intro_state = IntroState::Pending;
            }
            QueueItem::Audiobookshelf(ep) => {
                self.osd_title = ep.title.clone();
                let runtime = ep.duration_ticks.unwrap_or(0) as i64;
                self.last_valid_pos = if crate::api::should_resume(ep.position_ticks, runtime) {
                    ep.position_ticks
                } else {
                    0
                };
                self.series_id.clear();
                self.season = 0;
                self.episode = 0;
                self.intro_start = 0;
                self.intro_end = 0;
                self.intro_state = IntroState::Pending;
            }
            QueueItem::AudiobookshelfBook(book) => {
                self.osd_title = book.title.clone();
                let runtime = book.duration_ticks.unwrap_or(0) as i64;
                self.last_valid_pos = if crate::api::should_resume(book.position_ticks, runtime) {
                    book.position_ticks
                } else {
                    0
                };
                self.series_id.clear();
                self.season = 0;
                self.episode = 0;
                self.intro_start = 0;
                self.intro_end = 0;
                self.intro_state = IntroState::Pending;
            }
        }
    }

    /// Construct a `PlaybackRun` from the Player owner's canonical slots.
    #[allow(clippy::too_many_arguments)]
    fn new_from_slot_items(
        items: Vec<ExecSlot>,
        start_idx: usize,
        origin: PlaybackOrigin,
        reporter: SessionReporter,
        config: MpvRunConfig,
        startup_pause_for_pipe: bool,
        status: Arc<Mutex<PlayerStatus>>,
        event_tx: mpsc::Sender<PlayerEvent>,
        subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
        shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
        server_url: String,
        token: String,
        audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
        prepared_source: Option<PreparedSource>,
    ) -> Self {
        let active_slot_id = items.get(start_idx).map(|slot| slot.slot_id);
        let queue = ExecutionSequence::from_slot_items(
            items.into_iter().map(|slot| (slot.slot_id, slot.item)).collect(),
            active_slot_id,
        );
        Self::init_from_queue(
            queue,
            start_idx,
            origin,
            reporter,
            config,
            startup_pause_for_pipe,
            status,
            event_tx,
            subtitle_prefs,
            shutdown_report_timeout,
            server_url,
            token,
            audiobookshelf_context,
            prepared_source,
            Vec::new(), // no external subtitles for feed items
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn init_from_queue(
        queue: ExecutionSequence,
        start_idx: usize,
        origin: PlaybackOrigin,
        reporter: SessionReporter,
        config: MpvRunConfig,
        startup_pause_for_pipe: bool,
        status: Arc<Mutex<PlayerStatus>>,
        event_tx: mpsc::Sender<PlayerEvent>,
        subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
        shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
        server_url: String,
        token: String,
        audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
        prepared_source: Option<PreparedSource>,
        ext_sub_urls: Vec<String>,
    ) -> Self {
        let start_idx = start_idx.min(queue.slots().len().saturating_sub(1));
        let initial_item = queue
            .active_slot()
            .map(|slot| slot.item.clone())
            .expect("PlaybackRun::new requires at least one item");

        let (initial_pos, osd_title, series_id, season, episode, intro_start, intro_end, past) =
            match &initial_item {
                QueueItem::Emby(emby) => {
                    let pos = if emby.is_audio() {
                        0
                    } else {
                        emby.playback_position_ticks
                    };
                    let (intro_start, intro_end) = (0, 0);
                    let past = intro_end > 0 && pos >= intro_end;
                    let sid = if emby.item_type == "Episode" {
                        ItemId::new(emby.series_id.clone())
                    } else {
                        ItemId::empty()
                    };
                    (
                        pos,
                        emby.display_name(),
                        sid,
                        emby.parent_index_number,
                        emby.index_number,
                        intro_start,
                        intro_end,
                        past,
                    )
                }
                QueueItem::Feed(entry) => {
                    let runtime = entry.duration_ticks.unwrap_or(0) as i64;
                    let pos = if crate::api::should_resume(entry.position_ticks, runtime) {
                        entry.position_ticks
                    } else {
                        0
                    };
                    (pos, entry.title.clone(), ItemId::empty(), 0, 0, 0, 0, false)
                }
                QueueItem::Audiobookshelf(ep) => {
                    let runtime = ep.duration_ticks.unwrap_or(0) as i64;
                    let pos = if crate::api::should_resume(ep.position_ticks, runtime) {
                        ep.position_ticks
                    } else {
                        0
                    };
                    (pos, ep.title.clone(), ItemId::empty(), 0, 0, 0, 0, false)
                }
                QueueItem::AudiobookshelfBook(book) => {
                    let runtime = book.duration_ticks.unwrap_or(0) as i64;
                    let pos = if crate::api::should_resume(book.position_ticks, runtime) {
                        book.position_ticks
                    } else {
                        0
                    };
                    (pos, book.title.clone(), ItemId::empty(), 0, 0, 0, 0, false)
                }
            };

        log::info!(
            target: "player",
            "playback init origin={origin:?} idx={start_idx} item_pos={}s",
            initial_pos / crate::api::TICKS_PER_SECOND
        );
        let active_file = queue.has_audiobookshelf_entries();
        let active_file_starting = active_file && prepared_source.is_some();
        let mut prepared_source = prepared_source;
        let active_lifecycle = ActiveItemLifecycle::for_item(
            &initial_item,
            prepared_source
                .as_mut()
                .and_then(PreparedSource::take_lifecycle),
        );
        PlaybackRun {
            origin,
            config,
            reporter,
            event_tx,
            status,
            subtitle_prefs,
            server_url,
            token,
            queue,
            audiobookshelf_context,
            active_file,
            prepared_source,
            active_lifecycle,
            active_file_starting,
            ext_sub_urls,
            current_idx: start_idx,
            forced_slot_id: None,
            forced_transition: None,
            stop_slot: None,
            stop_runtime: None,
            quit_at: None,
            last_seek_at: None,
            last_valid_pos: initial_pos,
            tracks_initialized: false,
            load_state: LoadState::Ready,
            pending_initial_playlist_layout: start_idx > 0,
            stop_report: StopReport::NotSent,
            stopped_event_sent: false,
            mark_played_id: None,
            last_mouse_osd: None,
            series_id,
            season,
            episode,
            next_up: NextUp::Idle,
            queue_next_up: NextUp::Idle,
            next_up_jump: false,
            stopped_near_end: false,
            shutdown_report_timeout,
            startup_pause: StartupPause::new(startup_pause_for_pipe),
            intro_start,
            intro_end,
            intro_state: IntroState::new(past),
            osd_title,
        }
    }

    fn set_intro(&mut self, start: i64, end: i64, pos: i64) {
        self.intro_start = start;
        self.intro_end = end;
        let past = end > 0 && pos >= end;
        self.intro_state.reset(past);
    }
}
