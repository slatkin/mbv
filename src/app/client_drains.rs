//! The Client-neutral drain bodies (design D7 of `extract-client-core`): every
//! per-tick drain whose work is `App` state, not a Client's surfaces. They sit
//! beside [`crate::app::client_core`], which sequences them: the core owns the
//! order, this module owns the event semantics.
//!
//! A Client's tick host decides *when* a phase runs and owns what follows it —
//! its projections, its mount sync, its own lifecycle. A phase whose event
//! stream carries Client-owned lifecycle hands that event back as
//! [`ClientLifecycle`] instead of acting on it: the TUI Client swaps its
//! terminal, a GUI Client has no terminal or panel to swap.
//!
//! A drain that mixes `App` state with Client-owned state is split here at
//! that seam, named on each method. The shell's Client-owned half is in
//! `crate::app::shell::run::drains`.

use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use mbv_ctrl::player::PlayerEvent;
use mbv_ui_model::playback::QueueLink;

use crate::app::App;
use crate::app::dispatch::session::player_event::PlayerEventFlow;
use crate::app::state::playback::HomeContent;

/// A Client-owned lifecycle event the App hands back instead of handling: the
/// process side it acts on belongs to the Client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientLifecycle {
    /// The Owner asks this Client to prepare for a pin swap.
    SwapPrepare,
    /// The Owner asks this Client to quit because it has been swapped out.
    SwapQuit,
}

impl ClientLifecycle {
    /// The Client-owned event behind one `PlayerEvent`, if it is one. Both
    /// Owner links carry them, and the App must never dispatch them. Every
    /// other Owner event stays App business, so `None` classifies, it does not
    /// hide a variant a Client should act on.
    fn from_player_event(event: &PlayerEvent) -> Option<Self> {
        match event {
            PlayerEvent::SwapPrepare => Some(Self::SwapPrepare),
            PlayerEvent::SwapQuit => Some(Self::SwapQuit),
            _ => None,
        }
    }
}

/// One step of the player-event drain: the active Owner link's events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerEventDrain {
    /// No player event was pending.
    Empty,
    /// The App handled the event. `restart` mirrors
    /// [`PlayerEventFlow::RestartLoop`]: playback asked the loop to restart the
    /// iteration.
    Handled { restart: bool },
    /// A Client-owned lifecycle event; the App never saw it.
    Client(ClientLifecycle),
}

/// One step of the suspended home-link drain: the Owner link a peer
/// connection holds while the App is attached elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeEventDrain {
    /// No suspended home link exists, or it holds no event.
    Empty,
    /// The App handled the event.
    Consumed,
    /// A Client-owned lifecycle event; the App never saw it.
    Client(ClientLifecycle),
}

/// What one startup-worker drain applied.
#[derive(Debug, Default)]
pub struct StartupWorkerDrain {
    /// A completion or a worker disconnect landed.
    pub events: bool,
    /// Home content a completion rebuilt, for the Client that presents Home.
    pub home: Option<HomeContent>,
    /// The drain applied a successful Emby startup, so the client is installed
    /// and Ready. The GUI Client starts its playback-target discovery on this
    /// fact (add-gui-library-panel design D11); the TUI Client's tick is
    /// unaffected and ignores it.
    pub emby_started: bool,
}

/// The startup-drain contract: the App stores its own copy so the
/// presenting Client's assignment is the same facts; a Client that keeps no
/// shell state (the GUI) reads the stored one. A prior completion's snapshot
/// is replaced only when a rebuild ran (row 6.1)
fn store_drained_home(
    app: &mut HomeContent,
    drain: &mut StartupWorkerDrain,
    home: Option<HomeContent>,
) {
    if let Some(content) = home {
        *app = content.clone();
        drain.home = Some(content);
    }
}

