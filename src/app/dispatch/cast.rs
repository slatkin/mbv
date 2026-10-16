// Cast attachment lifecycle (5.1, 5.6) and dispatching a played selection to
// the attached receiver instead of the local player (5.3, 5.4). Transport-key
// routing lives in `playback_target_cast.rs`; status polling and progress
// reporting live in `cast_status_actions.rs`; the shared
// resolve-and-connect primitive (`connect_cast_receiver`) that attach-on-
// selection uses lives here too.

use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::panel_targets::PanelTarget;
use crate::app::state::types::cast::{CastAttachment, CastEvent, DispatchedCastItem};
use mbv_audiobookshelf::AudiobookshelfClient;
use mbv_cast::CastError;
use mbv_cast::CastJob;
use mbv_cast::CastTransport;
use mbv_daemon::{AbsCastContext, CastResolveContext, CastSource, Uncastable, prepare_cast_source};
use mbv_queue::{QueueItem, QueueItemContentId};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// Bounded so selecting a cast target from the panel doesn't wait
/// indefinitely for a slow/unreachable receiver -- this runs on a
/// background thread (`App::connect_cast_receiver`), so it never blocks the
/// UI loop regardless.
const CAST_ATTACH_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a single cast discovery browse is allowed to run when the F3
/// panel opens (8.1). Runs concurrently with the `/Sessions` fetch on its
/// own background thread, so this bounds only the cast half of the panel's
/// content, never Emby rows rendering.
const CAST_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(3);

/// One item's cast resolution, still tagged with its original selection
/// index so `partition_dispatch_with_start` can compute where the receiver
/// should begin. The resolve itself lives in the Owner's resolver
/// (`mbv_daemon::cast_resolve`, cast-owner-output task 4.1); the TUI's Cast
/// runtime keeps calling it until row 8.3 deletes that runtime.
struct ResolvedCastItem {
    name: String,
    content_id: QueueItemContentId,
    result: Result<CastSource, Uncastable>,
}

impl App {
    pub(in crate::app) fn is_cast_attached(&self) -> bool {
        self.cast_attachment.is_some()
    }

    /// Attach optimistically: state is set immediately, the live transport
    /// arrives later via `set_cast_client` once an async connect completes
    /// (task 8.3's attach-on-selection and 7.3's reattach both go through
    /// here). Never touches `self.player` (5.1's verification). Stops any
    /// running `PipeWire` capture (8.5): no audio reaches the local graph
    /// once a cast target is attached (`visualizer_should_run`'s
    /// `!self.is_cast_attached()` clause would tear it down on the next
    /// tick regardless, but doing it here covers the frame between attach
    /// and that tick, and both of this function's callers -- selection and
    /// reattach -- need the same teardown).
    pub(in crate::app) fn attach_cast(&mut self, receiver_id: String) {
        self.cast_attachment = Some(CastAttachment {
            receiver_id,
            client: None,
            dispatched: Vec::new(),
            status: None,
            status_at: Instant::now(),
            volume: self.ui_volume.min(100),
            muted: false,
            disconnected: false,
        });
        // A fresh attachment starts a fresh worker; any pending deadline
        // belonged to the old one, and a poll still in flight for it can no
        // longer report (the watchdog reconnect that lands here is the case
        // where no event will ever clear the flag) -- the tick gate
        // (`drains`) must be free to spawn polls for the fresh transport.
        self.cast_job_sent = None;
        self.cast_status_loading = false;
        self.stop_visualizer_capture();
    }

    /// Selecting a target from the F3 panel (design D13): the previously
    /// connected target is severed first (attachment slots are mutually
    /// exclusive), then an Emby target reuses the existing
    /// `connect_to_session` path unchanged; a cast target sends
    /// `CtrlCmd::SelectPlaybackOutput(Cast)` to the owner whose
    /// `playback-output` capability gated the row (D3). The owner performs
    /// the hand-off itself — no `Stop` here, or the active slot the new
    /// receiver must continue would be cleared (spec scenario "A different
    /// cast target is selected while one is attached"). The Client holds no
    /// receiver connection: the owner connects and broadcasts the resulting
    /// `CtrlEvent::PlaybackOutput`.
    pub(in crate::app) fn select_panel_target(&mut self, target: PanelTarget) {
        match target {
            PanelTarget::Emby(session) => self.connect_to_session(&session),
            PanelTarget::Cast(receiver) => {
                self.sever_connections_without_output_change();
                // Same panel-close-on-connect parity as the Emby path
                // (`connect_to_session` dismisses the Sessions sidebar).
                self.request_sidebar_dismiss(mbv_ui_model::overlay::SidebarId::Sessions);
                self.select_playback_output(mbv_ctrl::OutputSelection::Cast {
                    receiver_id: receiver.id,
                    name: receiver.friendly_name,
                    host: receiver.host,
                    port: receiver.port,
                });
            }
        }
    }

