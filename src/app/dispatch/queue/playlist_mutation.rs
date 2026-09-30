//! Playlist mutation tracking, queue-state persistence, and remote sync
//! helpers for [`App`], split out of `queue_actions.rs` to keep that file
//! within the repository's file-size limit.

use super::{
    App, EmbyItem, PlaylistMutation, QueueItem, QueueScope, SessionEvent, ToastSeverity,
    is_playable, queue_op::QueueOpEdit,
};
use crate::app::state::queue_owner::QueueOrigin;
use mbv_emby::EmbyClient;

impl App {
    pub(super) fn start_playlist_mutation(&mut self, playlist_id: &str) {
        if self.discard_stale_playlist_mutation(playlist_id) {
            return;
        }
        self.dispatch_playlist_mutation(playlist_id);
    }

    fn discard_stale_playlist_mutation(&mut self, playlist_id: &str) -> bool {
        let stale = self
            .playlist_mutations
            .get(playlist_id)
            .and_then(|state| state.active.as_ref())
            .is_some_and(|mutation| match mutation {
                PlaylistMutation::Save {
                    origin,
                    source_playlist_id,
                    ..
                } => {
                    !self.origin_is_current(*origin)
                        || self.queue_playlist_id() != Some(source_playlist_id.as_str())
                }
                PlaylistMutation::CreateAs {
                    origin,
                    source_playlist_id,
                    ..
                } => {
                    !self.origin_is_current(*origin)
                        || self.queue_playlist_id() != source_playlist_id.as_deref()
                }
                PlaylistMutation::Replace { origin, .. } => !self.origin_is_current(*origin),
            });
        if stale {
            tracing::debug!(name: "playlist.mutation.stale_discarded", target: "playlist", playlist = %playlist_id, "stale queued playlist mutation discarded");
            if let Some(mutation_id) = self
                .playlist_mutations
                .get(playlist_id)
                .and_then(|state| state.active.as_ref())
                .map(PlaylistMutation::mutation_id)
            {
                self.finish_playlist_mutation(playlist_id, mutation_id);
            }
        }
        stale
    }

