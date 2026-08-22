// Cast attachment lifecycle (5.1, 5.6) and dispatching a played selection to
// the attached receiver instead of the local player (5.3, 5.4). Transport-key
// routing lives in `playback_target_cast.rs`; status polling and progress
// reporting live in `cast_status_actions.rs`.

use super::notify_actions::ToastSeverity;
use super::panel_targets::PanelTarget;
use super::types_cast::{
    CastAttachment, CastEvent, CastJob, CastProgressTarget, CastTransport, DispatchedCastItem,
};
use super::App;
use mbv_core::api::{EmbyClient, EmbyItem};
use mbv_core::audiobookshelf::AudiobookshelfClient;
use mbv_core::cast_client::CastMediaItem;
use mbv_core::cast_dispatch::{self, build_cast_device_profile, CastSubtitleKind};
use mbv_core::playback_queue::{AudiobookshelfQueueItem, QueueItem};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// Bounded so selecting a cast target from the panel doesn't wait
/// indefinitely for a slow/unreachable receiver -- this runs on a
/// background thread (`App::connect_cast_receiver`), so it never blocks the
/// UI loop regardless. Mirrors `cast_reattach::CAST_REATTACH_TIMEOUT`.
const CAST_ATTACH_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a single cast discovery browse is allowed to run when the F3
/// panel opens (8.1). Runs concurrently with the `/Sessions` fetch on its
/// own background thread, so this bounds only the cast half of the panel's
/// content, never Emby rows rendering.
const CAST_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(3);

/// Everything an Audiobookshelf episode's cast dispatch needs: enough to
/// open a playback session and resolve its castable source, mirroring
/// `AudiobookshelfPlayerContext` (player_sources.rs) without depending on
/// that excluded file.
pub(super) struct AbsCastContext {
    pub(super) client: AudiobookshelfClient,
    pub(super) credential: String,
    device_id: String,
}

/// One item's cast resolution, still tagged with its original selection
/// index so `partition_dispatch_with_start` can compute where the receiver
/// should begin.
struct ResolvedCastItem {
    name: String,
    content_id: mbv_core::playback_queue::QueueItemContentId,
    result: Result<(CastMediaItem, CastProgressTarget), String>,
}

impl App {
    pub(super) fn is_cast_attached(&self) -> bool {
        self.cast_attachment.is_some()
    }

    /// Attach optimistically: state is set immediately, the live transport
    /// arrives later via `set_cast_client` once an async connect completes
    /// (task 8.3's attach-on-selection and 7.3's reattach both go through
    /// here). Never touches `self.player` (5.1's verification). Stops any
    /// running PipeWire capture (8.5): no audio reaches the local graph
    /// once a cast target is attached (`visualizer_should_run`'s
    /// `!self.is_cast_attached()` clause would tear it down on the next
    /// tick regardless, but doing it here covers the frame between attach
    /// and that tick, and both of this function's callers -- selection and
    /// reattach -- need the same teardown).
    pub(super) fn attach_cast(&mut self, receiver_id: String) {
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
        self.stop_visualizer_worker();
    }

    /// Selecting a target from the F3 panel (8.3): an Emby target reuses
    /// the existing `connect_to_session` path unchanged; a cast target
    /// attaches optimistically before its connect completes (matching this
    /// codebase's optimistic-attach convention -- see `types_cast.rs`'s doc
    /// comment on `CastAttachment.client`), then reuses 7.3's shared
    /// resolve-and-connect primitive.
    pub(super) fn select_panel_target(&mut self, target: PanelTarget) {
        match target {
            PanelTarget::Emby(session) => self.connect_to_session(&session),
            PanelTarget::Cast(receiver) => {
                self.attach_cast(receiver.id.clone());
                self.connect_cast_receiver(receiver.id, CAST_ATTACH_TIMEOUT);
            }
        }
    }

    /// Browses for cast receivers on a background thread and reports the
    /// result back as `CastEvent::DiscoveryCompleted` (8.1). Runs
    /// concurrently with `spawn_sessions_load`'s `/Sessions` fetch -- callers
    /// invoke both, never wait on this one before showing Emby rows.
    pub(super) fn spawn_cast_discovery(&mut self) {
        let tx = self.cast_tx.clone();
        std::thread::spawn(move || {
            let receivers = mbv_core::cast_discovery::browse_cast_receivers(CAST_DISCOVERY_TIMEOUT);
            let _ = tx.send(CastEvent::DiscoveryCompleted(receivers));
        });
    }

