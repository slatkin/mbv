//! The Owner's Cast source resolver (cast-owner-output design D6): turn a
//! queue item into the media a receiver loads plus the progress target its
//! provider reporting needs, and run that resolve off the event-loop thread
//! (design D4, task 4.2). The resolver is separate from the local mpv path's
//! `prepare_source` because the two share almost nothing: the Cast path puts
//! the credential in the URL, takes the server-chosen rendition, and leaves
//! the resolved session's lifecycle to the output that loaded it.

use std::fmt;

use mbv_audiobookshelf::AudiobookshelfClient;
use mbv_cast::CastError;
use mbv_cast::client::CastMediaItem;
use mbv_cast::dispatch::{self, CastSubtitleKind, build_cast_device_profile};
use mbv_ctrl::PlaybackGeneration;
use mbv_emby::EmbyClient;
use mbv_emby_model::EmbyItem;
use mbv_ids::{EmbySessionId, MediaSourceId};
use mbv_queue::{AudiobookshelfItem, AudiobookshelfQueueItem, FeedEntry, QueueItem, QueueSlotId};

use super::DaemonEvent;
use super::DaemonLoop;
use super::cast::{CastLoadedItem, PlaybackOutput};
use crate::ctrl::send_to;
use mbv_ctrl::CtrlEvent;

/// Where a cast item's progress is reported (design D6/D9), one variant per
/// provider the resolver resolves items for. Moved here from the TUI's
/// `state/types/cast.rs`; the TUI imports it from this crate until the row 8
/// deletion of its own Cast runtime.
#[derive(Debug, Clone)]
pub enum CastProgressTarget {
    Emby {
        item_id: String,
        media_source_id: MediaSourceId,
        session_id: EmbySessionId,
        runtime_ticks: i64,
    },
    Feed {
        feed_id: Option<String>,
        guid: String,
    },
    /// `session_id` is the Audiobookshelf playback session opened to resolve
    /// this episode's cast URL (`create_playback_session_bounded`) -- it must
    /// be opened at resolve time regardless of progress reporting, since
    /// `cast::dispatch::resolve_audiobookshelf_episode_dispatch` requires an
    /// already-resolved `AudiobookshelfAudioSource`. The session stays open:
    /// the output that loads the item owns its lifecycle (observe, sync, and
    /// close on advance, stop, or output change -- design D9, task 6.4); the
    /// resolve never closes it.
    AudiobookshelfEpisode {
        session_id: String,
        duration_seconds: f64,
    },
}

/// One item's resolved Cast source (design D6): the media the receiver is
/// told to load and the progress target the item's provider reporting needs.
#[derive(Debug)]
pub struct CastSource {
    pub media: CastMediaItem,
    pub progress: CastProgressTarget,
}

/// Why an item's Cast source could not be resolved (design D6): the item's
/// display name and the reason, ready to surface to the user.
#[derive(Debug, Clone)]
pub struct Uncastable {
    pub item: String,
    pub reason: String,
}

impl Uncastable {
    fn for_item(item: &str, error: &CastError) -> Self {
        Self {
            item: item.to_string(),
            reason: error.to_string(),
        }
    }
}

/// What one Audiobookshelf episode's resolve needs: enough to open a bounded
/// playback session and resolve its castable source. Moved from the TUI's
/// `dispatch/cast.rs`, mirroring `AudiobookshelfPlayerContext`
/// (`player_sources.rs`) without depending on that excluded file.
pub struct AbsCastContext {
    pub client: AudiobookshelfClient,
    pub credential: String,
    pub device_id: String,
}

impl fmt::Debug for AbsCastContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AbsCastContext")
            .field("credential", &"<redacted>")
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

/// The service contexts one resolve runs against (design D6). A missing
/// service context leaves that provider's arm unavailable; resolving an item
/// of that kind then reports the service as unconnected.
#[derive(Default)]
pub struct CastResolveContext {
    pub emby: Option<EmbyClient>,
    pub abs: Option<AbsCastContext>,
}

impl fmt::Debug for CastResolveContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CastResolveContext")
            .field("emby", &self.emby.is_some())
            .field("abs", &self.abs)
            .finish()
    }
}

