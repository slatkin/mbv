use super::{
    AudiobookshelfPlayerContext, EMBY_RESUME_RETRY_DELAYS, EmbyClient, EmbyItem, ExecSlot, Mpv,
    MpvRunConfig, PlaybackOrigin, PlaybackRun, Player, PlayerCommand, PlayerEvent, PlayerStatus,
    PreparedSource, ProgressGuard, QueueItem, QueueSlotId, RunInit, SessionReporter, SubtitlePrefs,
    active_file_load_location, init_mpv, init_volume, make_wakeup_pipe, observe_properties,
    prepare_source, queue_load_indices, queue_load_location, reassert_queue_layout,
    refresh_emby_resume, send_ep_info, spawn_progress_reporter, start_queue_playback,
};
use mbv_ids::{EmbySessionId, ItemId, MediaSourceId};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use crate::mpv_session::spawn::Spawner;

/// Why `submit_queue_slots` fell through to a cold start (design D1, D5).
#[derive(Clone, Copy)]
enum ColdStartReason {
    NoRun,
    HeadlessMismatch,
    StopRequested,
    RunExited,
    SendFailed,
}

impl ColdStartReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::NoRun => "no_run",
            Self::HeadlessMismatch => "headless_mismatch",
            Self::StopRequested => "stop_requested",
            Self::RunExited => "run_exited",
            Self::SendFailed => "send_failed",
        }
    }
}

