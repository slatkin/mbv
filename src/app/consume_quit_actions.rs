use super::App;

impl App {
    /// `q` (and every keyboard/mouse path that routes here). In stay-alive
    /// mode this is a **detach** only while the current session still has
    /// `Stay alive on exit` enabled: diverted before `player.stop()`, the
    /// player keeps running and the run loop keeps going (returns `false`).
    /// If the user disables that setting mid-session, the next `q` becomes
    /// a real quit for the current attached app instance. `mbv -q` / tray-Quit
    /// remain real quits regardless (see `crate::app::stay_alive` / T3's
    /// graceful SIGTERM path).
    ///
    /// In bare mode this is a real quit. Any dirty saved-playlist queue is
    /// saved/discarded **silently** per `save_playlist_on_quit` — no
    /// interactive modal (that modal is reserved for the attended
    /// ClearQueue/PlayItems cases; see issue #156).
    pub(super) fn try_quit(&mut self) -> bool {
        let stay_alive_on_exit = self.client.lock().unwrap().config.stay_alive;
        if stay_alive_on_exit {
            if let Some(ctrl) = &self.stay_alive_ctrl {
                match ctrl.send_detach() {
                    Ok(()) => {
                        self.flash_status("Detached — mbv keeps playing in the background".into());
                        // #156: no terminal-client left to answer the run loop's
                        // terminal.clear()/draw() calls until the next reattach
                        // sets this back via take_attach_pending(); see the
                        // `attached` field doc for why that matters.
                        self.attached = false;
                    }
                    Err(e) => {
                        self.flash_status_high(format!(
                        "Detach failed ({e}) — still attached; try again or `mbv -q` from another shell"
                    ));
                    }
                }
                return false;
            }
        }
        if self.queue_dirty && self.queue_is_saved_playlist() {
            let save_on_quit = self.client.lock().unwrap().config.save_playlist_on_quit;
            if save_on_quit {
                // non-blocking: enqueues save in a spawned thread, does not block quit
                self.save_playlist_to_emby();
                self.queue_dirty = false;
            } else {
                self.on_queue_replace_silent();
            }
        }
        self.save_prefs();
        if !self.player.is_remote() {
            self.player.stop();
        }
        true
    }

    /// Called when a video item is removed from the queue because "consume" is enabled.
    /// Marks the queue dirty (matching how other queue-mutating actions behave, so the
    /// user is prompted to save on quit/replace), and — if the user has opted in via
    /// `save_playlist_on_consume` and the current queue is a saved Emby playlist — pushes
    /// the shorter item list back to Emby immediately, so other devices loading this
    /// playlist see the consumed items already removed instead of stale, longer state.
    ///
    /// Both checks are gated on `local_queue_metadata_applies`: `save_playlist_to_emby`
    /// always pushes `player_tab.items` (the *local* queue), so if the consume actually
    /// happened on a direct-remote/daemon queue, autosaving here would push an unrelated,
    /// unmodified local playlist to Emby instead of the queue that actually changed.
    pub(super) fn on_video_consumed(&mut self) {
        let scope = self.playback_target_queue_scope();
        log::info!(target: "consume", "on_video_consumed: scope={scope:?} has_local_metadata={}",
            self.local_queue_metadata_applies(scope));
        if !self.local_queue_metadata_applies(scope) {
            return;
        }
        self.queue_dirty = true;
        let save_on_consume = self.client.lock().unwrap().config.save_playlist_on_consume;
        let is_saved_playlist = self.queue_is_saved_playlist();
        log::info!(target: "consume", "on_video_consumed: queue_dirty=true save_playlist_on_consume={save_on_consume} \
            is_saved_playlist={is_saved_playlist}");
        if save_on_consume && is_saved_playlist {
            self.queue_dirty = false;
            self.save_playlist_to_emby();
        }
    }

    /// Called when an audio item is removed from the queue because "consume" is enabled.
    /// Mirrors `on_video_consumed`, but is gated on the audio-specific
    /// `save_playlist_on_consume_audio` flag instead — kept as a separate method (rather
    /// than a shared helper with a boolean parameter) so the video and audio consume paths
    /// stay independently readable and don't require the caller to track which flag applies.
    pub(super) fn on_audio_consumed(&mut self) {
        let scope = self.playback_target_queue_scope();
        log::info!(target: "consume", "on_audio_consumed: scope={scope:?} has_local_metadata={}",
            self.local_queue_metadata_applies(scope));
        if !self.local_queue_metadata_applies(scope) {
            return;
        }
        self.queue_dirty = true;
        let save_on_consume = self
            .client
            .lock()
            .unwrap()
            .config
            .save_playlist_on_consume_audio;
        let is_saved_playlist = self.queue_is_saved_playlist();
        log::info!(target: "consume", "on_audio_consumed: queue_dirty=true \
            save_playlist_on_consume_audio={save_on_consume} is_saved_playlist={is_saved_playlist}");
        if save_on_consume && is_saved_playlist {
            self.queue_dirty = false;
            self.save_playlist_to_emby();
        }
    }
}
