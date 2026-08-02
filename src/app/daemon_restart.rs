use super::bootstrap::bootstrap_local_daemon_queue;
use super::{App, QueueScope};
use mbv_core::player::PlayerProxy;
use mbv_core::remote_player::{DaemonEndpoint, RemotePlayer};

impl App {
    pub(super) fn reset_local_daemon_queue_view(&mut self) {
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
    /// `resume = false` suppresses the saved-queue adoption so the new
    /// daemon comes up idle -- the `[S]` choice on the daemon-lost modal,
    /// which exists to escape a crash loop where resuming the offending
    /// item re-triggers the crash.
    pub(super) fn restart_local_daemon(&mut self, resume: bool) -> Result<(), String> {
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
                crate::local_daemon::spawn_detached(&socket_path.to_string_lossy())?;
            }
            Ok(crate::single_instance::Resolution::Refuse) => {
                return Err(
                    "another process holds the playback lock without a reachable daemon socket"
                        .to_string(),
                );
            }
            Err(e) => return Err(format!("single-instance check failed: {e}")),
        }

        let auth_token = self.client.lock().unwrap().token.clone();
        let (remote, remote_rx) =
            RemotePlayer::connect_endpoint(&DaemonEndpoint::Local, &auth_token)
                .map_err(|e| format!("failed to attach to local daemon: {e}"))?;

        let remote_items = remote.items.lock().unwrap().clone();
        let remote_cursor = remote.status.lock().unwrap().current_idx;
        let remote_queue_source = remote.queue_source.lock().unwrap().clone();
        let bootstrap = bootstrap_local_daemon_queue(
            remote_items,
            remote_cursor,
            remote_queue_source,
            if resume {
                crate::config::load_queue_state()
            } else {
                None
            },
        );
        let adoption_failed = bootstrap
            .adopt_queue
            .is_some_and(|(items, cursor, source)| !remote.adopt_queue(items, cursor, source));

        // Tear down the old (already-dead) connection before overwriting it,
        // mirroring `restore_local_mode`'s remote-to-remote swap (#233).
        self.player.disconnect_remote();
        let always_play_next = self.client.lock().unwrap().config.always_play_next;
        let mpris_remote = remote.clone();
        self.player = PlayerProxy::remote(remote, always_play_next);
        self.player_rx = remote_rx;
        if let Some(handle) = &self.mpris {
            let disconnected = mpris_remote.disconnected_flag();
            crate::mpris::rebind(
                handle,
                mpris_remote.status.clone(),
                move |cmd| {
                    mpris_remote.send_command(cmd);
                },
                Some(disconnected),
            );
        }

        self.player_tab = bootstrap.player_tab;
        self.reset_local_daemon_queue_view();
        self.queue_source = bootstrap.queue_source;
        self.last_played_item_id = bootstrap.last_played_item_id;
        self.last_played_completed = bootstrap.last_played_completed;
        if !bootstrap.positions.is_empty() {
            self.spawn_enrich_queue_state(bootstrap.positions);
        }
        self.player_endpoint = Some(DaemonEndpoint::Local);
        debug_assert_eq!(self.player.is_remote(), self.player_endpoint.is_some());
        self.next_up_item = None;
        self.skip_intro_end_ticks = None;
        self.daemon_lost_modal = None;

        if adoption_failed {
            self.handle_failed_local_daemon_adoption();
        }
        Ok(())
    }
}