impl App {
    /// Drain the Emby startup and Emby setup worker channels. Each channel
    /// delivers its one completion to this drain, which applies it through the
    /// App-side completion handlers; the rebuilt Home content is handed back
    /// for the Client that presents Home, and the App stores its own copy.
    pub fn drain_startup_workers(&mut self) -> StartupWorkerDrain {
        let mut drain = StartupWorkerDrain::default();
        if let Some(worker) = self.setup.emby_startup_rx.take() {
            let generation = worker.generation;
            match worker.rx.try_recv() {
                // Emby bootstrap wrote Home content; a stale or errored
                // completion returns None.
                Ok(completion) => {
                    drain.events = true;
                    let home = self.apply_emby_completion(completion);
                    drain.emby_started |= home.is_some();
                    store_drained_home(&mut self.home_content, &mut drain, home);
                }
                Err(TryRecvError::Empty) => self.setup.emby_startup_rx = Some(worker),
                Err(TryRecvError::Disconnected) => {
                    drain.events = true;
                    self.handle_emby_startup_worker_disconnect(generation);
                }
            }
        }
        if let Some(rx) = self.setup.emby_setup_rx.take() {
            // Emby setup drain re-bootstraps Home content; a stale or declined
            // completion returns None.
            match rx.try_recv() {
                Ok(completion) => {
                    drain.events = true;
                    let home = self.apply_emby_setup_completion(completion);
                    drain.emby_started |= home.is_some();
                    store_drained_home(&mut self.home_content, &mut drain, home);
                }
                Err(TryRecvError::Empty) => self.setup.emby_setup_rx = Some(rx),
                Err(TryRecvError::Disconnected) => {
                    drain.events = true;
                    self.handle_emby_setup_worker_disconnect();
                }
            }
        }
        drain
    }

    /// Drain one event from the suspended home link. It has no tick of its own,
    /// so a Client consumes every pending event before it returns.
    ///
    /// The suspended home link is not the active owner; its output state must
    /// never drive `App::playback_output`, which mirrors the active owner's
    /// last `CtrlEvent::PlaybackOutput` (cast-owner-output design D13). The
    /// active link's dispatch arm in `handle_player_event` owns that state.
    pub fn drain_home_event(&mut self) -> HomeEventDrain {
        let event = if self.suspended_local.is_some() {
            self.deferred_home_events
                .pop_front()
                .or_else(|| self.queue_link(QueueLink::Local).1.try_recv().ok())
        } else {
            None
        };
        let Some(event) = event else {
            return HomeEventDrain::Empty;
        };
        if let Some(lifecycle) = ClientLifecycle::from_player_event(&event) {
            return HomeEventDrain::Client(lifecycle);
        }
        match event {
            PlayerEvent::UnifiedQueueUpdated(snapshot)
            | PlayerEvent::QueueOpResult {
                outcome: mbv_ctrl::QueueOpOutcome::Applied(snapshot),
                ..
            } => self.adopt_home_snapshot(&snapshot),
            PlayerEvent::RemoteDisconnected(_) => {
                self.suspended_local = None;
                self.drop_staged_queue_transfer_on_teardown();
                self.raise_daemon_lost_modal();
            }
            PlayerEvent::DaemonShutdownAnnounced => {
                self.handle_daemon_shutdown_announced(true);
            }
            PlayerEvent::PlaybackOutput(_) => {
                tracing::debug!(name: "home_link.playback_output.ignored", target: "shell", "ignoring playback-output event from the suspended home link");
            }
            _ => {}
        }
        HomeEventDrain::Consumed
    }

    /// Drain one event from the active Owner link, adopting its snapshot. The
    /// answer pump defers non-answer events to `deferred_player_events`; they
    /// replay before the live receiver so tick-level handling stays ordered
    /// (row 5.1).
    pub fn drain_player_event(&mut self) -> PlayerEventDrain {
        let event = self
            .deferred_player_events
            .pop_front()
            .or_else(|| self.player_rx.try_recv().ok());
        let Some(event) = event else {
            return PlayerEventDrain::Empty;
        };
        if let Some(lifecycle) = ClientLifecycle::from_player_event(&event) {
            return PlayerEventDrain::Client(lifecycle);
        }
        let flow = self.handle_player_event(event);
        PlayerEventDrain::Handled {
            restart: flow == PlayerEventFlow::RestartLoop,
        }
    }

