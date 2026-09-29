use crate::app::state::bootstrap::{bootstrap_legacy_queue, bootstrap_unified_queue};
use crate::app::{App, QueueScope};
use mbv_player::PlayerProxy;
use mbv_remote_player::{DaemonEndpoint, RemotePlayer};

impl App {
    pub(in crate::app) fn reset_local_daemon_queue_view(&mut self) {
        self.remote_player_tab = None;
        self.remote_queue_undo_stack.clear();
        self.set_queue_scope(QueueScope::Local);
    }

    /// Ensure-daemon-then-attach, run from inside an already-running app
    /// (task 7.3): mirrors `main.rs`'s `Resolution::Fresh`/`Resolution::Attach`
    /// startup branches, but as an in-process reconnect rather than a fresh
    /// process, since the whole daemon *process* may be gone, not just this
    /// client's socket. Only called for a `home_is_local_daemon` app, so
    /// `player_endpoint` is always the managed local daemon after this succeeds.
    ///
    /// The owner reloads its own persisted queue; this Client never seeds it
    /// from its per-user snapshot.
    pub(in crate::app) fn restart_local_daemon(
        &mut self,
    ) -> Result<(), mbv_remote_player::RemotePlayerError> {
        let socket_path = crate::single_instance::socket_path();
        let lock_path = crate::single_instance::lock_path();
        match crate::single_instance::resolve(&socket_path, &lock_path) {
            Ok(crate::single_instance::Resolution::Attach) => {
                // A daemon is already up -- another client raced ahead and
                // restarted it first. The flock arbitrates this; nothing
                // more to do here (design.md decision 6).
            }
            Ok(crate::single_instance::Resolution::Fresh(guard)) => {
                // This process was only a liveness probe: release the lock
                // immediately (the spawned daemon reacquires it for real)
                // before attaching as a client.
                drop(guard);
                crate::local_daemon::spawn_detached(&socket_path.to_string_lossy(), None)?;
            }
            Ok(crate::single_instance::Resolution::Refuse) => {
                return Err(std::io::Error::other(
                    "another process holds the playback lock without a reachable daemon socket",
                )
                .into());
            }
            Err(error) => {
                return Err(std::io::Error::new(
                    error.kind(),
                    format!("single-instance check failed: {error}"),
                )
                .into());
            }
        }

        let (remote, remote_rx) =
            RemotePlayer::connect_endpoint(&DaemonEndpoint::Local).map_err(|error| {
                std::io::Error::other(format!("failed to attach to local daemon: {error}"))
            })?;

        let remote_unified_state = remote.unified_queue_state();
        let bootstrap = remote_unified_state.as_ref().map_or_else(
            || bootstrap_legacy_queue(Vec::new(), 0, mbv_queue::QueueSource::Unknown),
            bootstrap_unified_queue,
        );

        // Tear down the old (already-dead) connection before overwriting it,
        // mirroring `restore_local_mode`'s remote-to-remote swap (#233).
        self.player.disconnect_remote();
        let always_play_next = self.config.lock().unwrap().always_play_next;
        let mpris_remote = remote.clone();
        self.player = PlayerProxy::remote(remote, always_play_next);
        self.player_rx = remote_rx;
        if let Some(handle) = &self.mpris {
            let disconnected = mpris_remote.disconnected_flag();
            mbv_desktop::mpris::rebind(
                handle,
                std::sync::Arc::clone(&mpris_remote.status),
                move |transport| mpris_remote.send_transport(transport),
                Some(disconnected),
            );
        }

        let mut player_tab = bootstrap.player_tab;
        player_tab.adopt_revision_mint(self.player_tab.revision_mint());
        self.player_tab = player_tab;
        self.reset_local_daemon_queue_view();
        self.queue_source = bootstrap.queue_source;
        self.last_played_item_id = bootstrap.last_played_item_id;
        self.last_played_completed = bootstrap.last_played_completed;
        self.player_endpoint = Some(DaemonEndpoint::Local);
        debug_assert_eq!(self.player.is_remote(), self.player_endpoint.is_some());
        self.advance_queue_epoch();
        self.sync_subtitle_prefs_to_player();
        self.next_up_item = None;
        self.dismiss_daemon_lost();

        Ok(())
    }
}