/// Resolves `item`'s Cast source (design D6): Emby through a cast device
/// profile and `PlaybackInfo`, a feed entry through its enclosure URL, an
/// Audiobookshelf episode through a bounded session open that stays open for
/// progress reporting, and an Audiobookshelf book as uncastable outright.
/// Network-touching: run it through [`spawn_cast_resolve`], never on the
/// event-loop thread.
pub fn prepare_cast_source(
    item: &QueueItem,
    ctx: &CastResolveContext,
) -> Result<CastSource, Uncastable> {
    let name = item.display_name();
    match item {
        QueueItem::Emby(emby_item) => resolve_emby_cast_item(emby_item, ctx)
            .map_err(|error| Uncastable::for_item(&name, &error)),
        QueueItem::Feed(entry) => {
            resolve_feed_cast_item(entry).map_err(|error| Uncastable::for_item(&name, &error))
        }
        QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(episode)) => {
            resolve_abs_episode_cast_item(episode, ctx)
                .map_err(|error| Uncastable::for_item(&name, &error))
        }
        QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book)) => Err(Uncastable::for_item(
            &name,
            &CastError::audiobookshelf_book(&book.title),
        )),
    }
}

fn resolve_feed_cast_item(entry: &FeedEntry) -> Result<CastSource, CastError> {
    let media = dispatch::resolve_feed_dispatch(entry)?;
    Ok(CastSource {
        media,
        progress: CastProgressTarget::Feed {
            feed_id: entry.feed_id.clone(),
            guid: entry.guid.clone(),
        },
    })
}

fn resolve_emby_cast_item(
    item: &EmbyItem,
    ctx: &CastResolveContext,
) -> Result<CastSource, CastError> {
    let Some(client) = ctx.emby.as_ref() else {
        return Err(CastError::emby_unavailable(&item.name));
    };
    let profile = build_cast_device_profile(CastSubtitleKind::None);
    let info = client
        .get_playback_info_for_cast(&item.id, item.is_audio(), &profile)
        .map_err(CastError::emby_playback_info)?;
    Ok(CastSource {
        media: info.item,
        progress: CastProgressTarget::Emby {
            item_id: item.id.clone(),
            media_source_id: info.media_source_id,
            session_id: info.session_id,
            runtime_ticks: item.runtime_ticks,
        },
    })
}

fn resolve_abs_episode_cast_item(
    episode: &AudiobookshelfQueueItem,
    ctx: &CastResolveContext,
) -> Result<CastSource, CastError> {
    let Some(abs) = ctx.abs.as_ref() else {
        return Err(CastError::audiobookshelf_unavailable(&episode.title));
    };
    let session = abs
        .client
        .create_playback_session_bounded(
            &abs.credential,
            &abs.device_id,
            &episode.library_item_id,
            &episode.episode_id,
            false,
            AudiobookshelfClient::REQUEST_HARD_BOUND,
        )
        .map_err(|e| CastError::audiobookshelf_session(&episode.title, e))?;
    let media =
        dispatch::resolve_audiobookshelf_episode_dispatch(&session.source, &abs.credential)?;
    Ok(CastSource {
        media,
        progress: CastProgressTarget::AudiobookshelfEpisode {
            session_id: session.id,
            duration_seconds: session.duration_seconds,
        },
    })
}

/// The Audiobookshelf context one bounded-session call needs, built from the
/// owner runtime: the runtime's client and the Service secret. Shared by the
/// resolve (`owner_resolve_context`) and the loaded item's progress reporting
/// (`build_progress_reporting`'s Audiobookshelf arm), so both drive the same
/// server with the same credential.
pub(crate) fn abs_cast_context(
    runtime: &super::AudiobookshelfOwnerContext,
) -> Option<AbsCastContext> {
    let credential = mbv_config::load_service_secret(mbv_queue::ServiceKind::Audiobookshelf)?;
    let client = AudiobookshelfClient::new(&runtime.setup.server_url).ok()?;
    Some(AbsCastContext {
        client,
        credential,
        device_id: runtime.device_id.clone(),
    })
}

/// The resolve context a dispatch on the event-loop thread builds (design
/// D4/D6): the owner's Emby client, and — when the Audiobookshelf runtime is
/// connected — the bounded-session context an episode resolve needs. Both
/// constructions are local (no network): the resolve thread does the network
/// work.
pub(crate) fn owner_resolve_context(
    client: &std::sync::Arc<std::sync::Mutex<EmbyClient>>,
    audiobookshelf: Option<&super::AudiobookshelfOwnerContext>,
) -> CastResolveContext {
    CastResolveContext {
        emby: Some(client.lock().unwrap().clone()),
        abs: audiobookshelf.and_then(abs_cast_context),
    }
}

