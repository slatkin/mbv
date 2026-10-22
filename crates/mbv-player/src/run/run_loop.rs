use super::super::{
    Duration, Instant, Mpv, MpvEvent, Ordering, PlayerCommand, PlayerEvent, PreparedSource,
    PropertyValue, mpsc, mpv_position_ticks, quit_timeout_stop_flags, refresh_tracks, thread,
};
use super::{PlaybackRun, ProgressGuard, StopOutcome};
use mbv_core::applog as app_logging;
use mbv_queue::QueueItem;
use std::os::unix::io::RawFd;

/// Worst-case mpv event latency for the run's own timers.
///
/// The libmpv wakeup-byte loss is gone: the IPC socket is level-triggered, so
/// a readable socket stays readable until the run drains it and no event can
/// be missed. This ceiling is not an idle interval; it bounds how long the run
/// waits before it rechecks its timers (the stop request and the quit timeout)
/// when neither the wakeup pipe nor the socket is readable.
const WAKEUP_POLL_MS: i32 = 50;

/// Waits until the wakeup pipe or the mpv socket is readable, or `timeout_ms`
/// passes. `poll` ignores a negative fd, so a missing wakeup pipe leaves the
/// socket as the only source and the ceiling still bounds the timers.
fn poll_readable(wakeup_fd: RawFd, socket_fd: RawFd, timeout_ms: i32) {
    let mut fds = [
        libc::pollfd {
            fd: wakeup_fd,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: socket_fd,
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    // SAFETY: `fds` is a valid, writable two-entry pollfd array for this call;
    // each descriptor is open and owned elsewhere for its whole lifetime.
    unsafe {
        libc::poll(fds.as_mut_ptr(), 2, timeout_ms);
    }
}

fn drain_wakeup(fd: RawFd) {
    let mut buf = [0u8; 64];
    loop {
        // SAFETY: the caller owns this open self-pipe read descriptor, and
        // `buf` supplies `buf.len()` writable bytes for the read.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast::<libc::c_void>(), buf.len()) };
        if n <= 0 {
            break;
        }
    }
}

impl PlaybackRun {
    // wakeup_read_fd is the read end of a self-pipe created by the caller
    // (Player::play/play_queue). Commands arrive on `cmd_rx`, and the sender
    // writes one byte to the pipe to wake the poll below. -1 means pipe(2)
    // creation failed; `poll` ignores it, so the run then wakes on the mpv
    // socket alone and the 50 ms ceiling still bounds its timers.
    pub(crate) fn run(
        mut self,
        mpv: Mpv,
        stop_rx: &mpsc::Receiver<()>,
        cmd_rx: &mpsc::Receiver<PlayerCommand>,
        mut progress: ProgressGuard,
        wakeup_read_fd: RawFd,
    ) {
        let event_tx_panic = self.event_tx.clone();

        // Ordered progress-report worker: pause/unpause events are sent here
        // instead of each spawning its own thread, so two quick toggles can't
        // race and land at Emby out of order. Drains on its own when
        // progress_report_tx is dropped at the end of run().
        let (progress_report_tx, progress_report_rx) = mpsc::channel::<String>();
        let progress_worker_reporter = self.reporter.clone();
        // The progress worker outlives the slot, so it must not inherit the
        // spawning slot's `playback` span — dispatcher only (design D5).
        thread::spawn(app_logging::carry_dispatcher(move || {
            for event_name in progress_report_rx {
                progress_worker_reporter.report_progress(&event_name);
            }
        }));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            loop {
                // Playback correlation span (design D5): re-entered each tick,
                // so a slot change's rebuilt span takes over on the next pass.
                let playback = self.playback_span.clone();
                let _playback = playback.enter();
                if self.run_tick(
                    &mpv,
                    &mut progress,
                    &progress_report_tx,
                    stop_rx,
                    cmd_rx,
                    wakeup_read_fd,
                ) {
                    return;
                }
            }
        })); // end catch_unwind
        if wakeup_read_fd >= 0 {
            // SAFETY: this run exclusively owns the read descriptor and closes
            // it once, after its polling loop has stopped.
            unsafe {
                libc::close(wakeup_read_fd);
            }
        }
        // The session owns the child process, so dropping it here tears the
        // child down at the end of the run, before the panic teardown below
        // (design D5).
        drop(mpv);
        if let Err(panic) = result {
            let msg = panic
                .downcast_ref::<&str>()
                .map(ToString::to_string)
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            tracing::error!(name: "player.playback_run.panicked", target: "player", panic = %msg, "playback run panicked");
            let _ = event_tx_panic.send(PlayerEvent::Stopped {
                // Panic teardown: the run's queue is gone with the unwound
                // stack, so no slot identity can be resolved.
                slot_id: None,
                run_identity: self.run_identity,
                position_ticks: 0,
                played: false,
                consume: false,
                error: Some(msg),
            });
        }
    }

    /// One iteration of the playback event loop. Returns true when `run`
    /// should return (end of playlist, shutdown, or quit timeout).
    fn run_tick(
        &mut self,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
        progress_report_tx: &mpsc::Sender<String>,
        stop_rx: &mpsc::Receiver<()>,
        cmd_rx: &mpsc::Receiver<PlayerCommand>,
        wakeup_read_fd: RawFd,
    ) -> bool {
        let mut cancel_stop = false;
        while let Ok(cmd) = cmd_rx.try_recv() {
            cancel_stop |= self.handle_command(cmd, mpv, progress);
        }
        self.observe_reporting(false);

        if !cancel_stop && self.quit_at.is_none() && stop_rx.try_recv().is_ok() {
            self.handle_stop_request(mpv);
        }

        if self
            .quit_at
            .is_some_and(|t| t.elapsed() > Duration::from_millis(200))
        {
            self.finalize_quit(progress);
            return true;
        }

        let mut had_event = false;
        while let Some(event) = mpv.next_event() {
            had_event = true;
            if self.handle_mpv_event(event, mpv, progress, progress_report_tx) {
                return true;
            }
        }

        if !had_event {
            poll_readable(wakeup_read_fd, mpv.socket_fd(), WAKEUP_POLL_MS);
            if wakeup_read_fd >= 0 {
                drain_wakeup(wakeup_read_fd);
            }
        }
        false
    }

    /// A synchronous quit exits promptly for regular playback.
    /// FIFO output can block mpv in its audio thread, so keep
    /// that path asynchronous and let the bounded fallback
    /// handle a delayed Shutdown event.
    fn handle_stop_request(&mut self, mpv: &Mpv) {
        tracing::info!(name: "player.run.stop_request", target: "player", "run applying a stop request");
        if self.config.audio_pipe_path.is_some() {
            let _ = mpv.command_async("quit", &[]);
        } else {
            let _ = mpv.command("quit", &[]);
        }
        self.quit_at = Some(Instant::now());
        // Pin the slot identity now, while it is still the observed
        // active slot, so a QueueMove/QueueRemove drained before the
        // deferred Stopped emit cannot rename the occurrence (D2).
        self.stop_slot = self.active_slot_id();
        self.pin_stop_runtime();
    }

    /// Emit the final `Stopped` after the quit timeout elapsed, through the
    /// single exit: a run identity that already ended (a parked failed
    /// replacement) is closed again by the lifecycle and reports nothing.
    fn finalize_quit(&mut self, progress: &mut ProgressGuard) {
        if !self.stop_report_sent() {
            self.report_stop_now_or_background(progress);
        }
        let runtime = self
            .stop_runtime
            .or_else(|| self.active_item().map(QueueItem::runtime_ticks))
            .unwrap_or(0);
        let is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
        let (played, consume) = quit_timeout_stop_flags(
            self.origin,
            is_audio,
            self.last_valid_pos,
            runtime,
            self.stopped_near_end,
        );
        self.end_run(StopOutcome {
            slot_id: self.stop_slot.or_else(|| self.active_slot_id()),
            position_ticks: self.last_valid_pos,
            played,
            consume,
            error: None,
        });
    }

    /// Dispatch one mpv event. Returns true when `run` should return
    /// (end of playlist, shutdown, or unrecoverable error).
    fn handle_mpv_event(
        &mut self,
        event: MpvEvent,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
        progress_report_tx: &mpsc::Sender<String>,
    ) -> bool {
        match event {
            MpvEvent::PropertyChange { name, value } => {
                self.on_property_change(&name, value, mpv, progress_report_tx);
                false
            }
            MpvEvent::PlaybackRestart => {
                self.on_playback_restart(mpv);
                false
            }
            MpvEvent::EndFile(reason) => {
                let should_continue = self.on_end_file(reason, mpv, progress);
                // on_end_file returns false both for "continue normally" and for
                // "end of playlist — return from thread". Detect end-of-playlist
                // by checking active flag which on_end_file sets to false.
                if !should_continue && !self.status.lock().unwrap().active {
                    return true;
                }
                false
            }
            MpvEvent::LogMessage {
                prefix,
                level,
                text,
            } => {
                if let Some(t) = mpv_log_text(
                    self.prepared_source
                        .as_ref()
                        .is_some_and(PreparedSource::has_sensitive_lifecycle),
                    &text,
                ) {
                    tracing::warn!(name: "player.mpv.message", target: "mpv", prefix = prefix.as_str(), mpv_level = level.as_str(), message = %t, "mpv log message");
                }
                false
            }
            MpvEvent::ClientMessage(args) => {
                self.handle_client_message(&args, mpv);
                false
            }
            MpvEvent::Shutdown => {
                self.on_shutdown(progress);
                true
            }
            MpvEvent::Failed(failure) => self.on_mpv_error(&failure, progress),
            // Documented no-op: events the run loop does not act on.
            MpvEvent::Ignored => false,
        }
    }

    fn handle_client_message(&mut self, args: &[String], mpv: &Mpv) {
        match args.first().map(String::as_str) {
            Some("mbv-next-up-play") => {
                tracing::info!(name: "player.next_up.play_requested", target: "player", "Lua requested next-up playback");
                self.next_up_jump = true;
                let _ = self.event_tx.send(PlayerEvent::NextUpPlay);
            }
            Some("mbv-skip-intro-play") => {
                let _ = self.event_tx.send(PlayerEvent::SkipIntroPlay);
            }
            Some("mouse-moved") if self.config.use_mpv_config => {
                let show = self
                    .last_mouse_osd
                    .is_none_or(|t: Instant| t.elapsed() > Duration::from_secs(3));
                if show {
                    let _ = mpv.command("show-text", &[&self.osd_title, "2000"]);
                    self.last_mouse_osd = Some(Instant::now());
                }
            }
            _ => {}
        }
    }

    /// Property-change events: status mirror writes and the reporting hooks.
    fn on_property_change(
        &mut self,
        name: &str,
        change: PropertyValue,
        mpv: &Mpv,
        progress_report_tx: &mpsc::Sender<String>,
    ) {
        match (name, change) {
            // An unavailable property (JSON null) carries no value to mirror;
            // the IPC decoder builds `None` for it, and the `video-params/h`
            // unexpected-type arm must not log it (design D8).
            (_, PropertyValue::None) => {}
            ("volume", PropertyValue::Double(vol)) => {
                self.status.lock().unwrap().volume =
                    mbv_emby_model::saturating_i64_from_f64(vol * vol / 100.0);
            }
            (_, PropertyValue::Double(pos_secs)) => {
                self.on_time_pos(pos_secs, mpv);
            }
            ("pause", PropertyValue::Flag(paused)) => {
                self.status.lock().unwrap().paused = paused;
                if self.startup_pause.consume_event() {
                    return;
                }
                let _ = self.event_tx.send(PlayerEvent::PausedChanged(paused));
                self.observe_reporting(true);
                if self.quit_at.is_none() {
                    let event_name = if paused { "Pause" } else { "Unpause" };
                    let _ = progress_report_tx.send(event_name.to_string());
                }
            }
            ("sid", PropertyValue::Str(s)) => {
                self.set_subtitle_id(&s);
            }
            ("aid", PropertyValue::Str(_)) => {
                refresh_tracks(mpv, &self.status);
            }
            ("mute", PropertyValue::Flag(m)) => {
                self.status.lock().unwrap().muted = m;
            }
            ("video-params/h", PropertyValue::Int64(h)) => {
                self.set_video_height(h);
            }
            ("video-params/h", change) => {
                Self::warn_unexpected_video_height(&change);
            }
            ("audio-codec-name", PropertyValue::Str(s)) => {
                self.status.lock().unwrap().audio_codec = s.to_lowercase();
            }
            ("current-tracks/video/image", PropertyValue::Flag(is_img)) => {
                self.set_video_image(is_img);
            }
            ("playlist-pos", PropertyValue::Int64(pos)) => {
                self.on_playlist_pos_changed(pos, mpv_position_ticks(mpv));
            }
            ("playlist-count", PropertyValue::Int64(count)) if self.load_is_ready() => {
                self.on_playlist_count_changed(usize::try_from(count).unwrap_or(usize::MAX));
            }
            _ => {}
        }
    }

    /// Applies a subtitle stream id property change.
    fn set_subtitle_id(&mut self, s: &str) {
        let id = s.parse::<i64>().unwrap_or(0);
        tracing::info!(name: "player.subtitle_id.changed", target: "player", raw_sid = ?s, sid = id, "subtitle id changed");
        self.status.lock().unwrap().sub_id = id;
    }

    /// Applies a video height property change.
    fn set_video_height(&mut self, h: i64) {
        tracing::info!(name: "player.video_height.changed", target: "player", height = h, "video height changed");
        self.status.lock().unwrap().video_height = h;
    }

    fn warn_unexpected_video_height(change: &PropertyValue) {
        tracing::warn!(name: "player.video_height.unexpected_type", target: "player", value = ?change, "unexpected video height property type");
    }

    /// Applies an attached-cover (video image) property change.
    fn set_video_image(&mut self, is_img: bool) {
        tracing::info!(name: "player.video_image.changed", target: "player", is_image = is_img, "video image state changed");
        self.status.lock().unwrap().video_is_image = is_img;
    }
}

