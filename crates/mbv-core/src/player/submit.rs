use super::*;

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
        {
            let mut st = self.status.lock().unwrap();
            st.sequence_generation = st.sequence_generation.saturating_add(1);
        }

        // Fast path: reuse existing mpv window when headless state matches.
        if self.status.lock().unwrap().active
            && (self.current_is_headless.load(Ordering::Relaxed) == headless)
        {
            let start_item = &items[start_idx].item;
            {
                let mut st = self.status.lock().unwrap();
                st.seed_from_item(start_item, start_idx, items.len());
            }
            return self.send_command(PlayerCommand::SubmitQueue { items, start_idx });
        }

        // Cold start: stop, join, spawn fresh player thread.
        self.stop();
        self.join();

        let (audio_pipe_path, audio_pipe_samplerate, audio_pipe_bitdepth, always_skip_intro) =
            if let Some(ref c) = client {
                (
                    c.config.audio_pipe_target(),
                    c.config.audio_pipe_samplerate,
                    c.config.audio_pipe_bitdepth,
                    self.always_skip_intro,
                )
            } else {
                (None, 0, 0, false)
            };
        // Mutually exclusive output target: pipe output wins when selected
        // for this run, and clocked ALSA projection applies only when it is
        // not.
        let (audio_pipe_path, audio_device) = if audio_pipe_path.is_some() {
            (audio_pipe_path, None)
        } else {
            (None, self.audio_device.clone())
        };

        let config = MpvRunConfig {
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
        };
        let status = self.status.clone();
        let event_tx = self.event_tx.clone();
        let ws_tx = if client.is_some() {
            self.ws_tx.lock().unwrap().clone()
        } else {
            None
        };
        let subtitle_prefs = self.subtitle_prefs.clone();
        let shutdown_report_timeout = self.shutdown_report_timeout.clone();
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
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *self.stop_tx.lock().unwrap() = Some(stop_tx);
        *self.shutdown_report_timeout.lock().unwrap() = None;
        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        *self.cmd_tx.lock().unwrap() = Some(cmd_tx);
        let wakeup_pipe = make_wakeup_pipe();
        let wakeup_read_fd = wakeup_pipe.as_ref().map(|(r, _)| *r).unwrap_or(-1);
        let wakeup_write_fd = wakeup_pipe.as_ref().map(|(_, w)| w.0).unwrap_or(-1);
        *self.wakeup_fd.lock().unwrap() = wakeup_pipe.map(|(_, w)| w);
        let pre_warmed = self.pre_warmed_mpv.lock().unwrap().take();

        let handle = thread::spawn(move || {
            let (mpv, startup_pause_for_pipe) = match pre_warmed {
                Some(w) => w,
                None => match init_mpv(&config) {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!(target: "player", "{}", e);
                        status.lock().unwrap().active = false;
                        let _ = event_tx.send(PlayerEvent::Stopped {
                            slot_id: None,
                            position_ticks: 0,
                            played: false,
                            consume: false,
                            progress_report_accepted: false,
                            error: Some(format!("mpv startup failed: {e}")),
                        });
                        return;
                    }
                },
            };
            init_volume(&mpv, &status, initial_volume);

            let active_file_projection = items.iter().any(|slot| slot.item.is_audiobookshelf_any());
            let load_indices: Vec<_> = if active_file_projection {
                vec![start_idx]
            } else {
                queue_load_indices(items.len(), start_idx).collect()
            };
            let mut active_prepared_source = None;
            for i in load_indices {
                let item = &items[i].item;
                let prepared = match prepare_source(
                    item,
                    &server_url,
                    &token,
                    audiobookshelf_context.as_ref(),
                ) {
                    Ok(source) => source,
                    Err(error) => {
                        status.lock().unwrap().active = false;
                        let _ = event_tx.send(PlayerEvent::Stopped {
                            slot_id: None,
                            position_ticks: 0,
                            played: false,
                            consume: false,
                            progress_report_accepted: false,
                            error: Some(format!("failed to prepare media: {error}")),
                        });
                        return;
                    }
                };
                let (mode, index) = queue_load_location(i, start_idx);
                let opts = prepared.mpv_load_options(item);
                if let Err(e) =
                    mpv.command("loadfile", &[prepared.url.as_str(), mode, &index, &opts])
                {
                    log::warn!(
                        target: "player",
                        "submit_queue loadfile error: {e} | mode={mode}",
                    );
                    if i == start_idx {
                        let mut prepared = prepared;
                        prepared.close(0.0);
                        status.lock().unwrap().active = false;
                        let _ = event_tx.send(PlayerEvent::Stopped {
                            slot_id: None,
                            position_ticks: 0,
                            played: false,
                            consume: false,
                            progress_report_accepted: false,
                            error: Some(format!("failed to load media: {e}")),
                        });
                        return;
                    }
                }
                if i == start_idx {
                    active_prepared_source = Some(prepared);
                }
            }
            if !active_file_projection {
                // Design D3 load-then-play: every load above was no-play, so
                // playback starts here, at the fully built playlist's start
                // slot — reassert below must now observe Ok (a mismatch log
                // means the no-play plan drifted).
                start_queue_playback(&mpv, start_idx);
                reassert_queue_layout(&mpv, start_idx, items.len());
            }
            // send_ep_info only for Emby items.
            if let Some(emby) = items.get(start_idx).and_then(|slot| slot.item.as_emby()) {
                send_ep_info(&mpv, emby);
            }
            observe_properties(&mpv, config.use_mpv_config);

            // Set up reporter based on client availability and item variant.
            let (reporter, progress) = if let Some(client) = client {
                if let Some(emby) = items.get(start_idx).and_then(|slot| slot.item.as_emby()) {
                    let info = client.get_playback_info(&emby.id);
                    let reporter = SessionReporter::new(
                        client.clone(),
                        ws_tx,
                        ItemId::new(emby.id.clone()),
                        info.media_source_id.clone(),
                        info.session_id.clone(),
                        emby.is_audio(),
                        status.clone(),
                    );
                    let progress = spawn_progress_reporter(reporter.clone());
                    {
                        let c = client;
                        let item = emby.clone();
                        let msid = info.media_source_id;
                        let sid = info.session_id;
                        thread::spawn(move || {
                            let ok = c.report_start(&item, &msid, &sid);
                            if !ok {
                                log::warn!(
                                    target: "player",
                                    "report_start failed for item={}",
                                    item.id,
                                );
                            }
                        });
                    }
                    (reporter, progress)
                } else {
                    // Feed item with client available — clear session so
                    // all report calls are no-ops.
                    let reporter = SessionReporter::new(
                        client,
                        ws_tx,
                        ItemId::empty(),
                        MediaSourceId::new(""),
                        EmbySessionId::new(""),
                        items[start_idx].item.is_audio(),
                        status.clone(),
                    );
                    reporter.clear_session();
                    let (noop_stop_tx, _noop_stop_rx) = mpsc::channel();
                    let progress = ProgressGuard {
                        stop_tx: noop_stop_tx,
                        handle: None,
                    };
                    (reporter, progress)
                }
            } else {
                // No client (feed-only): no-op reporter.
                let client = Arc::new(EmbyClient::new(crate::config::Config::default()));
                let reporter = SessionReporter::new(
                    client,
                    None,
                    ItemId::empty(),
                    MediaSourceId::new(""),
                    EmbySessionId::new(""),
                    items[start_idx].item.is_audio(),
                    status.clone(),
                );
                reporter.clear_session();
                let (noop_stop_tx, _noop_stop_rx) = mpsc::channel();
                let progress = ProgressGuard {
                    stop_tx: noop_stop_tx,
                    handle: None,
                };
                (reporter, progress)
            };

            let session = PlaybackRun::new_from_slot_items(
                items,
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
                active_prepared_source,
            );
            session.run(
                mpv,
                stop_rx,
                cmd_rx,
                progress,
                wakeup_read_fd,
                wakeup_write_fd,
            );
        });
        *self.thread_handle.lock().unwrap() = Some(handle);
        true
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