/// Spawns one Cast source resolve on a dedicated thread (design D4, task
/// 4.2) and reports the outcome through `send` exactly once, whatever it is.
/// The caller pairs the spawn with a `PendingCastResolve` record and sends
/// the outcome on as a `CastOwnerEvent::Resolved` keyed by the slot and the
/// transition generation it started the resolve for; the owner drops an
/// outcome that no longer matches that record (task 4.2), so a superseded
/// resolve loads nothing.
pub fn spawn_cast_resolve(
    item: QueueItem,
    ctx: CastResolveContext,
    send: impl FnOnce(Result<CastSource, Uncastable>) + Send + 'static,
) {
    std::thread::spawn(move || {
        send(prepare_cast_source(&item, &ctx));
    });
}

/// One Cast source resolve outcome (design D4, task 4.2). Matched against the
/// owner's pending-resolve record by slot and transition generation: a result
/// whose pair no longer matches is a stale result for a superseded target and
/// is dropped without a load.
pub(super) fn handle_cast_resolved(
    daemon: &mut DaemonLoop,
    slot: QueueSlotId,
    generation: PlaybackGeneration,
    result: Result<CastSource, Uncastable>,
) {
    let matched = daemon
        .owner
        .pending_cast_resolve
        .as_ref()
        .is_some_and(|pending| {
            pending.transition.target == slot && pending.transition.generation == generation
        });
    if !matched {
        tracing::info!(
            name: "daemon.cast_resolve.stale",
            target: "daemon",
            slot = ?slot,
            generation,
            "stale cast resolve result dropped"
        );
        return;
    }
    let Some(pending) = daemon.owner.pending_cast_resolve.take() else {
        return;
    };
    let source = match result {
        Ok(source) => source,
        Err(uncastable) => {
            resolve_failed(daemon, &pending, &uncastable);
            return;
        }
    };
    // The transition the resolve belonged to must still be in flight: one
    // that expired in its bounded window (design D5, row 5.2) is no longer
    // guarded, so a load now would never settle and would leave the receiver
    // silently on the old slot.
    if daemon
        .owner
        .core
        .transitions
        .in_flight()
        .is_none_or(|t| t != pending.transition)
    {
        tracing::info!(
            name: "daemon.cast_resolve.expired",
            target: "transition",
            slot = ?slot,
            generation,
            "the resolve's transition expired; the resolved source is dropped"
        );
        return;
    }
    load_resolved_source(daemon, slot, pending.start_ticks, source);
}

/// One hand-off resolve outcome (design D10, row 7.1): the captured slot's
/// Cast source for a successful hand-off to a receiver (from Local or from
/// another receiver). Matched against the
/// output's pending hand-off load by slot: a result whose slot no longer
/// matches — or that lands after the output was replaced or stopped, which
/// dropped the record with it — is stale and loads nothing. On success the
/// resolved media loads at the captured position and a captured pause is
/// applied after the load; an uncastable slot is reported to the selecting
/// Client and the output stays idle.
pub(super) fn handle_handoff_resolved(
    daemon: &mut DaemonLoop,
    slot: QueueSlotId,
    result: Result<CastSource, Uncastable>,
) {
    let pending = {
        let crate::cast::PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
            return;
        };
        cast.pending_handoff_load
            .take_if(|pending| pending.slot == slot)
    };
    let Some(pending) = pending else {
        tracing::info!(
            name: "daemon.cast_handoff.stale",
            target: "daemon",
            slot = ?slot,
            "stale hand-off resolve result dropped"
        );
        return;
    };
    match result {
        Ok(source) => {
            load_resolved_source(daemon, slot, pending.start_ticks, source);
            // The captured pause applies after the load (design D10 step 5):
            // both jobs queue on the worker in order, so the receiver pauses
            // the item it just took.
            if pending.paused {
                crate::cast::set_receiver_paused(&mut daemon.owner.output, true);
            }
        }
        Err(uncastable) => {
            tracing::warn!(
                name: "daemon.cast_handoff.uncastable",
                target: "daemon",
                slot = ?slot,
                item = %uncastable.item,
                reason = %uncastable.reason,
                "the handed-off slot's item cannot be cast; the output stays idle"
            );
            send_to(
                &pending.reply_tx,
                &CtrlEvent::CommandRejected(format!("{} ({})", uncastable.item, uncastable.reason)),
            );
        }
    }
}

