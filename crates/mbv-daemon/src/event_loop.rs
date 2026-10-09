use super::{
    AudiobookshelfOwnerContext, ClientRegistry, DaemonEvent, DaemonOwnerContext, DaemonPlayerOwner,
    DaemonRole, EmbyOwnerContext, SharedQueueState, broadcast_queue_state,
    cancel_pending_idle_queue_load_if_run_changed, expire_and_redispatch,
    expire_pending_idle_queue_load, persist_stay_alive_owner_queue,
};
use mbv_ctrl::CtrlEvent;
use mbv_ctrl::player::PlayerEvent;
use mbv_emby::EmbyClient;
use mbv_player::Player;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod control_events;
mod owner_action;
mod pin_swap;
mod player_events;
mod service_events;
mod tray;
pub(crate) use pin_swap::{PendingSwapToken, PinSwapState};
pub(crate) use tray::TrayState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoopFlow {
    Continue,
    Shutdown,
}

/// What one per-event handler reports back to `handle_event`: whether the
/// loop keeps running, and whether the handler changed the canonical owner
/// queue (persisted once by the dispatcher, `DaemonRole::Local` only).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EventOutcome {
    flow: LoopFlow,
    owner_queue_dirty: bool,
}

impl EventOutcome {
    /// The loop keeps running and the owner queue is unchanged.
    const CONTINUE: Self = Self {
        flow: LoopFlow::Continue,
        owner_queue_dirty: false,
    };
    /// The loop keeps running and the canonical owner queue changed.
    const DIRTY: Self = Self {
        flow: LoopFlow::Continue,
        owner_queue_dirty: true,
    };
    /// The daemon is shutting down.
    const SHUTDOWN: Self = Self {
        flow: LoopFlow::Shutdown,
        owner_queue_dirty: false,
    };
}

/// Injected owner-queue persistence hook.
pub(crate) type OwnerQueueStore =
    Box<dyn FnMut(&mbv_config::StayAliveQueueState) -> Result<(), crate::DaemonLibError>>;

/// Owns every local the daemon event loop reads, so one event can be handled
/// without exiting the process (`Shutdown` is returned to the caller).
pub(crate) struct DaemonLoop {
    pub(super) owner: DaemonPlayerOwner,
    pub(super) player: Player,
    pub(super) shared_queue: SharedQueueState,
    pub(super) ctrl_clients: ClientRegistry,
    pub(super) client: Arc<Mutex<EmbyClient>>,
    pub(super) emby_runtime: Option<EmbyOwnerContext>,
    pub(super) audiobookshelf_runtime: Option<AudiobookshelfOwnerContext>,
    pub(super) merged_tx: mpsc::Sender<DaemonEvent>,
    pub(super) ws_send_tx: Option<mbv_ws::WsSender>,
    pub(super) direct_commands: Vec<String>,
    pub(super) owner_settings: super::OwnerSettingsReader,
    pub(super) role: DaemonRole,
    pub(super) audio_only: bool,
    pub(super) last_keepalive: Instant,
    pub(super) last_capabilities: Instant,
    /// Injected owner-queue persistence, so tests can record snapshots instead
    /// of writing real state files.
    pub(super) store: OwnerQueueStore,
    pub(super) queue_persist_tx: Option<mpsc::Sender<super::QueuePersistenceRequest>>,
    /// Tray ownership reconciled against the live owner settings (design D4).
    pub(super) tray: TrayState,
    /// Pin-swap state machine (tray-pin-swap design D3).
    pub(super) pin_swap: PinSwapState,
}

impl DaemonLoop {
    /// Keepalive/capability timers, run once per loop pass.
    pub(super) fn tick(&mut self, now: Instant) {
        if self.emby_runtime.is_some() && self.last_keepalive.elapsed() >= Duration::from_secs(30) {
            if let Some(ws_send_tx) = &self.ws_send_tx {
                let _ = ws_send_tx.send_text("{\"MessageType\":\"KeepAlive\"}".to_string());
            }
            self.last_keepalive = now;
        }
        if self.emby_runtime.is_some()
            && self.last_capabilities.elapsed() >= Duration::from_secs(600)
        {
            let client = self.client.lock().unwrap().clone();
            let direct_commands = self.direct_commands.clone();
            let audio_only = self.audio_only;
            std::thread::spawn(move || {
                client.register_capabilities_with_options(&direct_commands, audio_only);
            });
            self.last_capabilities = now;
        }
        self.tray.poll(now, &self.owner_settings);
        self.pin_swap.poll(now);
    }

