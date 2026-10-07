//! `SessionEvent` handling, split out of `run_loop_events.rs` to keep that
//! file within the repository's file-size limit.

use crate::app::SidebarId;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::queue_owner::QueueOrigin;
use crate::app::{App, PanelFocus, SessionEvent};
use std::time::{Duration, Instant};

impl App {
    /// Handle a single `SessionEvent` from the sessions-poll channel. Faithful
    /// transcription of the match arms previously inlined in `run()`'s
    /// `sessions_rx` drain loop (see `drain_session_events`).
    pub(in crate::app) fn handle_session_event(&mut self, ev: SessionEvent) {
        match ev {
            SessionEvent::Loaded { sessions } => self.handle_sessions_loaded(sessions),
            SessionEvent::CommandError { error } => self.flash(
                format!("Remote command failed: {error}"),
                ToastSeverity::Error,
            ),
            SessionEvent::PlaylistMutationComplete {
                mutation_id,
                playlist_id,
                origin,
                source_playlist_id,
                result,
            } => self.handle_playlist_mutation_complete(
                mutation_id,
                &playlist_id,
                origin,
                &source_playlist_id,
                result,
            ),
            SessionEvent::PlaylistReplacementComplete {
                mutation_id,
                playlist_id,
                origin,
                name,
                result,
            } => self.handle_playlist_replacement_complete(
                mutation_id,
                &playlist_id,
                origin,
                &name,
                result,
            ),
            SessionEvent::PlaylistCreateComplete {
                mutation_id,
                coordinator_key,
                name,
                origin,
                source_playlist_id,
                result,
            } => self.handle_playlist_create_complete(
                mutation_id,
                &coordinator_key,
                &name,
                origin,
                source_playlist_id.as_deref(),
                result,
            ),
            SessionEvent::Error(e) => {
                self.sessions_loading = false;
                self.flash(format!("Sessions error: {e}"), ToastSeverity::Error);
            }
        }
    }

    fn handle_sessions_loaded(&mut self, sessions: Vec<mbv_emby::SessionInfo>) {
        self.sessions = sessions;
        self.sessions_loading = false;
        self.remote.last_session_poll = Instant::now();
        // Rebuilds the F3 panel's merged Emby+Cast list and
        // re-locates the panel cursor by identity (8.1); this
        // supersedes what used to be a `self.sessions`-only
        // old_id/cursor-clamp here.
        self.rebuild_panel_targets();
        // Update connected session state; auto-disconnect if gone
        if let Some(ref conn_id) = self.connected_session_id.clone() {
            if let Some(s) = self.sessions.iter().find(|s| &s.id == conn_id).cloned() {
                self.reconcile_connected_session_position(&s);
            } else {
                self.handle_connected_session_miss();
            }
        }
    }

    fn handle_playlist_mutation_complete(
        &mut self,
        mutation_id: u64,
        playlist_id: &str,
        origin: QueueOrigin,
        source_playlist_id: &str,
        result: Result<(), mbv_emby::EmbyError>,
    ) {
        let succeeded = result.is_ok();
        let deferred_action = self.queue_deferrals.take_on_save_complete(mutation_id);
        if let Err(error) = result {
            self.flash(
                format!("Playlist save failed: {error}"),
                ToastSeverity::Error,
            );
        } else if self.origin_is_current(origin)
            && self.queue_playlist_id() == Some(source_playlist_id)
        {
            self.queue_dirty = false;
        }
        self.finish_playlist_mutation(playlist_id, mutation_id);
        if succeeded
            && self.origin_is_current(origin)
            && self.queue_playlist_id() == Some(playlist_id)
            && let Some(action) = deferred_action
        {
            self.execute_pending_queue_action(action);
            self.request_sidebar_dismiss(SidebarId::Playlists);
            self.set_panel_focus(PanelFocus::Queue);
        }
    }

