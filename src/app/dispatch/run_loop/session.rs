//! `SessionEvent` handling, split out of `run_loop_events.rs` to keep that
//! file within the repository's file-size limit.

use crate::app::SidebarId;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::queue_owner::QueueOrigin;
use crate::app::{App, PanelFocus, SessionEvent};

impl App {
    /// Handle a single `SessionEvent` from the sessions-poll channel. Faithful
    /// transcription of the match arms previously inlined in `run()`'s
    /// `sessions_rx` drain loop (see `drain_session_events`).
    pub(in crate::app) fn handle_session_event(&mut self, ev: SessionEvent) {
        match ev {
            SessionEvent::Loaded { sessions } => self.handle_sessions_loaded(sessions),
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
        // Rebuilds the F3 panel's merged Emby+Cast list and
        // re-locates the panel cursor by identity (8.1); this
        // supersedes what used to be a `self.sessions`-only
        // old_id/cursor-clamp here.
        self.rebuild_panel_targets();
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
}