    /// Idle work performed when no event arrived within the poll timeout.
    pub(super) fn on_recv_timeout(&mut self, now: Instant) {
        cancel_pending_idle_queue_load_if_run_changed(&mut self.owner, &self.player);
        expire_pending_idle_queue_load(&mut self.owner, now);
        if let Some((connection_id, event)) = self.owner.intents.settle_buffering_if_due() {
            tracing::info!(name: "daemon.pipe_latency.settled", target: "pipe_latency", request = %event.request_id, generation = event.generation, outcome = "settled", "pipe playback settled");
            let clients = self.ctrl_clients.lock().unwrap();
            if clients.has_client(connection_id) {
                clients.send_to_client(connection_id, &CtrlEvent::PlaybackIntent(event));
            } else {
                drop(clients);
                self.owner.intents.invalidate_connection(connection_id);
            }
        }
        expire_and_redispatch(&mut DaemonOwnerContext {
            player: &self.player,
            client: &self.client,
            owner: &mut self.owner,
            shared_queue: &self.shared_queue,
            ctrl_clients: &self.ctrl_clients,
        });
    }

    /// Processes exactly one event. Never exits the process: `Shutdown` is
    /// returned so `run_with_options` can remove the pid file and exit.
    ///
    /// The match stays exhaustive; every arm delegates to a named handler.
    /// The queue-dirty flag is collected here and the owner queue persisted
    /// once after the handler returns, instead of inline per mutation site.
    pub(super) fn handle_event(&mut self, ev: DaemonEvent) -> LoopFlow {
        let outcome = match ev {
            DaemonEvent::Player(PlayerEvent::TrackChanged {
                slot_id,
                transition,
            }) => self.handle_track_changed(slot_id, transition),
            DaemonEvent::Player(PlayerEvent::NextUpThreshold {
                series_id,
                season,
                episode,
            }) => self.handle_next_up_threshold(series_id, season, episode),
            DaemonEvent::Player(PlayerEvent::QueueNextUp { next_idx }) => {
                self.handle_queue_next_up(next_idx)
            }
            DaemonEvent::Player(PlayerEvent::OutputStarted) => self.handle_output_started(),
            DaemonEvent::Player(pe @ PlayerEvent::TrackCompleted { .. }) => {
                self.handle_track_completed(pe)
            }
            DaemonEvent::Player(pe) => self.handle_player_event(pe),
            DaemonEvent::Transport(command) => self.handle_transport(command),
            DaemonEvent::Ws { generation, event } => self.handle_ws_event(generation, event),
            DaemonEvent::QueueEnriched(items) => self.handle_queue_enriched(items),
            DaemonEvent::QueuePersistenceFailed(error) => {
                tracing::error!(name: "daemon.queue_state_persist.failed", target: "queue", error, "failed to persist Stay-alive queue");
                EventOutcome::CONTINUE
            }
            DaemonEvent::AudiobookshelfProgress(update) => {
                self.handle_audiobookshelf_progress(&update)
            }
            DaemonEvent::AudiobookshelfBookProgress(update) => {
                self.handle_audiobookshelf_book_progress(&update)
            }
            DaemonEvent::AudiobookshelfProgressRefreshed {
                generation,
                progress,
                book_progress,
            } => {
                self.handle_audiobookshelf_progress_refreshed(generation, &progress, &book_progress)
            }
            DaemonEvent::Ctrl(cmd, client_id, reply_tx) => {
                self.handle_ctrl_event(cmd, client_id, &reply_tx)
            }
            DaemonEvent::PlaybackResolved {
                start_idx,
                start_ticks,
                source,
                client_id,
                request_id,
                generation,
                fetched,
            } => self.handle_playback_resolved(
                start_idx,
                start_ticks,
                source,
                client_id,
                request_id,
                generation,
                fetched,
            ),
            DaemonEvent::CtrlDisconnected(client_id) => self.handle_ctrl_disconnected(client_id),
            DaemonEvent::PinSwapChildExited { token, success } => {
                self.pin_swap.on_child_exited(&token, success);
                EventOutcome::CONTINUE
            }
            DaemonEvent::PinSwapAdmitted { token } => {
                self.pin_swap.on_swap_admitted(&token, &self.ctrl_clients);
                EventOutcome::CONTINUE
            }
            DaemonEvent::LastClientGone => {
                if self.role == DaemonRole::Local
                    && !(self.owner_settings)().stay_alive
                    && !self.ctrl_clients.lock().unwrap().has_driver()
                {
                    self.handle_shutdown()
                } else {
                    EventOutcome::CONTINUE
                }
            }
            DaemonEvent::Shutdown => self.handle_shutdown(),
        };

        self.persist_owner_queue_if_dirty(outcome.owner_queue_dirty);

        outcome.flow
    }
}

