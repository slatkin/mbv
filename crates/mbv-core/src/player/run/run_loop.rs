use super::super::{
    mpsc, mpv_position_ticks, quit_timeout_stop_flags, refresh_tracks, thread, Duration, Event,
    Instant, Ordering, PlayerCommand, PlayerEvent, PreparedSource, PropertyData,
};
use super::{PlaybackRun, ProgressGuard};
use crate::playback_queue::QueueItem;
use libmpv2::Mpv;
use std::os::unix::io::RawFd;

fn command_quit_async(mpv: &Mpv) {
    let quit = std::ffi::CString::new("quit").unwrap();
    let mut args = [quit.as_ptr(), std::ptr::null()];
    // SAFETY: `mpv` owns a live context, and both argument pointers refer to
    // NUL-terminated strings that stay alive for the duration of this call.
    unsafe {
        libmpv2_sys::mpv_command_async(mpv.ctx.as_ptr(), 0, args.as_mut_ptr());
    }
}

/// Worst-case mpv event latency when a wakeup byte is lost.
///
/// libmpv's wakeup callback may fire *before* the event is deliverable by
/// `wait_event`, so its byte can be drained while `wait_event(0.0)` still
/// returns `None`. mpv then writes no second byte for that event, and the
/// client blocks for the whole timeout — which is how a 2000 ms poll turned
/// into "mpv takes 2 s to notice next/prev". The pipe stays a latency
/// optimisation; this ceiling, not an idle interval, is the real contract.
const WAKEUP_POLL_MS: i32 = 50;