    /// Browses for cast receivers on a background thread and reports the
    /// result back as `CastEvent::DiscoveryCompleted` (8.1). Runs
    /// concurrently with `spawn_sessions_load`'s `/Sessions` fetch -- callers
    /// invoke both, never wait on this one before showing Emby rows.
    pub(in crate::app) fn spawn_cast_discovery(&mut self) {
        let tx = self.channels.cast_tx.clone();
        std::thread::spawn(move || {
            let receivers = mbv_cast::discovery::browse_cast_receivers(CAST_DISCOVERY_TIMEOUT);
            let _ = tx.send(CastEvent::DiscoveryCompleted(receivers));
        });
    }

    pub(in crate::app) fn set_cast_client(&mut self, receiver_id: &str, client: Sender<CastJob>) {
        if let Some(attachment) = self.cast_attachment.as_mut()
            && attachment.receiver_id == receiver_id
        {
            attachment.client = Some(client);
        }
    }

    /// Leaves the receiver as it is (design.md "the receiver owns its
    /// queue"): no stop, no teardown call. Subsequent playback returns to
    /// the local player because `playback_target()` stops seeing a cast
    /// attachment (5.6).
    pub(in crate::app) fn detach_cast(&mut self) {
        self.cast_attachment = None;
        self.cast_job_sent = None;
    }

    /// Routes a played selection to the attached receiver instead of the
    /// local player (5.3), surfacing uncastable items by name and reason at
    /// dispatch time (5.4). The receiver is loaded with the single selected
    /// item only: the owner stays queue authority, and one item keeps the
    /// transport message small (a whole-queue `QUEUE_LOAD` oversized the
    /// receiver's message bound and killed the connection -- 2026-10-15
    /// logs). Resolution touches the network (Emby
    /// `PlaybackInfo`, Audiobookshelf session open), so it runs on a
    /// background thread; the result comes back through `cast_rx`
    /// (`handle_cast_event`).
    pub(in crate::app) fn dispatch_selection_to_cast(
        &mut self,
        all_items: Vec<QueueItem>,
        selected_index: usize,
    ) {
        let Some(attachment) = self.cast_attachment.as_ref() else {
            return;
        };
        let receiver_id = attachment.receiver_id.clone();
        let client = attachment.client.clone();
        // The unresponsive-watchdog clock starts at dispatch: it must cover
        // the resolution round trip plus the transport job, and no worker
        // event clears it until one actually arrives.
        self.cast_job_sent.get_or_insert_with(Instant::now);
        let ctx = CastResolveContext {
            emby: self.emby_snapshot(),
            abs: self.audiobookshelf_cast_context(),
        };
        let tx = self.channels.cast_tx.clone();
        std::thread::spawn(move || {
            let resolved: Vec<ResolvedCastItem> = all_items
                .iter()
                .map(|item| resolve_cast_dispatch_item(item, &ctx))
                .collect();
            let (media, uncastable, start_index, dispatched) =
                partition_dispatch_with_start(resolved, selected_index);
            if media.is_empty() {
                let _ = tx.send(CastEvent::Dispatched {
                    receiver_id,
                    outcome: Ok(Vec::new()),
                    uncastable,
                });
                return;
            }
            let Some(client) = client else {
                let _ = tx.send(CastEvent::Dispatched {
                    receiver_id,
                    outcome: Err(CastError::not_connected()),
                    uncastable,
                });
                return;
            };
            let job_tx = tx.clone();
            let sent = client.send(Box::new(move |transport: &mut dyn CastTransport| {
                // A fresh pong immediately before the blocking exchange: the
                // load's reply waits for the receiver to load the first
                // item, and the heartbeat deadline must survive that wait.
                let _ = transport.keep_alive();
                tracing::info!(name: "cast.dispatch.sent", target: "cast", items = media.len(), start_index = %start_index, "loading the selected item on the receiver");
                let outcome = transport
                    .load(&media[usize::from(start_index)], 0.0)
                    .map(|()| dispatched);
                if let Err(error) = &outcome {
                    tracing::warn!(name: "cast.dispatch.failed", target: "cast", error = %error, "cast queue dispatch failed");
                }
                let _ = job_tx.send(CastEvent::Dispatched {
                    receiver_id,
                    outcome,
                    uncastable,
                });
            }));
            if sent.is_err() {
                let _ = tx.send(CastEvent::Dispatched {
                    receiver_id: String::new(),
                    outcome: Err(CastError::worker_gone()),
                    uncastable: Vec::new(),
                });
            }
        });
    }