    fn dispatch_playlist_mutation(&mut self, playlist_id: &str) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let Some(state) = self.playlist_mutations.get_mut(playlist_id) else {
            return;
        };
        let Some(mutation) = state.active.as_mut() else {
            return;
        };
        let tx = self.channels.sessions_tx.clone();
        let playlist_id = playlist_id.to_string();
        let mutation_id = mutation.mutation_id();
        let ids: Vec<String> = self
            .local_view
            .slots()
            .iter()
            .filter_map(|slot| slot.item.as_emby())
            .map(|item| item.id.clone())
            .collect();
        match mutation {
            PlaylistMutation::Save {
                origin,
                source_playlist_id,
                item_ids,
                ..
            } => {
                *item_ids = Some(ids.clone());
                spawn_playlist_save(
                    client,
                    tx,
                    mutation_id,
                    playlist_id,
                    *origin,
                    source_playlist_id.clone(),
                    ids,
                );
            }
            PlaylistMutation::CreateAs {
                name,
                coordinator_key,
                item_ids,
                origin,
                source_playlist_id,
                ..
            } => {
                *item_ids = Some(ids.clone());
                spawn_playlist_create(
                    client,
                    tx,
                    mutation_id,
                    name.clone(),
                    coordinator_key.clone(),
                    source_playlist_id.clone(),
                    *origin,
                    ids,
                );
            }
            PlaylistMutation::Replace {
                name,
                origin,
                item_ids,
                ..
            } => {
                *item_ids = Some(ids.clone());
                spawn_playlist_replace(
                    client,
                    tx,
                    mutation_id,
                    playlist_id,
                    name.clone(),
                    *origin,
                    ids,
                );
            }
        }
    }

    /// Apply a completed Save As/Overwrite's new queue source (design D5).
    /// Sends a lineage-guarded source-only update to the owner as an answered
    /// queue op addressed to the Local queue's owner link (row 5.3: the
    /// suspended home link when a route or direct-remote peer is attached,
    /// not whichever peer is currently routed), and leaves the queue dirty
    /// until the owner's snapshot confirms it.
    pub(in crate::app) fn apply_saved_playlist_source(
        &mut self,
        source: mbv_queue::QueueSource,
        origin: QueueOrigin,
    ) -> bool {
        let lineage = origin.lineage;
        let sent = self.queue_op(
            QueueScope::Local,
            mbv_remote_player::QueueOp::SourceUpdate {
                source: source.clone(),
                lineage,
            },
        );
        if sent == QueueOpEdit::NotApplied {
            self.flash(
                "Could not update the Stay-alive queue source".into(),
                ToastSeverity::Error,
            );
            return false;
        }
        self.pending_owner_source_update = Some((source, lineage));
        true
    }

    /// Enqueue an explicitly resolved Home Continue Watching item (task 5.3d,
    /// Home effect decoupling): the shared Home branch of the former
    /// `enqueue_selected` (deleted in task 4.3, R1 — the item is now
    /// resolved at the caller), extracted so the CW effects pass the already-
    /// resolved item directly instead of temporarily pointing `home.section`
    /// at section 0 and re-reading `current_home_item`. Folder/non-playable
    /// guards and the ancestor-route resolution match the legacy Home branch
    /// exactly.
    pub(in crate::app) fn enqueue_home_item(&mut self, item: EmbyItem) {
        if item.is_folder {
            self.do_enqueue_folder(&item);
            return;
        }
        if !is_playable(&item) {
            return;
        }
        tracing::info!(name: "library_route.enqueue.requested", target: "library_route", item = %item.id, item_name = %item.name, source = "home", "enqueue requested");
        let resolved = self.route_for_item_via_ancestors(&item.id).map(|(n, _)| n);
        if self.enqueue_route_conflict(resolved.as_ref()) {
            return;
        }
        self.append_item_to_queue(item);
    }

    /// Enqueue an explicitly resolved library-view item (task 5.3d, Album
    /// track focus): the shared Emby branch of `enqueue_selected` (deleted in
    /// task 4.3, R1), extracted so the shell can enqueue a focused album
    /// track whose item resolution lives at the shell/component boundary
    /// rather than in `current_lib_item`.
    pub(in crate::app) fn enqueue_lib_item(&mut self, lib_idx: usize, item: EmbyItem) {
        if item.is_folder {
            self.do_enqueue_folder(&item);
            return;
        }
        if !is_playable(&item) {
            return;
        }
        tracing::info!(name: "library_route.enqueue.requested", target: "library_route", item = %item.id, item_name = %item.name, source = "library_view", "enqueue requested");
        let resolved = self.route_for_active_library_view(lib_idx).map(|(n, _)| n);
        if self.enqueue_route_conflict(resolved.as_ref()) {
            return;
        }
        self.append_item_to_queue(item);
    }

    /// Appends one item to the viewed queue by sending the answered Append op
    /// to that scope's Player owner (row 5.3, design D6): the Client holds no
    /// editable queue, so the entry appears only when the owner's answer (or
    /// a legacy owner's later snapshot) is adopted. Marks local playlist
    /// metadata dirty and advances the queue epoch when the owner took the
    /// edit. The visible-queue mutation is the success confirmation; no
    /// enqueue success toast is emitted.
    pub(super) fn append_item_to_queue(&mut self, item: EmbyItem) {
        let scope = self.viewed_queue_scope();
        self.append_on_owner(scope, vec![QueueItem::Emby(Box::new(item))]);
    }
}

/// Spawn the background Save request for an active playlist mutation and
/// report its completion through `tx`.
fn spawn_playlist_save(
    client: EmbyClient,
    tx: std::sync::mpsc::Sender<SessionEvent>,
    mutation_id: u64,
    playlist_id: String,
    origin: QueueOrigin,
    source_playlist_id: String,
    ids: Vec<String>,
) {
    std::thread::spawn(move || {
        let result = client.update_playlist_items(&playlist_id, &ids);
        let _ = tx.send(SessionEvent::PlaylistMutationComplete {
            mutation_id,
            playlist_id,
            origin,
            source_playlist_id,
            result,
        });
    });
}

/// Spawn the background Create-As request for an active playlist mutation
/// and report its completion through `tx`.
fn spawn_playlist_create(
    client: EmbyClient,
    tx: std::sync::mpsc::Sender<SessionEvent>,
    mutation_id: u64,
    name: String,
    coordinator_key: String,
    source_playlist_id: Option<String>,
    origin: QueueOrigin,
    ids: Vec<String>,
) {
    std::thread::spawn(move || {
        let result = client.create_playlist(&name, &ids);
        let _ = tx.send(SessionEvent::PlaylistCreateComplete {
            mutation_id,
            coordinator_key,
            name,
            origin,
            source_playlist_id,
            result,
        });
    });
}

/// Spawn the background Replace (delete + recreate) request for an active
/// playlist mutation and report its completion through `tx`.
fn spawn_playlist_replace(
    client: EmbyClient,
    tx: std::sync::mpsc::Sender<SessionEvent>,
    mutation_id: u64,
    playlist_id: String,
    name: String,
    origin: QueueOrigin,
    ids: Vec<String>,
) {
    std::thread::spawn(move || {
        let result = client
            .delete_playlist(&playlist_id)
            .and_then(|()| client.create_playlist(&name, &ids));
        let _ = tx.send(SessionEvent::PlaylistReplacementComplete {
            mutation_id,
            playlist_id,
            origin,
            name,
            result,
        });
    });
}