/// The `SubmitQueue` payload of a command a failed send returned. The reuse
/// path only ever sends `SubmitQueue`, so no other variant can come back.
fn submit_queue_parts(cmd: PlayerCommand) -> (Vec<ExecSlot>, usize) {
    match cmd {
        PlayerCommand::SubmitQueue { items, start_idx } => (items, start_idx),
        _ => unreachable!("the reuse path only sends SubmitQueue"),
    }
}

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
    ///
    /// # Panics
    ///
    /// Panics if any of the player's shared mutexes is poisoned, i.e. a previous
    /// owner panicked while holding it: the owner state read or seeded here
    /// (`status`, `origin`, `ws_tx`, `credentials`, `audiobookshelf_context`,
    /// `stop_tx`, `shutdown_report_timeout`, `cmd_tx`, `wakeup_fd`,
    /// `run_exited`, `pre_warmed_mpv`, `thread_handle`). An empty submission
    /// returns before locking; a submission rejected for Audiobookshelf
    /// admission still locks `audiobookshelf_context` to answer the admission
    /// question.
    pub fn submit_queue_slots(
        &self,
        items: Vec<ExecSlot>,
        start_idx: usize,
        client: Option<Arc<EmbyClient>>,
        headless: bool,
        initial_volume: u8,
    ) -> bool {
        if items.is_empty()
            || (items.iter().any(|slot| slot.item.is_audiobookshelf())
                && !self.can_admit_audiobookshelf())
        {
            return false;
        }
        let start_idx = start_idx.min(items.len() - 1);
        // Every accepted submission establishes a new queue identity. The
        // generation is serialized in PlayerStatus so clients can fence
        // slot-addressed commands against a locally replaced queue.
        let run_identity = self.advance_sequence_generation();
        let (reason, items, start_idx) =
            match self.submit_into_live_run(headless, items, start_idx, run_identity) {
                Ok(()) => return true,
                Err(failed) => failed,
            };
        tracing::info!(name: "player.submit.cold_started", target: "player", reason = reason.as_str(), "queue replacement starting a new playback run");

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
        // external: the player thread's first act is `init_mpv`, a live `mpv`
        // child process whose start a test must not perform (issue #757). The
        // queue/status seeding above is what stub-backed
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
        *self.wakeup_fd.lock().unwrap() = wakeup_pipe.map(|(_, w)| w);
        let pre_warmed = self.pre_warmed_mpv.lock().unwrap().take();
        let spawner = Arc::clone(&self.spawner);
        // A fresh exit flag for this run (design D7): the dying run's thread
        // can still set the previous flag after this point, and it must never
        // mark the new run.
        let run_exited = Arc::new(AtomicBool::new(false));
        *self.run_exited.lock().unwrap() = Arc::clone(&run_exited);

        let handle = thread::spawn(move || {
            run_player_thread(PlayerThreadStart {
                pre_warmed,
                spawner,
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
                run_exited,
            });
        });
        *self.thread_handle.lock().unwrap() = Some(handle);
        true
    }

    /// The reuse path (design D1): submit into the live run when the run
    /// exists, the headless class matches, no stop was requested for it, its
    /// loop has not exited, and the send is accepted. The reported playback
    /// state does not decide this: an idle or parked run is still live.
    ///
    /// Checks 3–5 hold the `stop_tx` lock with the `cmd_tx` lock taken inside
    /// it (`stop()` takes `stop_tx`, the send takes `cmd_tx`), so a `stop()`
    /// cannot slip between the check and the send. The status is seeded only
    /// after the send is accepted (design D2), so a submission that then
    /// cold-starts never publishes a seeded status. On any failed check,
    /// returns the cold-start reason with the submission restored for the
    /// cold start.
    fn submit_into_live_run(
        &self,
        headless: bool,
        mut items: Vec<ExecSlot>,
        mut start_idx: usize,
        run_identity: mbv_ctrl::PlaybackGeneration,
    ) -> Result<(), (ColdStartReason, Vec<ExecSlot>, usize)> {
        if self.cmd_tx.lock().unwrap().is_none() {
            return Err((ColdStartReason::NoRun, items, start_idx));
        }
        if self.current_is_headless.load(Ordering::Relaxed) != headless {
            return Err((ColdStartReason::HeadlessMismatch, items, start_idx));
        }
        let stop_tx = self.stop_tx.lock().unwrap();
        if stop_tx.is_none() {
            return Err((ColdStartReason::StopRequested, items, start_idx));
        }
        if self.run_exited.lock().unwrap().load(Ordering::Relaxed) {
            return Err((ColdStartReason::RunExited, items, start_idx));
        }
        // The start item is cloned before the send because the send consumes
        // the submission; the seed itself happens after the send is accepted
        // (design D2).
        let start_item = items[start_idx].item.clone();
        let items_len = items.len();
        let cmd = PlayerCommand::SubmitQueue {
            items: std::mem::take(&mut items),
            start_idx,
        };
        let cmd_tx = self.cmd_tx.lock().unwrap();
        let sent = if let Some(tx) = cmd_tx.as_ref() {
            match tx.send(cmd) {
                Ok(()) => true,
                Err(error) => {
                    (items, start_idx) = submit_queue_parts(error.0);
                    false
                }
            }
        } else {
            // Unreachable: the run-existence check above established `cmd_tx`
            // is `Some`, and with the `stop_tx` lock held no cold start can
            // replace it. Recover the unsent command so the cold start below
            // still submits it.
            (items, start_idx) = submit_queue_parts(cmd);
            false
        };
        drop(cmd_tx);
        drop(stop_tx);
        if sent {
            if let Some(w) = self.wakeup_fd.lock().unwrap().as_ref() {
                w.notify();
            }
            tracing::info!(name: "player.submit.reused_run", target: "player", run_identity, "queue replacement submitted into the live run");
            self.status
                .lock()
                .unwrap()
                .seed_from_item(&start_item, start_idx, items_len);
            return Ok(());
        }
        Err((ColdStartReason::SendFailed, items, start_idx))
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
}