    /// Take one pending library event and apply its App-owned half
    /// ([`Self::handle_lib_event`]). This is the drain a Client that keeps no
    /// library content state runs.
    ///
    /// A Client that does keep that state joins its own per-event drain to the
    /// same handler instead: the event tree stays crate-internal, so the drain
    /// that reads the events is the Client's, and the App half is this one
    /// handler for both.
    pub fn drain_lib_event(&mut self) -> bool {
        let Ok(event) = self.channels.lib_rx.try_recv() else {
            return false;
        };
        self.handle_lib_event(event);
        true
    }

    /// Drain ctrl transport commands. Owner actions are resolved inside the
    /// Owner process (tray-pin-swap task 3.5), and the pinned panel belongs to
    /// that process too, which runs the toggle request itself: the Client
    /// never receives either one.
    pub fn drain_transport_events(&mut self) -> bool {
        let mut had_events = false;
        while let Ok(transport) = self.transport_rx.try_recv() {
            had_events = true;
            match transport {
                mbv_ctrl::TransportCommand::Step(direction) => {
                    let _ = self.request_relative_step(direction);
                }
                mbv_ctrl::TransportCommand::Player(command) => {
                    let _ = self.player.send_command(command);
                }
                mbv_ctrl::TransportCommand::Stop => {
                    self.player.stop();
                }
                mbv_ctrl::TransportCommand::OwnerAction(_)
                | mbv_ctrl::TransportCommand::TogglePanel => {}
            }
        }
        had_events
    }

    /// Drain one Emby websocket event into the App. A Client projects the
    /// content it presents after each event; `UserDataChanged` refetches
    /// Continue Watching inside the handler.
    pub fn drain_ws_event(&mut self) -> bool {
        let Ok(event) = self.ws_rx.try_recv() else {
            return false;
        };
        self.handle_ws_event(event);
        true
    }

    /// Drain one Audiobookshelf socket event into the App. A Client projects
    /// the content it presents after each event.
    pub fn drain_audiobookshelf_socket_event(&mut self) -> bool {
        let Ok(event) = self.audiobookshelf_socket_rx.try_recv() else {
            return false;
        };
        self.handle_audiobookshelf_socket_event(event);
        true
    }

    /// Drain idle-feed items and trigger its 30-minute refetch.
    pub fn drain_idle_feed(&mut self) -> bool {
        let Some(ref mut idle_feed) = self.idle_feed else {
            return false;
        };
        let mut had_events = false;
        while let Ok(items) = idle_feed.items_rx.try_recv() {
            had_events = true;
            idle_feed.items = items;
            idle_feed.current_index = 0;
        }
        // Re-fetch every 30 minutes
        if idle_feed.last_fetch.elapsed() >= Duration::from_mins(30) {
            idle_feed.last_fetch = Instant::now();
            self.spawn_idle_feed_fetch();
        }
        had_events
    }

    /// Periodic maintenance between ticks: delayed settings save, websocket
    /// keepalive, capability refresh.
    ///
    /// # Panics
    ///
    /// Panics while saving when the configuration lock is poisoned: a writer
    /// panicked holding it, and the cached configuration cannot be trusted.
    pub fn run_periodic_maintenance(&mut self) {
        if let Some(at) = self.settings_save_at
            && Instant::now() >= at
        {
            let cfg = self.config.lock().unwrap().clone();
            crate::config::save_config_with_ui(&cfg, &self.ui_config_snapshot());
            self.settings_save_at = None;
        }

        // Keep this session visible to other Emby clients
        if let Some(ref tx) = self.ws_send_tx
            && self.last_keepalive.elapsed() >= Duration::from_secs(30)
        {
            let _ = tx.send_text("{\"MessageType\":\"KeepAlive\"}".to_string());
            self.last_keepalive = Instant::now();
        }
        if self.ws_send_tx.is_some() && self.last_capabilities.elapsed() >= Duration::from_secs(600)
        {
            if let Some(client) = self.emby_snapshot() {
                std::thread::spawn(move || client.register_capabilities());
            }
            self.last_capabilities = Instant::now();
        }
    }
}
