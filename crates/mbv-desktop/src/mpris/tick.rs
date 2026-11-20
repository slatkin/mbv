//! One polling tick: the values a tick publishes, the cache that keeps a tick
//! from re-signalling an unchanged property, and the property signals
//! themselves.
//!
//! Everything here runs inside the publisher loop of the parent module, in
//! non-test builds only: nothing may reach a live session bus from a test
//! process (issue #757).

use mbv_ctrl::player::PlayerStatus;
use mbv_emby_model::TICKS_PER_SECOND;

use super::{MediaPlayer2Player, MprisHandle, resolve_art_url};

/// Forces `s` to look inactive (Stopped/NoTrack, no metadata) when
/// `disconnected` is true -- see `start`'s doc comment. Pure and cheap so
/// it's cloned/called every poll tick without hesitation; kept independent of
/// the D-Bus connection.
pub(super) fn effective_status(mut s: PlayerStatus, disconnected: bool) -> PlayerStatus {
    if disconnected {
        s.active = false;
    }
    s
}

/// One polling tick's published values, read from the live source.
pub(super) struct TickView {
    /// Whether the source's publish rule holds (design D3).
    pub(super) publish: bool,
    status_name: &'static str,
    metadata_key: (String, String, String, String),
    position_seconds: i64,
    volume: i64,
}

/// Reads the live source once and derives the values this tick publishes.
pub(super) fn read_tick(
    source: &MprisHandle,
    art_path: fn(&str) -> Option<std::path::PathBuf>,
) -> (PlayerStatus, TickView) {
    let (disconnected, publish, raw) = {
        let source = source.lock().unwrap();
        (
            source.disconnected.as_ref().is_some_and(|signal| signal()),
            (source.publish)(),
            (source.status)(),
        )
    };
    let status = effective_status(raw, disconnected);
    let status_name = match (status.active, status.paused) {
        (false, _) => "Stopped",
        (true, true) => "Paused",
        (true, false) => "Playing",
    };
    let position_us = status.position_ticks * 1_000_000 / TICKS_PER_SECOND;
    // Include the resolved cache path so artwork appearing mid-track triggers
    // a metadata change instead of remaining absent.
    let art_key =
        resolve_art_url(&status.art_item_id, &status.art_album_id, art_path).unwrap_or_default();
    let tick = TickView {
        publish,
        status_name,
        metadata_key: (
            status.title.clone(),
            status.artist.clone(),
            status.album.clone(),
            art_key,
        ),
        position_seconds: position_us / 1_000_000,
        volume: status.volume,
    };
    (status, tick)
}

/// The values the last published tick signalled, so a tick signals only
/// changes. An empty cache differs from every tick.
pub(super) struct SignalCache {
    status: String,
    metadata_key: (String, String, String, String),
    position_seconds: i64,
    volume: i64,
}

impl SignalCache {
    pub(super) fn empty() -> Self {
        Self {
            status: String::new(),
            metadata_key: (String::new(), String::new(), String::new(), String::new()),
            position_seconds: -1,
            volume: -1,
        }
    }
}

/// Signals whatever property changed since the last published tick.
pub(super) async fn emit_changes(
    conn: &zbus::Connection,
    tick: &TickView,
    cache: &mut SignalCache,
) {
    let Ok(iface_ref) = conn
        .object_server()
        .interface::<_, MediaPlayer2Player>("/org/mpris/MediaPlayer2")
        .await
    else {
        return;
    };
    let ctxt = iface_ref.signal_emitter();
    let iface = iface_ref.get().await;

    if cache.status != tick.status_name {
        cache.status = tick.status_name.to_string();
        let _ = iface.playback_status_changed(ctxt).await;
    }
    if cache.metadata_key != tick.metadata_key {
        cache.metadata_key = tick.metadata_key.clone();
        let _ = iface.metadata_changed(ctxt).await;
    }
    if (tick.position_seconds - cache.position_seconds).abs() >= 5 {
        cache.position_seconds = tick.position_seconds;
        let _ = iface.position_changed(ctxt).await;
    }
    if cache.volume != tick.volume {
        cache.volume = tick.volume;
        let _ = iface.volume_changed(ctxt).await;
    }
}