impl DaemonLoop {
    fn handle_transport(&mut self, command: mbv_ctrl::TransportCommand) -> EventOutcome {
        match command {
            mbv_ctrl::TransportCommand::Player(command) => {
                self.player.send_command(command);
            }
            // Owner actions run through the one owner-action handler
            // (tray-pin-swap design D7); the transport path drops the result.
            mbv_ctrl::TransportCommand::OwnerAction(action) => {
                let _ = owner_action::run(self, action);
            }
            mbv_ctrl::TransportCommand::Step(direction) => {
                let target = self.owner.core.relative_step_target(direction);
                if let mbv_player::owner_state::StepTarget::Jump(slot_id) = target {
                    let (request_id, generation) = self.owner.core.transitions.mint_local_id();
                    super::core::dispatch_slot_jump(
                        &mut super::core::DaemonOwnerContext {
                            player: &self.player,
                            client: &self.client,
                            owner: &mut self.owner,
                            shared_queue: &self.shared_queue,
                            ctrl_clients: &self.ctrl_clients,
                        },
                        super::core::JumpOrigin::Transport,
                        mbv_player::transition::Transition::with_cause(
                            request_id,
                            generation,
                            slot_id,
                            mbv_player::transition::TransitionCause::Step(direction),
                        ),
                        None,
                    );
                }
            }
        }
        EventOutcome::CONTINUE
    }

    pub(super) fn handle_ws_step(&mut self, direction: mbv_ctrl::Direction) {
        self.handle_transport(mbv_ctrl::TransportCommand::Step(direction));
    }

    /// Broadcasts the canonical owner queue to every ctrl peer.
    pub(super) fn broadcast_owner_queue_state(&self) {
        broadcast_queue_state(
            &self.ctrl_clients,
            &self.player,
            &self.shared_queue,
            &self.owner.core.queue,
            &self.owner.core.source,
            &self.owner.core.transitions,
            None,
        );
    }

    fn persist_owner_queue_if_dirty(&mut self, owner_queue_dirty: bool) {
        if self.role != DaemonRole::Local || !owner_queue_dirty {
            return;
        }
        if let Some(tx) = &self.queue_persist_tx {
            let snapshot = super::stay_alive_owner_queue_snapshot(
                &self.owner,
                &self.player,
                &self.shared_queue,
            );
            if tx
                .send(super::QueuePersistenceRequest::Save(snapshot))
                .is_err()
            {
                tracing::error!(name: "daemon.queue_state_persist.failed", target: "queue", "queue persistence worker is unavailable");
            }
        } else if let Err(error) = self.persist_owner_queue() {
            tracing::error!(name: "daemon.queue_state_persist.failed", target: "queue", error = %error, "failed to persist Stay-alive queue");
        }
    }

    /// Persists the owner queue through the injected store, returning the
    /// store's error so the caller can log it in context.
    pub(super) fn persist_owner_queue(&mut self) -> Result<(), crate::DaemonLibError> {
        persist_stay_alive_owner_queue(
            &self.owner,
            &self.player,
            &self.shared_queue,
            &mut *self.store,
        )
    }
}
