impl PlaybackRun {
    fn queue_len(&self) -> usize {
        self.queue.slots().len()
    }

    fn slot_id_at(&self, idx: usize) -> Option<QueueSlotId> {
        self.queue.slots().get(idx).map(|slot| slot.slot_id)
    }

    fn item_at(&self, idx: usize) -> Option<&MediaItem> {
        self.queue.slots().get(idx).map(|slot| &slot.item)
    }

    fn active_item(&self) -> Option<&MediaItem> {
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

    fn report_stopped_for_end_file(&self, reason: EndFileReason) -> bool {
        match end_file_stop_report_context(reason) {
            StopReportContext::Ordinary => self.reporter.report_stopped(self.last_valid_pos),
            StopReportContext::ShutdownAware => self.report_stopped_for_current_context(),
        }
    }

    /// Budget for `ProgressGuard::stop_and_join`. During a real quit
    /// (`shutdown_report_timeout` set via `Player::stop_for_shutdown`),
    /// this is deliberately *half* of `quit_timeout_secs`, not the full
    /// value: `report_stopped_for_shutdown` (see
    /// `report_stopped_for_current_context`) keeps the full
    /// `quit_timeout_secs` as its own budget per the spec's resolved
    /// design (it's the session-terminating call and the one worth
    /// protecting most), so giving this secondary, non-network-critical
    /// join the same full budget would leave the outer teardown bound
    /// with only a thin, constant margin over the worst case of the two
    /// nested calls combined — see `App::teardown`'s `outer_bound` for
    /// the composition this budget feeds into. Outside of shutdown
    /// (ordinary track transitions), there is no time pressure, so a
    /// generous fixed budget (matching the shared agent's own ~30s worst
    /// case) just guards against a truly stuck thread without adding
    /// latency to the common fast case.
    fn progress_join_budget(&self) -> Duration {
        match *self.shutdown_report_timeout.lock().unwrap() {
            Some(quit_timeout) => quit_timeout / 2,
            None => Duration::from_secs(30),
        }
    }

    /// Clears any pending-quit state so a `LoadNew`/`ReplaceQueue` command
    /// that arrives while a quit is in flight fully cancels it — not just
    /// `quit_at`, but also the shutdown-scoped report budget set by
    /// `Player::stop_for_shutdown`. Without resetting
    /// `shutdown_report_timeout` here too, a cancelled quit would leave it
    /// `Some` for the rest of this `PlaybackRun`'s lifetime (nothing
    /// else clears it once set), so every subsequent track transition
    /// would silently keep using the tight shutdown budget/no-retry
    /// behavior via `progress_join_budget`/`report_stopped_for_current_context`
    /// instead of the ordinary one — no crash, just quietly degraded
    /// reliability for the rest of the session.
    fn cancel_pending_quit(&mut self) {
        self.quit_at = None;
        *self.shutdown_report_timeout.lock().unwrap() = None;
    }

    fn sync_status_position(&self) {
        let mut s = self.status.lock().unwrap();
        s.current_idx = self.current_idx;
        s.queue_len = self.queue_len();
    }

    fn refresh_current_idx_from_queue(&mut self) {
        if let Some(slot_id) = self.active_slot_id() {
            if let Some(idx) = self.queue.slot_index(slot_id) {
                self.current_idx = idx;
            }
        } else if self.queue_len() == 0 {
            self.current_idx = 0;
        } else {
            self.current_idx = self.current_idx.min(self.queue_len() - 1);
        }
        self.sync_status_position();
    }

    fn set_active_index(&mut self, idx: usize) -> bool {
        let Some(slot_id) = self.slot_id_at(idx) else {
            return false;
        };
        if !matches!(
            self.queue.set_active_slot(slot_id),
            crate::playback_queue::QueueMutationResult::Applied(())
        ) {
            return false;
        }
        self.current_idx = idx;
        self.sync_status_position();
        true
    }

    fn reset_next_up_state(&mut self) {
        self.next_up.reset();
        self.queue_next_up.reset();
        self.next_up_jump = false;
    }

    /// Reset per-item lifecycle flags shared by all three reset sites in
    /// `player_run_commands.rs` (`cmd_replace_queue` empty, non-empty,
    /// and `cmd_load_new`). The caller must set `stop_report` and
    /// `load_state` itself because those differ per call site.
    fn begin_item_lifecycle(&mut self) {
        self.tracks_initialized = false;
        self.forced_slot_id = None;
        self.reset_next_up_state();
        self.stopped_event_sent = false;
        self.mark_played_id = None;
        self.stopped_near_end = false;
    }

    fn load_active_item_state(&mut self) {
        let Some(item) = self.active_item().cloned() else {
            self.osd_title.clear();
            self.pending_resume_secs = None;
            self.last_valid_pos = 0;
            self.series_id.clear();
            self.season = 0;
            self.episode = 0;
            self.intro_start = 0;
            self.intro_end = 0;
            self.intro_state = IntroState::Pending;
            return;
        };

        self.osd_title = item.display_name();
        self.last_valid_pos = if item.is_audio() {
            0
        } else {
            item.playback_position_ticks
        };
        self.pending_resume_secs = if self.origin == PlaybackOrigin::Standalone {
            // Standalone fresh-start (cmd_load_new) already sets the mpv `start`
            // property to the resume position before calling this; setting
            // pending_resume_secs too would trigger a redundant absolute seek
            // in on_playback_restart that also suppresses the first progress
            // report for ~500ms. Queue playback and mid-session slot activation
            // always run with Queue origin, so they are unaffected.
            None
        } else if !item.is_audio() && item.should_resume() {
            Some(item.resume_seconds())
        } else {
            None
        };
        if item.item_type == "Episode" {
            self.series_id = ItemId::new(item.series_id.clone());
            self.season = item.parent_index_number;
            self.episode = item.index_number;
        } else {
            self.series_id.clear();
            self.season = 0;
            self.episode = 0;
        }
        let (intro_start, intro_end) = load_intro_times(&self.reporter.client, &item.id);
        self.set_intro(intro_start, intro_end, item.playback_position_ticks);
    }

    // `startup_pause_for_pipe` (added for audio-pipe startup-pause handling)
    // pushed this past clippy's 10-argument default; grouping these into a
    // params struct is a reasonable follow-up but out of scope here.
    #[allow(clippy::too_many_arguments)]
    fn new(
        items: Vec<MediaItem>,
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
        ext_sub_urls: Vec<String>,
    ) -> Self {
        let start_idx = start_idx.min(items.len().saturating_sub(1));
        let queue = PlaybackQueue::from_items(items, Some(start_idx));
        let initial_item = queue
            .active_slot()
            .map(|slot| slot.item.clone())
            .expect("PlaybackRun::new requires at least one item");
        let initial_pos = if initial_item.is_audio() {
            0
        } else {
            initial_item.playback_position_ticks
        };
        let (intro_start, intro_end) = load_intro_times(&reporter.client, &initial_item.id);
        let past = intro_end > 0 && initial_pos >= intro_end;
        let pending_resume_secs = if !initial_item.is_audio() && initial_item.should_resume() {
            Some(initial_item.resume_seconds())
        } else {
            None
        };
        log::info!(
            target: "player",
            "playback init origin={origin:?} idx={start_idx} item_pos={}s pending_resume={pending_resume_secs:?}s",
            initial_pos / crate::api::TICKS_PER_SECOND
        );
        let osd_title = initial_item.display_name();
        let series_id = if initial_item.item_type == "Episode" {
            ItemId::new(initial_item.series_id.clone())
        } else {
            ItemId::empty()
        };
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
            ext_sub_urls,
            current_idx: start_idx,
            forced_slot_id: None,
            quit_at: None,
            last_seek_at: None,
            last_valid_pos: initial_pos,
            tracks_initialized: false,
            load_state: LoadState::Ready,
            pending_initial_jump: start_idx > 0,
            stop_report: StopReport::NotSent,
            stopped_event_sent: false,
            mark_played_id: None,
            last_mouse_osd: None,
            series_id,
            season: initial_item.parent_index_number,
            episode: initial_item.index_number,
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
            pending_resume_secs,
        }
    }

    fn set_intro(&mut self, start: i64, end: i64, pos: i64) {
        self.intro_start = start;
        self.intro_end = end;
        let past = end > 0 && pos >= end;
        self.intro_state.reset(past);
    }
}