/// Loads the resolved source on the receiver (design D4): the item the output
/// still presents reports its stop before the new load's session can report
/// its start (design D9; an already-ended item's reporting is gone, so that
/// is a no-op there), then the load job is submitted and the loaded record
/// installed, ending any advance the step belonged to (design D7).
fn load_resolved_source(
    daemon: &mut DaemonLoop,
    slot: QueueSlotId,
    start_ticks: i64,
    source: CastSource,
) {
    crate::cast::progress::stop_loaded_reporting(daemon);
    let reporting = crate::cast::progress::build_progress_reporting(
        daemon,
        slot,
        &source.progress,
        start_ticks,
    );
    let PlaybackOutput::Cast(cast) = &mut daemon.owner.output else {
        return;
    };
    let start_seconds = mbv_emby_model::ticks_to_seconds(start_ticks);
    tracing::info!(
        name: "daemon.cast_output.load",
        target: "daemon",
        slot = ?slot,
        url = %source.media.url,
        start_seconds,
        "loading the resolved cast source on the receiver"
    );
    let content_id = source.media.url.clone();
    submit_load_job(
        &cast.worker,
        source.media,
        start_seconds,
        slot,
        &daemon.merged_tx,
    );
    cast.loaded = Some(CastLoadedItem {
        slot,
        content_id,
        progress: source.progress,
        reporting,
        start_reported: false,
        end_handled: false,
    });
    cast.advance = None;
    // The new load's first presenting status settles its own arrival, not a
    // seek the replaced item took (design D9).
    cast.pending_seek = None;
}

/// A failed resolve expires the transition it belonged to (design D5, row
/// 5.2) and names the item and reason to the client that issued the play or
/// jump (row 5.1); the receiver is given nothing new.
fn resolve_failed(
    daemon: &mut DaemonLoop,
    pending: &crate::cast::PendingCastResolve,
    uncastable: &Uncastable,
) {
    let slot = pending.transition.target;
    tracing::warn!(
        name: "daemon.cast_resolve.uncastable",
        target: "daemon",
        slot = ?slot,
        item = %uncastable.item,
        reason = %uncastable.reason,
        "the slot's item cannot be cast; nothing was loaded"
    );
    let report = format!("{} ({})", uncastable.item, uncastable.reason);
    if crate::output::advance::step_failed(daemon, pending.transition.request_id) {
        // The failed step was the advance's own (design D7): it has no
        // issuing client, so the report goes to every Client (spec "Advance
        // reaches an uncastable slot"), and the advance steps over unless the
        // expiry promoted a queued user transition — that user took the queue
        // over.
        crate::core::broadcast(
            &daemon.ctrl_clients,
            &mbv_ctrl::CtrlEvent::CommandRejected(report),
        );
        let mut ctx = crate::cast::owner_context(daemon);
        if crate::cast::expire_cast_transition(&mut ctx) {
            crate::output::advance::clear_advance(daemon);
        } else {
            crate::output::advance::continue_after_failed_step(daemon, pending.transition.target);
        }
    } else {
        let mut ctx = crate::cast::owner_context(daemon);
        crate::cast::report_resolve_failure(
            &mut ctx,
            &pending.transition,
            pending.origin,
            uncastable,
        );
        crate::cast::expire_cast_transition(&mut ctx);
    }
}

/// Submits one load job (design D4): a fresh pong immediately before the
/// blocking exchange -- the load's reply waits for the receiver to load the
/// item, and the heartbeat deadline must survive that wait (the same ordering
/// the TUI's dispatch used). A load the receiver refuses reports back as
/// `CastOwnerEvent::LoadFailed` for `slot`, which expires the transition the
/// load belonged to (design D5).
fn submit_load_job(
    worker: &std::sync::mpsc::Sender<mbv_cast::CastJob>,
    media: CastMediaItem,
    start_seconds: f64,
    slot: QueueSlotId,
    merged_tx: &std::sync::mpsc::Sender<DaemonEvent>,
) {
    let merged_tx = merged_tx.clone();
    let _ = worker.send(Box::new(move |transport: &mut dyn mbv_cast::CastTransport| {
        if let Err(error) = transport.keep_alive() {
            tracing::warn!(name: "cast.keep_alive.failed", target: "cast", error = %error, "cast keep-alive failed");
        }
        if let Err(error) = transport.load(&media, start_seconds) {
            let _ = merged_tx.send(DaemonEvent::Cast(crate::cast::CastOwnerEvent::LoadFailed {
                slot,
                error: error.to_string(),
            }));
        }
    }));
}