    /// Plays a selection on the attached receiver (cast-session-control
    /// "Playing while attached"): the owner queue is loaded without
    /// starting playback -- the owner's `Replace` op always starts it -- and
    /// the selection is dispatched to the receiver. Capable owners gate the
    /// dispatch on the load's correlated answer; legacy owners are
    /// dispatched with no owner call at all (the same gate
    /// `submit_queue_item` applies). Returns whether the dispatch was sent.
    pub(in crate::app) fn play_selection_on_cast(
        &mut self,
        items: Vec<QueueItem>,
        start_index: usize,
        source: mbv_queue::QueueSource,
    ) -> bool {
        let answered = self
            .queue_link(self.playing_queue_scope())
            .0
            .remote()
            .supports_answered_queue_ops();
        if answered {
            let Some(request_id) = self.send_idle_queue_load(items.clone(), start_index, source)
            else {
                return false;
            };
            if !self.await_queue_load(request_id) {
                return false;
            }
        }
        self.dispatch_selection_to_cast(items, start_index);
        true
    }

    pub(in crate::app) fn audiobookshelf_cast_context(&self) -> Option<AbsCastContext> {
        let config = self.config.lock().unwrap().clone();
        let (setup, credential) =
            crate::app::dispatch::session::service_startup::audiobookshelf_setup_and_key(&config)?;
        let client = AudiobookshelfClient::new(&setup.server_url).ok()?;
        Some(AbsCastContext {
            client,
            credential,
            device_id: mbv_emby::device_id(),
        })
    }

    /// Resolves `id`'s current address via a fresh discovery browse,
    /// connects, and hands the connected transport to a new cast worker
    /// (`mbv_cast::spawn_cast_worker`), reporting the outcome back over
    /// `cast_tx` as `CastEvent::Connected`/`ConnectFailed`. Runs entirely on
    /// a background thread: both the discovery browse and `CastClient::connect`
    /// are blocking network calls (mirrors `try_daemon_route_connect`'s
    /// blocking-network style), and neither may run on the caller's thread.
    /// Shared by attach-on-selection from the discovery panel (8.3).
    pub(in crate::app) fn connect_cast_receiver(&mut self, id: String) {
        let tx = self.channels.cast_tx.clone();
        let connect = cast_connect_fn();
        std::thread::spawn(move || {
            let event = match connect(&id, CAST_ATTACH_TIMEOUT) {
                Ok(client) => CastEvent::Connected {
                    receiver_id: id,
                    client,
                },
                Err(error) => CastEvent::ConnectFailed {
                    receiver_id: id,
                    error,
                },
            };
            let _ = tx.send(event);
        });
    }