    fn handle_playlist_replacement_complete(
        &mut self,
        mutation_id: u64,
        playlist_id: &str,
        origin: QueueOrigin,
        name: &str,
        result: Result<String, mbv_emby::EmbyError>,
    ) {
        match result {
            Ok(id) if self.origin_is_current(origin) => {
                let source = mbv_queue::QueueSource::Playlist {
                    id: Some(id),
                    name: name.to_string(),
                };
                self.apply_saved_playlist_source(source, origin);
            }
            Ok(_) => {
                tracing::debug!(name: "playlist.replacement.stale_completion_ignored", target: "playlist", "stale playlist replacement completion ignored");
            }
            Err(error) => self.flash(
                format!("Playlist overwrite failed: {error}"),
                ToastSeverity::Error,
            ),
        }
        self.finish_playlist_mutation(playlist_id, mutation_id);
    }

    fn handle_playlist_create_complete(
        &mut self,
        mutation_id: u64,
        coordinator_key: &str,
        name: &str,
        origin: QueueOrigin,
        source_playlist_id: Option<&str>,
        result: Result<String, mbv_emby::EmbyError>,
    ) {
        match result {
            Ok(id)
                if self.origin_is_current(origin)
                    && self.queue_playlist_id() == source_playlist_id =>
            {
                let source = mbv_queue::QueueSource::Playlist {
                    id: Some(id),
                    name: name.to_string(),
                };
                self.apply_saved_playlist_source(source, origin);
            }
            Ok(_) => {
                tracing::debug!(name: "playlist.save_as.stale_completion_ignored", target: "playlist", "stale Save As completion ignored");
            }
            Err(error) => self.flash(
                format!("Playlist save failed: {error}"),
                ToastSeverity::Error,
            ),
        }
        self.finish_playlist_mutation(coordinator_key, mutation_id);
    }

    /// The found-connected-session half of `SessionEvent::Loaded`: maintain
    /// the monotonic position estimate, stamp the canonical queue's active
    /// slot, and arm the fast repoll while runtime is still zero.
    fn reconcile_connected_session_position(&mut self, s: &mbv_emby::SessionInfo) {
        // Maintain a monotonic position estimate within a single video.
        // Reset the anchor only when the playing item ID changes.
        // Avoid keying on runtime or title — the API occasionally returns
        // missing RunTimeTicks (as_i64 returns None → 0) or a slightly
        // different name, which would spuriously reset the position anchor
        // every poll and prevent smooth interpolation.
        let now = Instant::now();
        let prev_item_id = self
            .connected_session_state
            .as_ref()
            .and_then(|p| p.now_playing_item_id.as_deref());
        let item_changed = s.now_playing_item_id.as_deref() != prev_item_id;
        // Detect playback via API position advancing, not IsPaused.
        // Some Emby clients always report IsPaused=true even while playing;
        // the only reliable signal is that PositionTicks keeps moving.
        let prev_api_pos = self
            .connected_session_state
            .as_ref()
            .map_or(0, |p| p.position_s);
        if s.position_s > prev_api_pos {
            self.remote.remote_api_pos_advanced_at = now;
            self.remote.remote_stalled_while_paused = false;
        } else if s.is_paused {
            // Position not advancing AND the session says paused:
            // the transport is genuinely paused. Buggy clients
            // that report IsPaused=true while playing still
            // advance the position each poll, so this branch
            // won't latch on for them.
            self.remote.remote_stalled_while_paused = true;
        }
        // Extrapolate if API advanced recently (within 2× the ~11s report
        // interval). After that window lapses we treat it as paused/stopped.
        let api_active = self.remote.remote_api_pos_advanced_at.elapsed().as_secs() < 22;
        let seek_pending = now < self.remote.remote_seek_pending_until;
        self.apply_remote_position_estimate(s, now, item_changed, seek_pending, api_active);
        if !seek_pending || item_changed {
            self.remote.remote_pos_at = now;
        }
        if item_changed {
            self.remote.runtime_zero_since = None;
        }
        self.connected_session_state = Some(s.clone());
        self.remote.session_miss_count = 0;
        // Remote hasn't started playing yet — repoll sooner.
        // Cap fast-poll at 30 s: if runtime stays 0 that long the
        // remote client likely won't report it and we stop hammering.
        if s.runtime_s == 0 {
            let since = self
                .remote
                .runtime_zero_since
                .get_or_insert_with(Instant::now);
            if since.elapsed() < Duration::from_secs(30) {
                self.remote.last_session_poll = Instant::now()
                    .checked_sub(Duration::from_millis(500))
                    .unwrap_or_else(Instant::now);
            }
        } else {
            self.remote.runtime_zero_since = None;
        }
    }

