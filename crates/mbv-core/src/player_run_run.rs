use std::os::unix::io::RawFd;

fn command_quit_async(mpv: &Mpv) {
    let quit = std::ffi::CString::new("quit").unwrap();
    let mut args = [quit.as_ptr(), std::ptr::null()];
    unsafe {
        libmpv2_sys::mpv_command_async(mpv.ctx.as_ptr(), 0, args.as_mut_ptr());
    }
}

fn poll_wakeup(fd: RawFd, timeout_ms: i32) {
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    unsafe {
        libc::poll(&mut pfd, 1, timeout_ms);
    }
}

fn drain_wakeup(fd: RawFd) {
    let mut buf = [0u8; 64];
    loop {
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n <= 0 {
            break;
        }
    }
}

impl PlaybackRun {
    // wakeup_read_fd/wakeup_write_fd are the two ends of a self-pipe created by
    // the caller (Player::play/play_queue). -1 means pipe(2) creation failed;
    // the loop falls back to a bounded sleep so it still makes progress, just
    // without the immediate wakeup.
    fn run(
        mut self,
        mut mpv: Mpv,
        stop_rx: mpsc::Receiver<()>,
        cmd_rx: mpsc::Receiver<PlayerCommand>,
        mut progress: ProgressGuard,
        wakeup_read_fd: RawFd,
        wakeup_write_fd: RawFd,
    ) {
        let event_tx_panic = self.event_tx.clone();
        let current_idx_panic = self.current_idx;

        // Ordered progress-report worker: pause/unpause events are sent here
        // instead of each spawning its own thread, so two quick toggles can't
        // race and land at Emby out of order. Drains on its own when
        // progress_report_tx is dropped at the end of run().
        let (progress_report_tx, progress_report_rx) = mpsc::channel::<String>();
        let progress_worker_reporter = self.reporter.clone();
        thread::spawn(move || {
            for event_name in progress_report_rx {
                progress_worker_reporter.report_progress(&event_name);
            }
        });

        if wakeup_write_fd >= 0 {
            mpv.set_wakeup_callback(move || {
                let byte = [0u8; 1];
                unsafe {
                    libc::write(wakeup_write_fd, byte.as_ptr() as *const libc::c_void, 1);
                }
            });
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            loop {
                let mut cancel_stop = false;
                while let Ok(cmd) = cmd_rx.try_recv() {
                    cancel_stop |= self.handle_command(cmd, &mpv, &mut progress);
                }

                if !cancel_stop && self.quit_at.is_none() && stop_rx.try_recv().is_ok() {
                    // A synchronous quit exits promptly for regular playback.
                    // FIFO output can block mpv in its audio thread, so keep
                    // that path asynchronous and let the bounded fallback
                    // handle a delayed Shutdown event.
                    if self.config.audio_pipe_path.is_some() {
                        command_quit_async(&mpv);
                    } else {
                        let _ = mpv.command("quit", &[]);
                    }
                    self.quit_at = Some(Instant::now());
                }

                if self
                    .quit_at
                    .is_some_and(|t| t.elapsed() > Duration::from_secs(2))
                {
                    if !self.stop_report.is_sent() {
                        self.report_stop_now_or_background(&mut progress);
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
                        progress_report_accepted: self.stop_report.is_accepted(),
                        error: None,
                    });
                    return;
                }

                let mut had_event = false;
                while let Some(event_result) = mpv.wait_event(0.0) {
                    had_event = true;
                    match event_result {
                        Ok(Event::PropertyChange {
                            name: "volume",
                            change: PropertyData::Double(vol),
                            ..
                        }) => {
                            self.status.lock().unwrap().volume = (vol * vol / 100.0) as i64;
                        }
                        Ok(Event::PropertyChange {
                            change: PropertyData::Double(pos_secs),
                            ..
                        }) => {
                            self.on_time_pos(pos_secs, &mpv);
                        }
                        Ok(Event::PropertyChange {
                            name: "pause",
                            change: PropertyData::Flag(paused),
                            ..
                        }) => {
                            self.status.lock().unwrap().paused = paused;
                            if self.startup_pause.consume_event() {
                                continue;
                            }
                            let _ = self.event_tx.send(PlayerEvent::PausedChanged(paused));
                            if self.quit_at.is_none() {
                                let event_name = if paused { "Pause" } else { "Unpause" };
                                let _ = progress_report_tx.send(event_name.to_string());
                            }
                        }
                        Ok(Event::PropertyChange {
                            name: "sid",
                            change: PropertyData::Str(s),
                            ..
                        }) => {
                            let id = s.parse::<i64>().unwrap_or(0);
                            log::info!(target: "player", "sid PropertyChange: raw={s:?} parsed={id}");
                            self.status.lock().unwrap().sub_id = id;
                        }
                        Ok(Event::PropertyChange {
                            name: "aid",
                            change: PropertyData::Str(_),
                            ..
                        }) => {
                            refresh_tracks(&mpv, &self.status);
                        }
                        Ok(Event::PropertyChange {
                            name: "mute",
                            change: PropertyData::Flag(m),
                            ..
                        }) => {
                            self.status.lock().unwrap().muted = m;
                        }
                        Ok(Event::PropertyChange {
                            name: "video-params/h",
                            change: PropertyData::Int64(h),
                            ..
                        }) => {
                            log::info!(target: "player", "video-params/h (playlist): h={h}");
                            self.status.lock().unwrap().video_height = h;
                        }
                        Ok(Event::PropertyChange {
                            name: "video-params/h",
                            change,
                            ..
                        }) => {
                            log::warn!(target: "player", "video-params/h (playlist) unexpected type: {:?}", change);
                        }
                        Ok(Event::PropertyChange {
                            name: "audio-codec-name",
                            change: PropertyData::Str(s),
                            ..
                        }) => {
                            self.status.lock().unwrap().audio_codec = s.to_lowercase();
                        }
                        Ok(Event::PropertyChange {
                            name: "current-tracks/video/image",
                            change: PropertyData::Flag(is_img),
                            ..
                        }) => {
                            log::info!(target: "player", "video/image (playlist): is_img={is_img}");
                            self.status.lock().unwrap().video_is_image = is_img;
                        }
                        Ok(Event::PropertyChange {
                            name: "playlist-pos",
                            change: PropertyData::Int64(pos),
                            ..
                        }) => {
                            self.on_playlist_pos_changed(pos);
                        }
                        Ok(Event::PropertyChange {
                            name: "playlist-count",
                            change: PropertyData::Int64(count),
                            ..
                        }) => {
                            if self.load_state.is_ready() {
                                self.on_playlist_count_changed(count as usize);
                            }
                        }
                        Ok(Event::PlaybackRestart) => {
                            self.on_playback_restart(&mpv);
                        }
                        Ok(Event::EndFile(reason)) => {
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
                        Ok(Event::LogMessage {
                            prefix,
                            level,
                            text,
                            ..
                        }) => {
                            let t = text.trim_end();
                            if !t.is_empty() {
                                log::warn!(target: "mpv", "[{}/{}] {}", prefix, level, t);
                            }
                        }
                        Ok(Event::ClientMessage(args))
                            if args.first().copied() == Some("mbv-next-up-play") =>
                        {
                            log::info!(target: "player", "next-up: mbv-next-up-play received from Lua");
                            self.next_up_jump = true;
                            let _ = self.event_tx.send(PlayerEvent::NextUpPlay);
                        }
                        Ok(Event::ClientMessage(args))
                            if args.first().copied() == Some("mbv-skip-intro-play") =>
                        {
                            let _ = self.event_tx.send(PlayerEvent::SkipIntroPlay);
                        }
                        Ok(Event::ClientMessage(args))
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
                        Ok(Event::Shutdown) => {
                            self.on_shutdown(&mut progress);
                            return;
                        }
                        Err(e) => {
                            log::warn!(target: "player", "event error: {}", mpv_err_str(&e));
                        }
                        _ => {}
                    }
                }

                if !had_event {
                    if wakeup_read_fd >= 0 {
                        poll_wakeup(wakeup_read_fd, 2000);
                        drain_wakeup(wakeup_read_fd);
                    } else {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        })); // end catch_unwind
        if wakeup_read_fd >= 0 {
            unsafe {
                libc::close(wakeup_read_fd);
            }
        }
        if let Err(panic) = result {
            let msg = panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            log::error!(target: "player", "PlaybackRun panicked: {msg}");
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