fn mpv_log_text(sensitive_source_active: bool, text: &str) -> Option<&str> {
    let text = text.trim_end();
    (!sensitive_source_active && !text.is_empty()).then_some(text)
}

#[cfg(test)]
mod mpv_log_tests {
    use super::mpv_log_text;

    #[test]
    fn suppresses_only_logs_while_sensitive_source_is_active() {
        let diagnostic = "https://books.test/hls/session-id Authorization: Bearer secret";
        assert_eq!(mpv_log_text(true, diagnostic), None);
        assert_eq!(mpv_log_text(false, diagnostic), Some(diagnostic));
        assert_eq!(mpv_log_text(false, "\n"), None);
    }
}

#[cfg(test)]
mod unavailable_property_tests {
    use super::{Mpv, PropertyValue};
    use crate::mpv_session::ipc::IpcSession;
    use crate::tests::make_queue_session_for_pos_tests_with_events;
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;

    /// An unavailable `video-params/h` (JSON null) leaves the status snapshot
    /// alone instead of logging an unexpected type (design D8).
    #[test]
    fn unavailable_video_height_leaves_status_unchanged() {
        let (mut run, status, _events) = make_queue_session_for_pos_tests_with_events(0);
        status.lock().unwrap().video_height = 720;
        let (ours, _peer) = UnixStream::pair().expect("socket pair");
        let session = IpcSession::from_stream(ours).expect("session over a socket pair");
        let mpv = Mpv::for_test(session);
        let (tx, _rx) = mpsc::channel();

        run.on_property_change("video-params/h", PropertyValue::None, &mpv, &tx);

        assert_eq!(status.lock().unwrap().video_height, 720);
    }
}