    pub(in crate::app) fn handle_cast_event(&mut self, event: CastEvent) {
        match event {
            CastEvent::Dispatched {
                receiver_id,
                outcome,
                uncastable,
            } => {
                // A worker event proves the worker thread is alive; the
                // unresponsive-worker watchdog (`check_cast_worker_responsive`)
                // reads the deadline this clears. Only an event for the
                // attached receiver clears it: a late event from a previous
                // attachment's worker (the watchdog reconnect race) must not
                // suspend watchdog coverage for this attachment's in-flight
                // job. The `worker_gone` dispatch outcome carries an empty id
                // and also leaves the clock armed -- nothing further will
                // arrive from that worker, so the reconnect is the recovery.
                if self
                    .cast_attachment
                    .as_ref()
                    .is_some_and(|a| a.receiver_id == receiver_id)
                {
                    self.cast_job_sent = None;
                }
                self.apply_cast_dispatch(&receiver_id, outcome, &uncastable);
            }
            CastEvent::StatusUpdated {
                receiver_id,
                status,
            } => {
                if self
                    .cast_attachment
                    .as_ref()
                    .is_some_and(|a| a.receiver_id == receiver_id)
                {
                    self.cast_job_sent = None;
                }
                self.apply_cast_status(&receiver_id, status);
            }
            CastEvent::TransportError(error) => {
                // Carries no receiver id and can only come from a worker that
                // just ran one of this attachment's jobs.
                self.cast_job_sent = None;
                self.flash(
                    format!("Cast command failed: {error}"),
                    ToastSeverity::Error,
                );
            }
            CastEvent::Connected {
                receiver_id,
                client,
            } => {
                // The connected connect re-attaches over the optimistic
                // attachment (8.3).
                self.attach_cast(receiver_id.clone());
                self.set_cast_client(&receiver_id, client);
            }
            CastEvent::ConnectFailed { receiver_id, error } => {
                tracing::warn!(name: "cast.connect.failed", target: "cast", receiver = %receiver_id, error = %error, "cast receiver connection failed");
                // Detach if the failure is for the currently-attached
                // receiver and that attachment is still waiting for its
                // connect (attach-on-selection attaches optimistically; a
                // reattach or a loss-reconnect has no live client either).
                // An attachment that already connected is live: a stale
                // connect's failure (the loss-reconnect racing a manual
                // re-attach) must not kill it.
                if self
                    .cast_attachment
                    .as_ref()
                    .is_some_and(|a| a.receiver_id == receiver_id && a.client.is_none())
                {
                    self.detach_cast();
                }
                self.flash(
                    format!("\u{26a0} Couldn't connect to cast receiver: {error}"),
                    ToastSeverity::Warning,
                );
            }
            CastEvent::DiscoveryCompleted(receivers) => {
                self.cast_receivers = receivers;
                self.rebuild_panel_targets();
            }
        }
    }

    fn apply_cast_dispatch(
        &mut self,
        receiver_id: &str,
        outcome: Result<Vec<DispatchedCastItem>, CastError>,
        uncastable: &[(String, String)],
    ) {
        let attached = self
            .cast_attachment
            .as_ref()
            .is_some_and(|a| a.receiver_id == receiver_id);
        match outcome {
            Ok(dispatched) => {
                if attached && let Some(attachment) = self.cast_attachment.as_mut() {
                    attachment.dispatched = dispatched;
                }
            }
            Err(error) => {
                self.flash(format!("Couldn't cast: {error}"), ToastSeverity::Error);
                return;
            }
        }
        if !uncastable.is_empty() {
            let names = uncastable
                .iter()
                .map(|(name, reason)| format!("{name} ({reason})"))
                .collect::<Vec<_>>()
                .join(", ");
            self.flash(format!("Not cast: {names}"), ToastSeverity::Warning);
        }
    }

    /// Sends one transport command to the attached receiver's worker thread
    /// (5.5): `CastClient`'s operations are blocking network calls, so
    /// running them on the caller's thread would freeze the UI loop for the
    /// round trip -- the job runs on the dedicated worker instead (see
    /// `mbv_cast::spawn_cast_worker`). A no-op when nothing is attached or
    /// the transport hasn't connected yet.
    pub(in crate::app) fn send_cast_command(
        &self,
        f: impl FnOnce(&mut dyn CastTransport) -> Result<(), CastError> + Send + 'static,
    ) {
        let Some(client) = self.cast_attachment.as_ref().and_then(|a| a.client.clone()) else {
            return;
        };
        let tx = self.channels.cast_tx.clone();
        let _ = client.send(Box::new(move |transport: &mut dyn CastTransport| {
            if let Err(e) = f(transport) {
                let _ = tx.send(CastEvent::TransportError(e));
            }
        }));
    }
}

/// Resolves `id` by identifier via a fresh discovery browse -- `None` (no
/// address, no attempt to connect) is treated as unavailable rather than
/// a stale address -- then connects and spawns the worker thread that
/// owns the resulting `CastClient` for the rest of this attachment's life.
/// `spawn_cast_worker`'s fallible `build` (see its doc comment) is what
/// lets `CastClient::connect`'s real failure mode surface here as an `Err`
/// instead of only being discoverable once a job is submitted.
fn resolve_and_connect_cast_receiver(
    id: &str,
    timeout: Duration,
) -> Result<Sender<CastJob>, CastError> {
    let receiver = mbv_cast::discovery::resolve_cast_receiver(id, timeout)
        .ok_or_else(CastError::receiver_not_found)?;
    mbv_cast::spawn_cast_worker(move || {
        mbv_cast::client::CastClient::connect(
            &receiver.host,
            receiver.port,
            mbv_cast::ConnectMode::FirstSelect,
        )
    })
}

