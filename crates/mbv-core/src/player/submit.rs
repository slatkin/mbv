use super::{
    active_file_load_location, init_mpv, init_volume, make_wakeup_pipe, observe_properties,
    prepare_source, queue_load_indices, queue_load_location, reassert_queue_layout, send_ep_info,
    spawn_progress_reporter, start_queue_playback, AudiobookshelfPlayerContext, EmbyClient,
    EmbyItem, EmbySessionId, ExecSlot, ItemId, MediaSourceId, Mpv, MpvRunConfig, PlaybackOrigin,
    PlaybackRun, Player, PlayerCommand, PlayerEvent, PlayerStatus, PreparedSource, ProgressGuard,
    QueueItem, QueueSlotId, RunInit, SessionReporter, SubtitlePrefs,
};
use std::sync::{atomic::Ordering, mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

impl Player {
    /// Play a freshly fetched Emby sequence with no pre-existing canonical
    /// queue. Callers that already own a queue must use `submit_queue_slots`
    /// with that queue's slot pairs so owner and client address the same
    /// occurrences.
    fn sequential_slot_ids(items: Vec<QueueItem>) -> Vec<ExecSlot> {
        items
            .into_iter()
            .enumerate()
            .map(|(i, item)| ExecSlot {
                slot_id: QueueSlotId::from_raw(i as u64 + 1),
                item,
            })
            .collect()
    }

    pub fn play(&self, item: &EmbyItem, client: Arc<EmbyClient>, initial_volume: u8) {
        let headless = self.headless_for(&client, item.is_audio());
        self.submit_queue_slots(
            Self::sequential_slot_ids(vec![QueueItem::Emby(Box::new(item.clone()))]),
            0,
            Some(client),
            headless,
            initial_volume,
        );
    }

    pub fn play_queue(
        &self,
        items: Vec<EmbyItem>,
        start_idx: usize,
        client: Arc<EmbyClient>,
        initial_volume: u8,
    ) {
        if items.is_empty() {
            return;
        }
        let all_audio = items
            .iter()
            .all(|i| i.media_type == "Audio" || i.item_type == "Audio");
        let headless = self.headless_for(&client, all_audio);
        let queue_items: Vec<QueueItem> = items
            .into_iter()
            .map(|i| QueueItem::Emby(Box::new(i)))
            .collect();
        self.submit_queue_slots(
            Self::sequential_slot_ids(queue_items),
            start_idx,
            Some(client),
            headless,
            initial_volume,
        );
    }

    /// Submit a canonical Bound queue. The Playback run must preserve these
    /// identities because every later command and observation addresses slots,
    /// not playlist positions.
    pub fn submit_queue_slots(
        &self,
        items: Vec<ExecSlot>,
        start_idx: usize,
        client: Option<Arc<EmbyClient>>,
        headless: bool,
        initial_volume: u8,
    ) -> bool {
        if items.is_empty()
            || (items.iter().any(|slot| slot.item.is_audiobookshelf_any())
                && !self.can_admit_audiobookshelf())
        {
            return false;
        }
        let start_idx = start_idx.min(items.len() - 1);
        // Every accepted submission establishes a new queue identity. The
        // generation is serialized in PlayerStatus so clients can fence
        // slot-addressed commands against a locally replaced queue.
        let run_identity = self.advance_sequence_generation();

        // Fast path: reuse existing mpv window when headless state matches.
        if self.status.lock().unwrap().active
            && (self.current_is_headless.load(Ordering::Relaxed) == headless)
        {
            let start_item = &items[start_idx].item;
            {
                let mut st = self.status.lock().unwrap();
                st.seed_from_item(start_item, start_idx, items.len());
            };
            return self.send_command(PlayerCommand::SubmitQueue { items, start_idx });
        }

        // Cold start: stop, join, spawn fresh player thread.
        self.stop();
        self.join();

        let config = self.cold_start_config(client.as_deref(), headless);
        let status = Arc::clone(&self.status);
        let event_tx = self.event_tx.clone();
        let ws_tx = if client.is_some() {
            self.ws_tx.lock().unwrap().clone()
        } else {
            None
        };
        let subtitle_prefs = Arc::clone(&self.subtitle_prefs);
        let shutdown_report_timeout = Arc::clone(&self.shutdown_report_timeout);
        let (server_url, token) = self.credentials.lock().unwrap().clone().unwrap_or_default();
        let audiobookshelf_context = self.audiobookshelf_context.lock().unwrap().clone();
        let origin = if items.len() == 1 {
            PlaybackOrigin::Standalone
        } else {
            PlaybackOrigin::Queue
        };
        *self.origin.lock().unwrap() = origin;
        self.current_is_headless.store(headless, Ordering::Relaxed);

        // Set initial status for the start item.
        let start_item = &items[start_idx].item;
        {
            let mut st = status.lock().unwrap();
            st.seed_from_item(start_item, start_idx, items.len());
            st.active = true;
        };

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *self.stop_tx.lock().unwrap() = Some(stop_tx);
        *self.shutdown_report_timeout.lock().unwrap() = None;
        // Test builds (PlayerProxy::stub) must never construct the real
        // external: the player thread's first act is `init_mpv`, a live
        // libmpv/libav/GnuTLS handle whose initialization races process
        // teardown when the test returns (glibc double-free / SIGSEGV,
        // issue #757). The queue/status seeding above is what stub-backed
        // tests assert; only the real player thread (and the internal
        // command/wakeup plumbing only that thread serves) is skipped, so
        // `cmd_tx` keeps pointing at whatever the stub installed (a spy or
        // a dropped receiver).
        if self.mpv_inhibited.load(Ordering::Relaxed) {
            return true;
        }
        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        *self.cmd_tx.lock().unwrap() = Some(cmd_tx);
        let wakeup_pipe = make_wakeup_pipe();
        let wakeup_read_fd = wakeup_pipe.as_ref().map_or(-1, |(r, _)| *r);
        let wakeup_write_fd = wakeup_pipe.as_ref().map_or(-1, |(_, w)| w.0);
        *self.wakeup_fd.lock().unwrap() = wakeup_pipe.map(|(_, w)| w);
        let pre_warmed = self.pre_warmed_mpv.lock().unwrap().take();

        let handle = thread::spawn(move || {
            run_player_thread(PlayerThreadStart {
                pre_warmed,
                config,
                status,
                event_tx,
                initial_volume,
                items,
                start_idx,
                run_identity,
                server_url,
                token,
                audiobookshelf_context,
                client,
                ws_tx,
                origin,
                subtitle_prefs,
                shutdown_report_timeout,
                stop_rx,
                cmd_rx,
                wakeup_read_fd,
                wakeup_write_fd,
            });
        });
        *self.thread_handle.lock().unwrap() = Some(handle);
        true
    }

    fn cold_start_config(&self, client: Option<&EmbyClient>, headless: bool) -> MpvRunConfig {
        let (audio_pipe_path, audio_pipe_samplerate, audio_pipe_bitdepth, always_skip_intro) =
            if let Some(client) = client {
                (
                    client.config.audio_pipe_target(),
                    client.config.audio_pipe_samplerate,
                    client.config.audio_pipe_bitdepth,
                    self.always_skip_intro,
                )
            } else {
                (None, 0, 0, false)
            };
        let (audio_pipe_path, audio_device) = if audio_pipe_path.is_some() {
            (audio_pipe_path, None)
        } else {
            (None, self.audio_device.clone())
        };
        MpvRunConfig {
            headless,
            use_mpv_config: self.use_mpv_config,
            video_cache_forward_mb: self.video_cache_forward_mb,
            video_cache_back_mb: self.video_cache_back_mb,
            no_scripts: self.no_scripts,
            always_skip_intro,
            audio_pipe_path,
            audio_pipe_samplerate,
            audio_pipe_bitdepth,
            audio_device,
        }
    }

    pub fn queue_append(&self, slots: Vec<ExecSlot>) -> bool {
        if slots.is_empty()
            || (slots.iter().any(|slot| slot.item.is_audiobookshelf_any())
                && !self.can_admit_audiobookshelf())
        {
            return false;
        }
        self.send_command(PlayerCommand::QueueAppend { items: slots })
    }
}

fn make_reporter(
    client: Option<Arc<EmbyClient>>,
    ws_tx: Option<crate::ws::WsSender>,
    item: &QueueItem,
    status: Arc<Mutex<PlayerStatus>>,
) -> (SessionReporter, ProgressGuard) {
    let session = client.as_ref().zip(item.as_emby()).map(|(client, emby)| {
        let info = client.get_playback_info(&emby.id);
        let report_client = Arc::clone(client);
        let report_item = emby.clone();
        let media_source_id = info.media_source_id.clone();
        let session_id = info.session_id.clone();
        thread::spawn(move || {
            let ok = report_client.report_start(&report_item, &media_source_id, &session_id);
            if !ok {
                log::warn!(
                    target: "player",
                    "report_start failed for item={}",
                    report_item.id,
                );
            }
        });
        (
            ItemId::new(emby.id.clone()),
            info.media_source_id,
            info.session_id,
        )
    });
    let client =
        client.unwrap_or_else(|| Arc::new(EmbyClient::new(crate::config::Config::default())));
    let has_session = session.is_some();
    let (item_id, media_source_id, session_id) = session.unwrap_or_else(|| {
        (
            ItemId::empty(),
            MediaSourceId::new(""),
            EmbySessionId::new(""),
        )
    });
    let reporter = SessionReporter::new(
        client,
        ws_tx,
        item_id,
        media_source_id,
        session_id,
        item.is_audio(),
        status,
    );
    let progress = if has_session {
        spawn_progress_reporter(reporter.clone())
    } else {
        reporter.clear_session();
        let (stop_tx, _stop_rx) = mpsc::channel();
        ProgressGuard {
            stop_tx,
            handle: None,
        }
    };
    (reporter, progress)
}

struct PlayerThreadStart {
    pre_warmed: Option<(Mpv, bool)>,
    config: MpvRunConfig,
    status: Arc<Mutex<PlayerStatus>>,
    event_tx: mpsc::Sender<PlayerEvent>,
    initial_volume: u8,
    items: Vec<ExecSlot>,
    start_idx: usize,
    run_identity: crate::ctrl::PlaybackGeneration,
    server_url: String,
    token: String,
    audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
    client: Option<Arc<EmbyClient>>,
    ws_tx: Option<crate::ws::WsSender>,
    origin: PlaybackOrigin,
    subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    stop_rx: mpsc::Receiver<()>,
    cmd_rx: mpsc::Receiver<PlayerCommand>,
    wakeup_read_fd: std::os::unix::io::RawFd,
    wakeup_write_fd: std::os::unix::io::RawFd,
}

fn run_player_thread(mut start: PlayerThreadStart) {
    let (mpv, startup_pause_for_pipe) = match start.pre_warmed.take() {
        Some(warmed) => warmed,
        None => match init_mpv(&start.config) {
            Ok(value) => value,
            Err(error) => {
                log::error!(target: "player", "{error}");
                start.status.lock().unwrap().active = false;
                let _ = start.event_tx.send(PlayerEvent::Stopped {
                    slot_id: None,
                    run_identity: start.run_identity,
                    position_ticks: 0,
                    played: false,
                    consume: false,
                    progress_report_accepted: false,
                    error: Some(format!("mpv startup failed: {error}")),
                });
                return;
            }
        },
    };
    init_volume(&mpv, &start.status, start.initial_volume);

    let active_file_projection = start
        .items
        .iter()
        .any(|slot| slot.item.is_audiobookshelf_any());
    let Some(active_prepared_source) = load_queue_sources(&mpv, &start, active_file_projection)
    else {
        return;
    };
    if !active_file_projection {
        // Design D3 load-then-play: every load above was no-play, so playback
        // starts at the requested slot and the layout check must now pass.
        start_queue_playback(&mpv, start.start_idx);
        reassert_queue_layout(&mpv, start.start_idx, start.items.len());
    }
    if let Some(emby) = start
        .items
        .get(start.start_idx)
        .and_then(|slot| slot.item.as_emby())
    {
        send_ep_info(&mpv, emby);
    }
    observe_properties(&mpv, start.config.use_mpv_config);

    let PlayerThreadStart {
        config,
        status,
        event_tx,
        items,
        start_idx,
        server_url,
        token,
        audiobookshelf_context,
        client,
        ws_tx,
        origin,
        subtitle_prefs,
        shutdown_report_timeout,
        stop_rx,
        cmd_rx,
        wakeup_read_fd,
        wakeup_write_fd,
        ..
    } = start;
    let (reporter, progress) =
        make_reporter(client, ws_tx, &items[start_idx].item, Arc::clone(&status));
    let session = PlaybackRun::new_from_slot_items(
        items,
        RunInit {
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
            prepared_source: Some(active_prepared_source),
        },
    );
    session.run(
        mpv,
        &stop_rx,
        &cmd_rx,
        progress,
        wakeup_read_fd,
        wakeup_write_fd,
    );
}

fn load_queue_sources(
    mpv: &Mpv,
    start: &PlayerThreadStart,
    active_file_projection: bool,
) -> Option<PreparedSource> {
    let load_indices: Vec<_> = if active_file_projection {
        vec![start.start_idx]
    } else {
        queue_load_indices(start.items.len(), start.start_idx).collect()
    };
    let mut active_prepared_source = None;
    for index in load_indices {
        let item = &start.items[index].item;
        let prepared = match prepare_source(
            item,
            &start.server_url,
            &start.token,
            start.audiobookshelf_context.as_ref(),
        ) {
            Ok(source) => source,
            Err(error) => {
                start.status.lock().unwrap().active = false;
                let _ = start.event_tx.send(PlayerEvent::Stopped {
                    slot_id: None,
                    run_identity: start.run_identity,
                    position_ticks: 0,
                    played: false,
                    consume: false,
                    progress_report_accepted: false,
                    error: Some(format!("failed to prepare media: {error}")),
                });
                return None;
            }
        };
        let (mode, position) = if active_file_projection {
            // An active-file load is the whole playlist, so it must start it.
            active_file_load_location()
        } else {
            queue_load_location(index, start.start_idx)
        };
        let options = prepared.mpv_load_options(item);
        if let Err(error) = mpv.command(
            "loadfile",
            &[prepared.url.as_str(), mode, &position, &options],
        ) {
            log::warn!(target: "player", "submit_queue loadfile error: {error} | mode={mode}");
            if index == start.start_idx {
                let mut prepared = prepared;
                prepared.close(0.0);
                start.status.lock().unwrap().active = false;
                let _ = start.event_tx.send(PlayerEvent::Stopped {
                    slot_id: None,
                    run_identity: start.run_identity,
                    position_ticks: 0,
                    played: false,
                    consume: false,
                    progress_report_accepted: false,
                    error: Some(format!("failed to load media: {error}")),
                });
                return None;
            }
        }
        if index == start.start_idx {
            active_prepared_source = Some(prepared);
        }
    }
    active_prepared_source
}