fn poll_wakeup(fd: RawFd, timeout_ms: i32) {
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: `pfd` is a valid, writable pollfd for this call; the caller
    // passes an open self-pipe descriptor owned by the playback run.
    unsafe {
        libc::poll(&raw mut pfd, 1, timeout_ms);
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
    // wakeup_read_fd/wakeup_write_fd are the two ends of a self-pipe created by
    // the caller (Player::play/play_queue). -1 means pipe(2) creation failed;
    // the loop falls back to a bounded sleep so it still makes progress, just
    // without the immediate wakeup.
    pub(in crate::player) fn run(
        mut self,
        mut mpv: Mpv,
        stop_rx: &mpsc::Receiver<()>,
        cmd_rx: &mpsc::Receiver<PlayerCommand>,
        mut progress: ProgressGuard,
        wakeup_read_fd: RawFd,
        wakeup_write_fd: RawFd,
    ) {
        let event_tx_panic = self.event_tx.clone();

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
                // SAFETY: the run keeps the self-pipe write descriptor open
                // until the callback is removed with its Mpv instance; `byte`
                // points to one readable byte for this write.
                unsafe {
                    libc::write(wakeup_write_fd, byte.as_ptr().cast::<libc::c_void>(), 1);
                }
            });
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| loop {
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
        })); // end catch_unwind
        if wakeup_read_fd >= 0 {
            // SAFETY: this run exclusively owns the read descriptor and closes
            // it once, after its polling loop has stopped.
            unsafe {
                libc::close(wakeup_read_fd);
            }
        }
        if let Err(panic) = result {
            let msg = panic
                .downcast_ref::<&str>()
                .map(ToString::to_string)
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            log::error!(target: "player", "PlaybackRun panicked: {msg}");
            let _ = event_tx_panic.send(PlayerEvent::Stopped {
                // Panic teardown: the run's queue is gone with the unwound
                // stack, so no slot identity can be resolved.
                slot_id: None,
                run_identity: self.run_identity,
                position_ticks: 0,
                played: false,
                consume: false,
                progress_report_accepted: false,
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
        while let Some(event_result) = mpv.wait_event(0.0) {
            had_event = true;
            if self.handle_mpv_event(event_result, mpv, progress, progress_report_tx) {
                return true;
            }
        }

        if !had_event {
            if wakeup_read_fd >= 0 {
                poll_wakeup(wakeup_read_fd, WAKEUP_POLL_MS);
                drain_wakeup(wakeup_read_fd);
            } else {
                std::thread::sleep(Duration::from_millis(WAKEUP_POLL_MS as u64));
            }
        }
        false
    }

    /// A synchronous quit exits promptly for regular playback.
    /// FIFO output can block mpv in its audio thread, so keep
    /// that path asynchronous and let the bounded fallback
    /// handle a delayed Shutdown event.
    fn handle_stop_request(&mut self, mpv: &Mpv) {
        if self.config.audio_pipe_path.is_some() {
            command_quit_async(mpv);
        } else {
            let _ = mpv.command("quit", &[]);
        }
        self.quit_at = Some(Instant::now());
        // Pin the slot identity now, while it is still the observed
        // active slot, so a QueueMove/QueueRemove drained before the
        // deferred Stopped emit cannot rename the occurrence (D2).
        self.stop_slot = self.active_slot_id();
        self.stop_runtime = self.active_item().map(QueueItem::runtime_ticks);
    }

    /// Emit the final `Stopped` after the quit timeout elapsed.
    fn finalize_quit(&mut self, progress: &mut ProgressGuard) {
        if !self.stop_report.is_sent() {
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
        self.status.lock().unwrap().active = false;
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: self.stop_slot.or_else(|| self.active_slot_id()),
            run_identity: self.run_identity,
            position_ticks: self.last_valid_pos,
            played,
            consume,
            progress_report_accepted: self.stop_report.is_accepted(),
            error: None,
        });
    }

    /// Dispatch one mpv event. Returns true when `run` should return
    /// (end of playlist, shutdown, or unrecoverable error).
    fn handle_mpv_event(
        &mut self,
        event_result: Result<Event<'_>, libmpv2::Error>,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
        progress_report_tx: &mpsc::Sender<String>,
    ) -> bool {
        match event_result {
            Ok(Event::PropertyChange { name, change, .. }) => {
                self.on_property_change(name, change, mpv, progress_report_tx);
                false
            }
            Ok(Event::PlaybackRestart) => {
                self.on_playback_restart(mpv);
                false
            }
            Ok(Event::EndFile(reason)) => {
                let should_continue = self.on_end_file(reason, mpv, progress);
                // on_end_file returns false both for "continue normally" and for
                // "end of playlist — return from thread". Detect end-of-playlist
                // by checking active flag which on_end_file sets to false.
                if !should_continue && !self.status.lock().unwrap().active {
                    return true;
                }
                false
            }
            Ok(Event::LogMessage {
                prefix,
                level,
                text,
                ..
            }) => {
                if let Some(t) = mpv_log_text(
                    self.prepared_source
                        .as_ref()
                        .is_some_and(PreparedSource::has_sensitive_lifecycle),
                    text,
                ) {
                    log::warn!(target: "mpv", "[{prefix}/{level}] {t}");
                }
                false
            }
            Ok(Event::ClientMessage(args)) => {
                self.handle_client_message(&args, mpv);
                false
            }
            Ok(Event::Shutdown) => {
                self.on_shutdown(progress);
                true
            }
            Err(e) => {
                if self.on_mpv_error(&e, progress) {
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn handle_client_message(&mut self, args: &[&str], mpv: &Mpv) {
        match args.first().copied() {
            Some("mbv-next-up-play") => {
                log::info!(target: "player", "next-up: mbv-next-up-play received from Lua");
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
        change: PropertyData,
        mpv: &Mpv,
        progress_report_tx: &mpsc::Sender<String>,
    ) {
        match (name, change) {
            ("volume", PropertyData::Double(vol)) => {
                self.status.lock().unwrap().volume =
                    super::super::saturating_i64_from_f64(vol * vol / 100.0);
            }
            (_, PropertyData::Double(pos_secs)) => {
                self.on_time_pos(pos_secs, mpv);
            }
            ("pause", PropertyData::Flag(paused)) => {
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
            ("sid", PropertyData::Str(s)) => {
                let id = s.parse::<i64>().unwrap_or(0);
                log::info!(target: "player", "sid PropertyChange: raw={s:?} parsed={id}");
                self.status.lock().unwrap().sub_id = id;
            }
            ("aid", PropertyData::Str(_)) => {
                refresh_tracks(mpv, &self.status);
            }
            ("mute", PropertyData::Flag(m)) => {
                self.status.lock().unwrap().muted = m;
            }
            ("video-params/h", PropertyData::Int64(h)) => {
                log::info!(target: "player", "video-params/h (playlist): h={h}");
                self.status.lock().unwrap().video_height = h;
            }
            ("video-params/h", change) => {
                log::warn!(target: "player", "video-params/h (playlist) unexpected type: {change:?}");
            }
            ("audio-codec-name", PropertyData::Str(s)) => {
                self.status.lock().unwrap().audio_codec = s.to_lowercase();
            }
            ("current-tracks/video/image", PropertyData::Flag(is_img)) => {
                log::info!(target: "player", "video/image (playlist): is_img={is_img}");
                self.status.lock().unwrap().video_is_image = is_img;
            }
            ("playlist-pos", PropertyData::Int64(pos)) => {
                self.on_playlist_pos_changed(pos, mpv_position_ticks(mpv));
            }
            ("playlist-count", PropertyData::Int64(count)) if self.load_state.is_ready() => {
                self.on_playlist_count_changed(usize::try_from(count).unwrap_or(usize::MAX));
            }
            _ => {}
        }
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