#[cfg(test)]
fn cast_connect_fn() -> crate::app::CastConnectFn {
    (*crate::app::CAST_CONNECT_OVERRIDE.lock().unwrap())
        .unwrap_or(resolve_and_connect_cast_receiver)
}

#[cfg(not(test))]
fn cast_connect_fn() -> fn(&str, Duration) -> Result<Sender<CastJob>, CastError> {
    resolve_and_connect_cast_receiver
}

fn resolve_cast_dispatch_item(item: &QueueItem, ctx: &CastResolveContext) -> ResolvedCastItem {
    ResolvedCastItem {
        name: item.display_name(),
        content_id: item.content_id(),
        result: prepare_cast_source(item, ctx),
    }
}

/// Media to load, the uncastable `(name, reason)` pairs, the start index
/// within the *dispatchable* list the receiver should begin at, and the
/// dispatched-item metadata (6.5/6.6) in the same compacted order as the
/// loaded queue -- `partition_dispatch_with_start`'s result.
type CastDispatchPartition = (
    Vec<mbv_cast::client::CastMediaItem>,
    Vec<(String, String)>,
    u16,
    Vec<DispatchedCastItem>,
);

/// Partitions resolved items into a `CastDispatchPartition`, falling back to
/// a start index of 0 when the selected item itself is uncastable or out of
/// range.
fn partition_dispatch_with_start(
    items: Vec<ResolvedCastItem>,
    selected_index: usize,
) -> CastDispatchPartition {
    let mut media = Vec::new();
    let mut uncastable = Vec::new();
    let mut dispatched = Vec::new();
    let mut start_index = 0u16;
    for (i, item) in items.into_iter().enumerate() {
        match item.result {
            Ok(source) => {
                if i == selected_index {
                    start_index = u16::try_from(media.len()).unwrap_or(u16::MAX);
                }
                dispatched.push(DispatchedCastItem {
                    url: source.media.url.clone(),
                    content_id: item.content_id,
                    title: item.name,
                    report: source.progress,
                });
                media.push(source.media);
            }
            Err(reason) => uncastable.push((reason.item, reason.reason)),
        }
    }
    (media, uncastable, start_index, dispatched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::types::cast::CastProgressTarget;
    use crate::app::tests::{QueueViewTestExt, live_owner_channel, make_app_stub};
    use mbv_queue::FeedEntry;

    fn feed_item(guid: &str, url: Option<&str>) -> QueueItem {
        QueueItem::Feed(FeedEntry {
            guid: guid.to_string(),
            title: format!("Episode {guid}"),
            enclosure_url: url.map(str::to_string),
            link: None,
            mime_type: Some("audio/mpeg".to_string()),
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: Some("feed".to_string()),
            position_ticks: 0,
            played: false,
        })
    }

    #[test]
    fn attaching_a_cast_target_stops_a_running_pipewire_capture() {
        // No real `PipeWireWorker` is started here (no audio device in
        // CI) -- `stop_visualizer_capture`'s window teardown is the
        // observable half of "capture teardown runs on attach" (8.5), the
        // same shape `visualizer.rs`'s `selecting_artwork_stops_capture`
        // test uses for the same reason.
        let mut app = make_app_stub();
        app.visualizer_window.samples = vec![mbv_visualizer::StereoSample {
            left: 1.0,
            right: 1.0,
        }];

        app.attach_cast("device-1".to_string());

        assert_eq!(
            app.visualizer_window.samples,
            [] as [mbv_visualizer::StereoSample; 0]
        );
    }

    /// cast-owner-output row 8.1, design D13, spec "Selecting a cast target
    /// makes it the owner's output": selecting a cast row sends
    /// `CtrlCmd::SelectPlaybackOutput(Cast)` to the owner (no Stop — the
    /// owner performs the hand-off), attaches no local transport, dismisses
    /// the F3 panel like the Emby path, and leaves the queue intact.
    #[test]
    fn selecting_a_cast_target_from_the_panel_selects_the_owner_output_and_leaves_the_queue_intact()
    {
        let mut app = make_app_stub();
        let cmd_rx = live_owner_channel(&mut app);
        app.local_view
            .adopt_queue_items(vec![feed_item("a", Some("https://feed/a.mp3"))], 0);
        let before: Vec<String> = app
            .local_view
            .slots()
            .iter()
            .map(|s| s.item.id().to_string())
            .collect();

        let receiver = mbv_cast::discovery::CastReceiver {
            id: "device-1".to_string(),
            friendly_name: "Living Room".to_string(),
            host: "192.168.0.5".to_string(),
            port: 8009,
        };
        app.select_panel_target(PanelTarget::Cast(receiver));

        assert!(matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::Cast {
                ref receiver_id,
                ..
            }) if receiver_id == "device-1"
        ));
        assert!(
            cmd_rx.try_recv().is_err(),
            "no command precedes the selection: the owner performs the hand-off"
        );
        assert!(
            app.cast_attachment.is_none(),
            "the Client holds no receiver connection: the owner does"
        );
        // Emby-path parity: selecting a target dismisses the F3 Sessions
        // panel (`connect_to_session` requests the same dismissal).
        match app.pending_overlay {
            Some(mbv_ui_model::overlay::OverlayRequest::DismissSidebar(
                mbv_ui_model::overlay::SidebarId::Sessions,
            )) => {}
            other => panic!("expected Sessions-sidebar dismissal, got {other:?}"),
        }
        let after: Vec<String> = app
            .local_view
            .slots()
            .iter()
            .map(|s| s.item.id().to_string())
            .collect();
        assert_eq!(before, after);
    }

    /// Selecting a cast target severs the watched session it replaces
    /// (spec "Cast target is selected while a session is watched") but
    /// sends no Cast hand-back: the owner performs the hand-off (D13).
    #[test]
    fn selecting_a_cast_target_severs_a_watched_session() {
        let mut app = make_app_stub();
        app.connected_session_id = Some("sess-1".to_string());
        app.connected_session_state = Some(mbv_emby::test_support::make_session("tv", "mbv"));

        let receiver = mbv_cast::discovery::CastReceiver {
            id: "device-1".to_string(),
            friendly_name: "Living Room".to_string(),
            host: "192.168.0.5".to_string(),
            port: 8009,
        };
        app.select_panel_target(PanelTarget::Cast(receiver));

        assert!(app.connected_session_id.is_none());
        assert!(app.connected_session_state.is_none());
    }

    /// A connect failure for a receiver whose attachment already connected
    /// is stale (a loss-reconnect racing a manual re-attach): it must not
    /// kill the live attachment.
    #[test]
    fn a_stale_connect_failure_does_not_detach_a_connected_attachment() {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string());
        let (job_tx, _calls) =
            mbv_cast::spawn_fake_cast_worker(mbv_cast::FakeCastTransport::default());
        app.set_cast_client("device-1", job_tx);

        app.handle_cast_event(CastEvent::ConnectFailed {
            receiver_id: "device-1".to_string(),
            error: CastError::receiver_not_found(),
        });

        let attachment = app.cast_attachment.as_ref().unwrap();
        assert!(attachment.client.is_some(), "the live attachment survives");
    }

    #[test]
    fn partition_dispatch_with_start_remaps_selected_index_around_uncastable_items() {
        let resolved = vec![
            ResolvedCastItem {
                name: "Uncastable".into(),
                content_id: mbv_queue::QueueItemContentId::Feed("a".into()),
                result: Err(Uncastable {
                    item: "Uncastable".into(),
                    reason: "\"Uncastable\" has no media URL to cast".into(),
                }),
            },
            ResolvedCastItem {
                name: "Castable".into(),
                content_id: mbv_queue::QueueItemContentId::Feed("b".into()),
                result: Ok(CastSource {
                    media: mbv_cast::client::CastMediaItem {
                        url: "https://b".into(),
                        content_type: "audio/mpeg".into(),
                    },
                    progress: CastProgressTarget::Feed {
                        feed_id: None,
                        guid: "b".into(),
                    },
                }),
            },
        ];
        let (media, uncastable, start_index, dispatched) =
            partition_dispatch_with_start(resolved, 1);
        assert_eq!(media.len(), 1);
        assert_eq!(start_index, 0);
        assert_eq!(
            uncastable,
            vec![(
                "Uncastable".to_string(),
                "\"Uncastable\" has no media URL to cast".to_string(),
            )]
        );
        assert_eq!(dispatched.len(), 1);
        assert_eq!(dispatched[0].url, "https://b");
    }
}
