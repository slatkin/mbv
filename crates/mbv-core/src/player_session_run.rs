impl PlaybackSession {
    fn run(
        mut self,
        mpv: Mpv,
        stop_rx: mpsc::Receiver<()>,
        cmd_rx: mpsc::Receiver<PlayerCommand>,
        mut progress: ProgressGuard,
    ) {
        let event_tx_panic = self.event_tx.clone();
        let current_idx_panic = self.current_idx;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            loop {
                let mut cancel_stop = false;
                while let Ok(cmd) = cmd_rx.try_recv() {
                    cancel_stop |= self.handle_command(cmd, &mpv, &mut progress);
                }

                if !cancel_stop && self.quit_at.is_none() && stop_rx.try_recv().is_ok() {
                    let _ = mpv.command("quit", &[]);
                    self.quit_at = Some(Instant::now());
                }

                if self
                    .quit_at
                    .is_some_and(|t| t.elapsed() > Duration::from_secs(2))
                {
                    if !self.stop_reported {
                        progress.stop_and_join(self.progress_join_budget());
                        self.stop_report_accepted = self.report_stopped_for_current_context();
                        self.stop_reported = true;
                    }
                    let runtime = self.status.lock().unwrap().runtime_ticks;
                    let is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
                    let (played, consume) = quit_timeout_stop_flags(
                        self.origin,
                        is_audio,
                        self.last_valid_pos,
                        runtime,
                        self.stopped_near_end,
                    );
                    self.status.lock().unwrap().active = false;
                    let _ = self.event_tx.send(PlayerEvent::Stopped {
                        idx: self.current_idx,
                        position_ticks: self.last_valid_pos,
                        played,
                        consume,
                        progress_report_accepted: self.stop_report_accepted,
                        error: None,
                    });
                    return;
                }

                match mpv.wait_event(0.5) {
                    Some(Ok(Event::PropertyChange {
                        name: "volume",
                        change: PropertyData::Double(vol),
                        ..
                    })) => {
                        self.status.lock().unwrap().volume = (vol * vol / 100.0) as i64;
                    }
                    Some(Ok(Event::PropertyChange {
                        change: PropertyData::Double(pos_secs),
                        ..
                    })) => {
                        self.on_time_pos(pos_secs, &mpv);
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "pause",
                        change: PropertyData::Flag(paused),
                        ..
                    })) => {
                        self.status.lock().unwrap().paused = paused;
                        if self.startup_pause_events_to_skip > 0 {
                            self.startup_pause_events_to_skip -= 1;
                            continue;
                        }
                        if self.quit_at.is_none() {
                            let event_name = if paused { "Pause" } else { "Unpause" };
                            self.reporter.report_progress(event_name);
                        }
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "sid",
                        change: PropertyData::Str(s),
                        ..
                    })) => {
                        let id = s.parse::<i64>().unwrap_or(0);
                        log::info!(target: "player", "sid PropertyChange: raw={s:?} parsed={id}");
                        self.status.lock().unwrap().sub_id = id;
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "aid",
                        change: PropertyData::Str(_),
                        ..
                    })) => {
                        refresh_tracks(&mpv, &self.status);
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "mute",
                        change: PropertyData::Flag(m),
                        ..
                    })) => {
                        self.status.lock().unwrap().muted = m;
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "video-params/h",
                        change: PropertyData::Int64(h),
                        ..
                    })) => {
                        log::info!(target: "player", "video-params/h (playlist): h={h}");
                        self.status.lock().unwrap().video_height = h;
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "video-params/h",
                        change,
                        ..
                    })) => {
                        log::warn!(target: "player", "video-params/h (playlist) unexpected type: {:?}", change);
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "audio-codec-name",
                        change: PropertyData::Str(s),
                        ..
                    })) => {
                        self.status.lock().unwrap().audio_codec = s.to_lowercase();
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "current-tracks/video/image",
                        change: PropertyData::Flag(is_img),
                        ..
                    })) => {
                        log::info!(target: "player", "video/image (playlist): is_img={is_img}");
                        self.status.lock().unwrap().video_is_image = is_img;
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "playlist-pos",
                        change: PropertyData::Int64(pos),
                        ..
                    })) => {
                        self.on_playlist_pos_changed(pos);
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "playlist-count",
                        change: PropertyData::Int64(count),
                        ..
                    })) => {
                        if self.pending_load == 0 {
                            self.on_playlist_count_changed(count as usize);
                        }
                    }
                    Some(Ok(Event::PlaybackRestart)) => {
                        self.on_playback_restart(&mpv);
                    }
                    Some(Ok(Event::EndFile(reason))) => {
                        let should_continue = self.on_end_file(reason, &mpv, &mut progress);
                        // on_end_file returns false both for "continue normally" and for
                        // "end of playlist — return from thread". Detect end-of-playlist
                        // by checking active flag which on_end_file sets to false.
                        if !should_continue && !self.status.lock().unwrap().active {
                            return;
                        }
                        if should_continue {
                            continue;
                        }
                    }
                    Some(Ok(Event::LogMessage {
                        prefix,
                        level,
                        text,
                        ..
                    })) => {
                        let t = text.trim_end();
                        if !t.is_empty() {
                            log::warn!(target: "mpv", "[{}/{}] {}", prefix, level, t);
                        }
                    }
                    Some(Ok(Event::ClientMessage(args)))
                        if args.first().copied() == Some("mbv-next-up-play") =>
                    {
                        log::info!(target: "player", "next-up: mbv-next-up-play received from Lua");
                        self.next_up_jump = true;
                        let _ = self.event_tx.send(PlayerEvent::NextUpPlay);
                    }
                    Some(Ok(Event::ClientMessage(args)))
                        if args.first().copied() == Some("mbv-skip-intro-play") =>
                    {
                        let _ = self.event_tx.send(PlayerEvent::SkipIntroPlay);
                    }
                    Some(Ok(Event::ClientMessage(args)))
                        if self.config.use_mpv_config
                            && args.first().copied() == Some("mouse-moved") =>
                    {
                        let show = self
                            .last_mouse_osd
                            .is_none_or(|t: Instant| t.elapsed() > Duration::from_secs(3));
                        if show {
                            let _ = mpv.command("show-text", &[&self.osd_title, "2000"]);
                            self.last_mouse_osd = Some(Instant::now());
                        }
                    }
                    Some(Ok(Event::Shutdown)) => {
                        self.on_shutdown(&mut progress);
                        return;
                    }
                    Some(Err(e)) => {
                        log::warn!(target: "player", "event error: {}", mpv_err_str(&e));
                    }
                    _ => {}
                }
            }
        })); // end catch_unwind
        if let Err(panic) = result {
            let msg = panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            log::error!(target: "player", "PlaybackSession panicked: {msg}");
            let _ = event_tx_panic.send(PlayerEvent::Stopped {
                idx: current_idx_panic,
                position_ticks: 0,
                played: false,
                consume: false,
                progress_report_accepted: false,
                error: Some(msg),
            });
        }
    }
}