    /// Dispatches the monotonic position estimate for one poll. Each arm is a
    /// one-event helper so no function carries more than three tracing events.
    fn apply_remote_position_estimate(
        &mut self,
        s: &mbv_emby::SessionInfo,
        now: Instant,
        item_changed: bool,
        seek_pending: bool,
        api_active: bool,
    ) {
        if seek_pending && !item_changed {
            self.hold_remote_position_for_pending_seek(s);
        } else if item_changed {
            self.reset_remote_position_after_item_change(s, now);
        } else if api_active {
            self.advance_extrapolated_remote_position(s);
        } else {
            self.idle_remote_position(s);
        }
    }

    /// A seek was just dispatched; hold the optimistic position until the API
    /// catches up. Once the API reports the new position (or the window
    /// expires) reconciliation falls through to the other arms.
    fn hold_remote_position_for_pending_seek(&self, s: &mbv_emby::SessionInfo) {
        tracing::debug!(name: "sessions.position.held", target: "sessions", api_position_seconds = s.position_s, remote_position_seconds = self.remote.remote_pos_s, "holding position while seek is pending");
    }

    /// The playing item changed; re-anchor the estimate to the API position.
    fn reset_remote_position_after_item_change(&mut self, s: &mbv_emby::SessionInfo, now: Instant) {
        tracing::debug!(name: "sessions.position.reset", target: "sessions", api_position_seconds = s.position_s, previous_remote_position_seconds = self.remote.remote_pos_s, remote_position_seconds = s.position_s, "resetting position after item change");
        self.remote.remote_pos_s = s.position_s;
        self.remote.remote_api_pos_advanced_at = now;
        self.remote.remote_seek_pending_until =
            now.checked_sub(Duration::from_secs(1)).unwrap_or(now);
    }

    /// Interpolate forward from the last anchor, never behind the API position.
    fn advance_extrapolated_remote_position(&mut self, s: &mbv_emby::SessionInfo) {
        let elapsed = self.remote.remote_pos_at.elapsed().as_secs_f64();
        let extrapolated = Self::extrapolated_remote_position(
            self.remote.remote_pos_s,
            self.remote.remote_pos_at.elapsed(),
        );
        let new_pos = s.position_s.max(extrapolated);
        tracing::debug!(name: "sessions.position.extrapolated", target: "sessions", api_position_seconds = s.position_s, paused = s.is_paused, elapsed_seconds = elapsed, previous_remote_position_seconds = self.remote.remote_pos_s, remote_position_seconds = new_pos, "extrapolated remote session position");
        self.remote.remote_pos_s = new_pos;
    }

    /// The API position is not advancing; track it verbatim.
    fn idle_remote_position(&mut self, s: &mbv_emby::SessionInfo) {
        tracing::debug!(name: "sessions.position.idle", target: "sessions", api_position_seconds = s.position_s, previous_remote_position_seconds = self.remote.remote_pos_s, remote_position_seconds = s.position_s, "remote session position is idle");
        self.remote.remote_pos_s = s.position_s;
    }

    /// The missing-connected-session half of `SessionEvent::Loaded`: count
    /// the poll gap and auto-disconnect after three consecutive misses.
    fn handle_connected_session_miss(&mut self) {
        self.remote.session_miss_count += 1;
        // A poll gap means the connected session is not
        // currently observable, but the logical attachment is
        // still held (capable of observing a return).
        if self.remote.session_miss_count >= 3 {
            tracing::warn!(name: "sessions.connection.session_missing", target: "sessions", miss_count = self.remote.session_miss_count, "connected session missing; disconnecting");
            self.flash(
                "Remote session ended; disconnected".to_string(),
                ToastSeverity::Error,
            );
            self.connected_session_id = None;
            self.connected_session_state = None;
            self.remote.session_miss_count = 0;
            self.remote.remote_pos_s = 0;
        } else {
            tracing::warn!(name: "sessions.connection.session_missing", target: "sessions", miss_count = self.remote.session_miss_count, miss_limit = 3, "connected session missing from poll; holding");
        }
    }
}