fn make_reporter(
    client: Option<Arc<EmbyClient>>,
    ws_tx: Option<mbv_ws::WsSender>,
    item: &QueueItem,
    status: Arc<Mutex<PlayerStatus>>,
) -> (SessionReporter, ProgressGuard) {
    let session = client.as_ref().zip(item.as_emby()).map(|(client, emby)| {
        // The ids resolve here, but the start report itself waits until the
        // run observes actual playback (`PlaybackRestart`), so a run that
        // never plays reports no session start and no server-side churn.
        let info = client.get_playback_info(&emby.id);
        (
            ItemId::new(emby.id.clone()),
            info.media_source_id,
            info.session_id,
        )
    });
    let client = client.unwrap_or_else(|| Arc::new(EmbyClient::new(mbv_config::Config::default())));
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
    /// The controller's long-lived child spawner; the on-demand start uses it
    /// when no run was pre-warmed (design D5).
    spawner: Arc<Spawner>,
    config: MpvRunConfig,
    status: Arc<Mutex<PlayerStatus>>,
    event_tx: mpsc::Sender<PlayerEvent>,
    initial_volume: u8,
    items: Vec<ExecSlot>,
    start_idx: usize,
    run_identity: mbv_ctrl::PlaybackGeneration,
    server_url: String,
    token: String,
    audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
    client: Option<Arc<EmbyClient>>,
    ws_tx: Option<mbv_ws::WsSender>,
    origin: PlaybackOrigin,
    subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    stop_rx: mpsc::Receiver<()>,
    cmd_rx: mpsc::Receiver<PlayerCommand>,
    wakeup_read_fd: std::os::unix::io::RawFd,
    /// This run's exit flag (design D7): the run sets it when its loop
    /// returns, before the mpv teardown, so the Controller's reuse decision
    /// never submits into a dying run.
    run_exited: Arc<AtomicBool>,
}

fn run_player_thread(mut start: PlayerThreadStart) {
    let (mpv, startup_pause_for_pipe) = match start.pre_warmed.take() {
        Some(warmed) => warmed,
        None => match init_mpv(&start.spawner, &start.config) {
            Ok(value) => value,
            Err(error) => {
                tracing::error!(name: "player.startup.failed", target: "player", error = %error, "mpv startup failed");
                start.status.lock().unwrap().active = false;
                let _ = start.event_tx.send(PlayerEvent::Stopped {
                    slot_id: None,
                    run_identity: start.run_identity,
                    position_ticks: 0,
                    played: false,
                    consume: false,
                    error: Some(format!("mpv startup failed: {error}")),
                });
                return;
            }
        },
    };
    init_volume(&mpv, &start.status, start.initial_volume);

    let active_file_projection = start.items.iter().any(|slot| slot.item.is_audiobookshelf());
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
        run_exited,
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
        &run_exited,
    );
}

fn load_queue_sources(
    mpv: &Mpv,
    start: &PlayerThreadStart,
    active_file_projection: bool,
) -> Option<PreparedSource> {
    // Design D1: an Emby video's start position comes from Emby, fetched
    // before any `start=` option is baked into the loads below. This runs
    // before `make_reporter`, so it uses the same Emby client the reporter
    // will hold (`start.client`).
    let items: Vec<QueueItem> = match start.client.as_deref() {
        Some(client) => refresh_emby_resume(
            start.items.iter().map(|slot| slot.item.clone()).collect(),
            |ids| client.get_items_by_ids(ids),
            &EMBY_RESUME_RETRY_DELAYS,
        ),
        None => start.items.iter().map(|slot| slot.item.clone()).collect(),
    };
    let load_indices: Vec<_> = if active_file_projection {
        vec![start.start_idx]
    } else {
        queue_load_indices(items.len(), start.start_idx).collect()
    };
    let mut active_prepared_source = None;
    for index in load_indices {
        let item = &items[index];
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
        tracing::info!(name: "player.queue_load.started", target: "player", { url.path = %mbv_core::applog::UrlPath(prepared.url.as_str()), mode = mode }, "queue loadfile starting");
        if let Err(error) = mpv.command(
            "loadfile",
            &[prepared.url.as_str(), mode, &position, &options],
        ) {
            tracing::warn!(name: "player.queue_load.failed", target: "player", error = %error, mode, "queue loadfile failed");
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