    pub(super) fn set_cast_client(&mut self, receiver_id: &str, client: Sender<CastJob>) {
        if let Some(attachment) = self.cast_attachment.as_mut() {
            if attachment.receiver_id == receiver_id {
                attachment.client = Some(client);
            }
        }
    }

    /// Leaves the receiver as it is (design.md "the receiver owns its
    /// queue"): no stop, no teardown call. Subsequent playback returns to
    /// the local player because `playback_target()` stops seeing a cast
    /// attachment (5.6).
    pub(super) fn detach_cast(&mut self) {
        self.cast_attachment = None;
    }

    /// Routes a played selection to the attached receiver instead of the
    /// local player (5.3), surfacing uncastable items by name and reason at
    /// dispatch time (5.4). Resolution touches the network (Emby
    /// PlaybackInfo, Audiobookshelf session open), so it runs on a
    /// background thread; the result comes back through `cast_rx`
    /// (`handle_cast_event`).
    pub(super) fn dispatch_selection_to_cast(
        &mut self,
        all_items: Vec<QueueItem>,
        selected_index: usize,
    ) {
        let Some(attachment) = self.cast_attachment.as_ref() else {
            return;
        };
        let receiver_id = attachment.receiver_id.clone();
        let client = attachment.client.clone();
        let emby = self.emby_snapshot();
        let abs = self.audiobookshelf_cast_context();
        let tx = self.cast_tx.clone();
        std::thread::spawn(move || {
            let resolved: Vec<ResolvedCastItem> = all_items
                .iter()
                .map(|item| resolve_cast_dispatch_item(item, emby.as_ref(), abs.as_ref()))
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
                    outcome: Err("not connected to the receiver yet".to_string()),
                    uncastable,
                });
                return;
            };
            let job_tx = tx.clone();
            let sent = client.send(Box::new(move |transport: &mut dyn CastTransport| {
                let outcome = transport
                    .load_queue(&media, start_index)
                    .map(|()| dispatched);
                let _ = job_tx.send(CastEvent::Dispatched {
                    receiver_id,
                    outcome,
                    uncastable,
                });
            }));
            if sent.is_err() {
                let _ = tx.send(CastEvent::Dispatched {
                    receiver_id: String::new(),
                    outcome: Err("cast worker is gone".to_string()),
                    uncastable: Vec::new(),
                });
            }
        });
    }

    pub(super) fn audiobookshelf_cast_context(&self) -> Option<AbsCastContext> {
        let config = self.config.lock().unwrap().clone();
        let (setup, credential) = super::service_startup::audiobookshelf_setup_and_key(&config)?;
        let client = AudiobookshelfClient::new(&setup.server_url).ok()?;
        Some(AbsCastContext {
            client,
            credential,
            device_id: mbv_core::api::device_id(),
        })
    }

    pub(super) fn handle_cast_event(&mut self, event: CastEvent) {
        match event {
            CastEvent::Dispatched {
                receiver_id,
                outcome,
                uncastable,
            } => self.apply_cast_dispatch(receiver_id, outcome, uncastable),
            CastEvent::StatusUpdated {
                receiver_id,
                status,
            } => self.apply_cast_status(receiver_id, status),
            CastEvent::TransportError(error) => {
                self.flash(
                    format!("Cast command failed: {error}"),
                    ToastSeverity::Error,
                );
            }
            CastEvent::Connected {
                receiver_id,
                client,
            } => {
                self.attach_cast(receiver_id.clone());
                self.set_cast_client(&receiver_id, client);
            }
            CastEvent::ConnectFailed { receiver_id, error } => {
                log::warn!(target: "cast", "connect to receiver {receiver_id:?} failed: {error}");
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
        receiver_id: String,
        outcome: Result<Vec<DispatchedCastItem>, String>,
        uncastable: Vec<(String, String)>,
    ) {
        let attached = self
            .cast_attachment
            .as_ref()
            .is_some_and(|a| a.receiver_id == receiver_id);
        match outcome {
            Ok(dispatched) => {
                if attached {
                    if let Some(attachment) = self.cast_attachment.as_mut() {
                        attachment.dispatched = dispatched;
                    }
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
    /// `types_cast::spawn_cast_worker`). A no-op when nothing is attached or
    /// the transport hasn't connected yet.
    pub(super) fn send_cast_command(
        &self,
        f: impl FnOnce(&mut dyn CastTransport) -> Result<(), String> + Send + 'static,
    ) {
        let Some(client) = self.cast_attachment.as_ref().and_then(|a| a.client.clone()) else {
            return;
        };
        let tx = self.cast_tx.clone();
        let _ = client.send(Box::new(move |transport: &mut dyn CastTransport| {
            if let Err(e) = f(transport) {
                let _ = tx.send(CastEvent::TransportError(e));
            }
        }));
    }
}

fn resolve_cast_dispatch_item(
    item: &QueueItem,
    emby: Option<&EmbyClient>,
    abs: Option<&AbsCastContext>,
) -> ResolvedCastItem {
    let name = item.display_name();
    let content_id = item.content_id();
    let result = match item {
        QueueItem::Emby(emby_item) => resolve_emby_cast_item(emby_item, emby),
        QueueItem::Feed(entry) => cast_dispatch::resolve_feed_dispatch(entry).map(|media| {
            (
                media,
                CastProgressTarget::Feed {
                    feed_id: entry.feed_id.clone(),
                    guid: entry.guid.clone(),
                },
            )
        }),
        QueueItem::Audiobookshelf(episode) => resolve_abs_episode_cast_item(episode, abs),
        QueueItem::AudiobookshelfBook(book) => {
            Err(cast_dispatch::resolve_audiobookshelf_book_dispatch(book).unwrap_err())
        }
    };
    ResolvedCastItem {
        name,
        content_id,
        result,
    }
}

fn resolve_emby_cast_item(
    item: &EmbyItem,
    emby: Option<&EmbyClient>,
) -> Result<(CastMediaItem, CastProgressTarget), String> {
    let Some(client) = emby else {
        return Err(format!(
            "\"{}\" needs Emby, which isn't connected",
            item.name
        ));
    };
    let profile = build_cast_device_profile(CastSubtitleKind::None);
    let info = client.get_playback_info_for_cast(&item.id, item.is_audio(), &profile)?;
    Ok((
        info.item,
        CastProgressTarget::Emby {
            item_id: item.id.clone(),
            media_source_id: info.media_source_id,
            session_id: info.session_id,
            runtime_ticks: item.runtime_ticks,
        },
    ))
}

fn resolve_abs_episode_cast_item(
    episode: &AudiobookshelfQueueItem,
    abs: Option<&AbsCastContext>,
) -> Result<(CastMediaItem, CastProgressTarget), String> {
    let Some(ctx) = abs else {
        return Err(format!(
            "\"{}\" needs Audiobookshelf, which isn't connected",
            episode.title
        ));
    };
    let session = ctx
        .client
        .create_playback_session_bounded(
            &ctx.credential,
            &ctx.device_id,
            &episode.library_item_id,
            &episode.episode_id,
            false,
            AudiobookshelfClient::REQUEST_HARD_BOUND,
        )
        .map_err(|e| format!("\"{}\" Audiobookshelf session failed: {e}", episode.title))?;
    let media =
        cast_dispatch::resolve_audiobookshelf_episode_dispatch(&session.source, &ctx.credential)?;
    Ok((
        media,
        CastProgressTarget::AudiobookshelfEpisode {
            session_id: session.id,
            duration_seconds: session.duration_seconds,
        },
    ))
}

/// Media to load, the uncastable `(name, reason)` pairs, the start index
/// within the *dispatchable* list the receiver should begin at, and the
/// dispatched-item metadata (6.5/6.6) in the same compacted order as the
/// loaded queue -- `partition_dispatch_with_start`'s result.
type CastDispatchPartition = (
    Vec<CastMediaItem>,
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
            Ok((cast_media, report)) => {
                if i == selected_index {
                    start_index = media.len() as u16;
                }
                dispatched.push(DispatchedCastItem {
                    url: cast_media.url.clone(),
                    content_id: item.content_id,
                    title: item.name,
                    report,
                });
                media.push(cast_media);
            }
            Err(reason) => uncastable.push((item.name, reason)),
        }
    }
    (media, uncastable, start_index, dispatched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use mbv_core::playback_queue::FeedEntry;

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
    fn attach_and_detach_set_and_clear_without_touching_player() {
        let mut app = make_app_stub();
        assert!(!app.is_cast_attached());
        app.attach_cast("device-1".to_string());
        assert!(app.is_cast_attached());
        assert_eq!(
            app.cast_attachment.as_ref().unwrap().receiver_id,
            "device-1"
        );
        assert!(!app.player.status.lock().unwrap().active);
        app.detach_cast();
        assert!(!app.is_cast_attached());
        assert!(!app.player.status.lock().unwrap().active);
    }

    #[test]
    fn attaching_a_cast_target_stops_a_running_pipewire_capture() {
        // No real `PipeWireWorker` is started here (no audio device in
        // CI) -- `stop_visualizer_worker`'s window teardown is the
        // observable half of "capture teardown runs on attach" (8.5), the
        // same shape `visualizer.rs`'s `selecting_artwork_stops_capture`
        // test uses for the same reason.
        let mut app = make_app_stub();
        app.visualizer_window.samples = vec![crate::app::visualizer_worker::StereoSample {
            left: 1.0,
            right: 1.0,
        }];

        app.attach_cast("device-1".to_string());

        assert!(app.visualizer_window.samples.is_empty());
    }

    #[test]
    fn selecting_a_cast_target_from_the_panel_attaches_and_leaves_the_queue_intact() {
        let _connect_guard = super::super::CAST_CONNECT_TEST_LOCK.lock().unwrap();
        fn connect_stub(_id: &str, _timeout: Duration) -> Result<Sender<CastJob>, String> {
            Err("not reached by this test".to_string())
        }
        *super::super::CAST_CONNECT_OVERRIDE.lock().unwrap() = Some(connect_stub);

        let mut app = make_app_stub();
        app.player_tab.queue = mbv_core::playback_queue::PlaybackQueue::from_queue_items(
            vec![feed_item("a", Some("https://feed/a.mp3"))],
            Some(0),
        );
        let before: Vec<String> = app
            .player_tab
            .queue
            .slots()
            .iter()
            .map(|s| s.item.id().to_string())
            .collect();

        let receiver = mbv_core::cast_discovery::CastReceiver {
            id: "device-1".to_string(),
            friendly_name: "Living Room".to_string(),
            host: "192.168.0.5".to_string(),
            port: 8009,
        };
        app.select_panel_target(PanelTarget::Cast(receiver));

        *super::super::CAST_CONNECT_OVERRIDE.lock().unwrap() = None;

        assert!(app.is_cast_attached());
        assert_eq!(
            app.cast_attachment.as_ref().unwrap().receiver_id,
            "device-1"
        );
        let after: Vec<String> = app
            .player_tab
            .queue
            .slots()
            .iter()
            .map(|s| s.item.id().to_string())
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn partition_dispatch_with_start_remaps_selected_index_around_uncastable_items() {
        let resolved = vec![
            ResolvedCastItem {
                name: "Uncastable".into(),
                content_id: mbv_core::playback_queue::QueueItemContentId::Feed("a".into()),
                result: Err("no url".into()),
            },
            ResolvedCastItem {
                name: "Castable".into(),
                content_id: mbv_core::playback_queue::QueueItemContentId::Feed("b".into()),
                result: Ok((
                    CastMediaItem {
                        url: "https://b".into(),
                        content_type: "audio/mpeg".into(),
                    },
                    CastProgressTarget::Feed {
                        feed_id: None,
                        guid: "b".into(),
                    },
                )),
            },
        ];
        let (media, uncastable, start_index, dispatched) =
            partition_dispatch_with_start(resolved, 1);
        assert_eq!(media.len(), 1);
        assert_eq!(start_index, 0);
        assert_eq!(
            uncastable,
            vec![("Uncastable".to_string(), "no url".to_string())]
        );
        assert_eq!(dispatched.len(), 1);
        assert_eq!(dispatched[0].url, "https://b");
    }

    #[test]
    fn dispatch_to_cast_issues_no_local_player_command_and_flashes_uncastable_reason() {
        use super::super::types_cast::{spawn_fake_cast_worker, FakeCastTransport};
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string());
        let (job_tx, _calls) = spawn_fake_cast_worker(FakeCastTransport::default());
        app.set_cast_client("device-1", job_tx);
        let items = vec![
            feed_item("a", None),
            feed_item("b", Some("https://feed/b.mp3")),
        ];
        app.dispatch_selection_to_cast(items, 1);
        let event = app
            .cast_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        app.handle_cast_event(event);
        // No local playback command is ever reachable from this path: cast
        // dispatch loads the receiver's own queue instead.
        assert!(!app.player.status.lock().unwrap().active);
        assert_eq!(app.cast_attachment.as_ref().unwrap().dispatched.len(), 1);
        assert!(app.status.contains("Episode a") && app.status.contains("no media URL to cast"));
    }
}
