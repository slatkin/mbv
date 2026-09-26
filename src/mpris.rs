// In test builds the D-Bus-serving machinery below is deliberately dead:
// nothing may claim `org.mpris.MediaPlayer2.mbv` on the real session bus
// from a test process (issue #757), so `start` is only reachable from
// production. Production builds keep full dead-code checking.

use std::collections::HashMap;
#[cfg(not(test))]
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
#[cfg(not(test))]
use std::thread;
#[cfg(not(test))]
use std::time::Duration;

use zbus::zvariant;
#[cfg(not(test))]
use zbus::{connection, interface};

use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::{PlayerCommand, PlayerStatus};

#[cfg(not(test))]
struct MediaPlayer2;

#[cfg(not(test))]
#[interface(name = "org.mpris.MediaPlayer2")]
impl MediaPlayer2 {
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn quit(&self) {}
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn raise(&self) {}

    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn can_quit(&self) -> bool {
        false
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn can_raise(&self) -> bool {
        false
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn identity(&self) -> &'static str {
        "Emby Browser"
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec![]
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn supported_mime_types(&self) -> Vec<String> {
        vec![]
    }
}

/// The live status/command-sender/disconnect-flag triple MPRIS publishes.
///
/// Kept behind a handle (rather than baked directly into `MediaPlayer2Player`
/// and the polling loop's captured variables) so `rebind` (#175) can
/// re-point an already-registered MPRIS service at a different
/// `PlayerStatus` source without restarting the D-Bus connection: the App
/// swaps between a local `Player` and a `RemotePlayer` at runtime
/// (`switch_to_direct_remote` / `restore_local_mode`), and MPRIS must track
/// whichever one currently owns playback rather than staying wired to
/// whatever was live when `start` was first called.
pub(crate) struct MprisSource {
    status: Arc<Mutex<PlayerStatus>>,
    send: Arc<dyn Fn(PlayerCommand) + Send + Sync>,
    disconnected: Option<Arc<std::sync::atomic::AtomicBool>>,
}

/// Handle returned by `start`; pass it to `rebind` to re-point a live MPRIS
/// registration at a different playback source. Callers outside this module
/// (`main.rs`, `app.rs`) only ever move this opaque handle around and
/// pass it back into `rebind` -- they never touch `MprisSource`'s fields
/// directly, which stay module-private.
pub(crate) type MprisHandle = Arc<Mutex<MprisSource>>;

#[cfg(not(test))]
struct MediaPlayer2Player {
    /// The current status/command-sender/disconnect-flag triple, shared
    /// with `start`'s polling thread. Rebindable at runtime via `rebind`
    /// (#175) so the same live D-Bus registration can be re-pointed at a
    /// different `PlayerStatus` source (e.g. after `App::switch_to_direct_remote`
    /// swaps the app from a local `Player` to a `RemotePlayer`, or back)
    /// without tearing down and re-registering the MPRIS bus name.
    source: MprisHandle,
    /// Snapshot updated every 500ms by the polling loop so all property reads
    /// within one D-Bus call batch see consistent state.
    snapshot: Arc<Mutex<PlayerStatus>>,
}

/// Candidate on-disk image-cache keys for a track's cover art, in the order
/// the existing UI card-image cache (`src/app/images.rs` and
/// `src/app/render/card.rs`) is most likely to have already
/// populated them under -- checked cheaply via `std::path::Path::is_file`,
/// no network I/O. Covers every write site that keys on an album/item id:
/// the card (`:card`) and album-level card (`:album_card`). `album_id`
/// mirrors the audio-album grouping the queue card already uses: tracks on
/// the same album share one cache entry keyed by album id rather than
/// track id.
fn art_cache_key_candidates(item_id: &str, album_id: &str) -> Vec<String> {
    use crate::config::{IMAGE_CACHE_SUFFIX_ALBUM_CARD, IMAGE_CACHE_SUFFIX_CARD_PRIMARY};
    let mut keys = Vec::new();
    if !album_id.is_empty() {
        keys.push(format!("{album_id}:{IMAGE_CACHE_SUFFIX_CARD_PRIMARY}"));
        keys.push(format!("{album_id}:{IMAGE_CACHE_SUFFIX_ALBUM_CARD}"));
    }
    if !item_id.is_empty() {
        keys.push(format!("{item_id}:{IMAGE_CACHE_SUFFIX_CARD_PRIMARY}"));
    }
    keys
}

/// Resolve `mpris:artUrl` to a local `file://` URI for the current track's
/// cover art, or `None` when it isn't cached yet.
///
/// Per the #158 triage decision this must NEVER fall back to an Emby image
/// URL: that would embed the API token in a query string and leak it onto
/// the session D-Bus. An uncached track simply omits `mpris:artUrl`.
///
/// `resolve_path` is injected so this stays pure and unit-testable without
/// touching a real cache directory -- production code passes
/// `crate::config::image_disk_cache_path`.
fn resolve_art_url(
    item_id: &str,
    album_id: &str,
    resolve_path: impl Fn(&str) -> Option<std::path::PathBuf>,
) -> Option<String> {
    art_cache_key_candidates(item_id, album_id)
        .into_iter()
        .find_map(|key| resolve_path(&key))
        .map(|path| format!("file://{}", path.display()))
}

/// Forces `s` to look inactive (Stopped/NoTrack, no metadata) when
/// `disconnected` is true -- see `start`'s doc comment. Pure and cheap so
/// it's cloned/called every poll tick without hesitation; kept separate
/// from `start` so the "what should published state look like" decision is
/// unit-testable without a real D-Bus connection.
fn saturating_i64_from_f64(value: f64) -> i64 {
    const I64_MIN_AS_F64: f64 = -9_223_372_036_854_775_808.0;
    const I64_MAX_EXCLUSIVE_AS_F64: f64 = 9_223_372_036_854_775_808.0;

    if value.is_nan() {
        0
    } else if value >= I64_MAX_EXCLUSIVE_AS_F64 {
        i64::MAX
    } else if value <= I64_MIN_AS_F64 {
        i64::MIN
    } else {
        format!("{value:.0}")
            .parse()
            .expect("rounded bounded float fits i64")
    }
}

fn effective_status(mut s: PlayerStatus, disconnected: bool) -> PlayerStatus {
    if disconnected {
        s.active = false;
    }
    s
}

#[cfg(not(test))]
fn make_metadata(s: &PlayerStatus) -> HashMap<String, zvariant::Value<'static>> {
    make_metadata_with_art_resolver(s, crate::config::image_disk_cache_path)
}

fn make_metadata_with_art_resolver(
    s: &PlayerStatus,
    resolve_path: impl Fn(&str) -> Option<std::path::PathBuf>,
) -> HashMap<String, zvariant::Value<'static>> {
    let mut m = HashMap::new();
    let track_id = zvariant::ObjectPath::try_from(if s.active && !s.title.is_empty() {
        "/org/mpris/MediaPlayer2/TrackList/Track1"
    } else {
        "/org/mpris/MediaPlayer2/TrackList/NoTrack"
    })
    .unwrap();
    m.insert("mpris:trackid".to_string(), zvariant::Value::new(track_id));
    if s.active && !s.title.is_empty() {
        m.insert(
            "xesam:title".to_string(),
            zvariant::Value::new(s.title.clone()),
        );
        if s.runtime_ticks > 0 {
            let length_us = s.runtime_ticks * 1_000_000 / TICKS_PER_SECOND;
            m.insert("mpris:length".to_string(), zvariant::Value::new(length_us));
        }
        if let Some(art_url) = resolve_art_url(&s.art_item_id, &s.art_album_id, resolve_path) {
            m.insert("mpris:artUrl".to_string(), zvariant::Value::new(art_url));
        }
        if !s.artist.is_empty() {
            m.insert(
                "xesam:artist".to_string(),
                zvariant::Value::new(vec![s.artist.clone()]),
            );
        }
        if !s.album.is_empty() {
            m.insert(
                "xesam:album".to_string(),
                zvariant::Value::new(s.album.clone()),
            );
        }
    }
    m
}

#[cfg(not(test))]
type StatusAndSender = (
    Arc<Mutex<PlayerStatus>>,
    Arc<dyn Fn(PlayerCommand) + Send + Sync>,
);

#[cfg(not(test))]
impl MediaPlayer2Player {
    /// Clones the current `status`/`send` pair out from behind `self.source`'s
    /// lock, dropping that lock immediately -- so callers below never hold
    /// both `self.source`'s lock and `status`'s lock at once, and always act
    /// on whatever `rebind` (#175) most recently set rather than something
    /// captured once at registration time.
    ///
    /// Deliberately kept in a plain (non-`#[interface]`) impl block: zbus's
    /// `#[interface]` macro treats every method in its block as an exposed
    /// D-Bus method/property, and this helper's tuple return type has no
    /// D-Bus marshaling impl.
    fn status_and_sender(&self) -> StatusAndSender {
        let source = self.source.lock().unwrap();
        (Arc::clone(&source.status), Arc::clone(&source.send))
    }
}

#[cfg(not(test))]
#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MediaPlayer2Player {
    fn play(&self) {
        let (status, send) = self.status_and_sender();
        if let Some(cmd) = status.lock().unwrap().toggle_to_reach(false) {
            send(cmd);
        };
    }

    fn pause(&self) {
        let (status, send) = self.status_and_sender();
        if let Some(cmd) = status.lock().unwrap().toggle_to_reach(true) {
            send(cmd);
        };
    }

    fn play_pause(&self) {
        (self.status_and_sender().1)(PlayerCommand::TogglePause);
    }

    fn stop(&self) {
        (self.status_and_sender().1)(PlayerCommand::TogglePause);
    }

    fn next(&self) {
        (self.status_and_sender().1)(PlayerCommand::Next);
    }

    fn previous(&self) {
        (self.status_and_sender().1)(PlayerCommand::Previous);
    }

    fn seek(&self, offset_us: i64) {
        #[expect(
            clippy::cast_precision_loss,
            reason = "seconds↔ticks conversion through f64; no lossless integer-path conversion exists (approved, issue #804)"
        )]
        let secs = offset_us as f64 / 1_000_000.0;
        // Clamp seek to reasonable bounds (avoid seeking hours into the future).
        if secs.abs() > 86400.0 {
            return;
        }
        (self.source.lock().unwrap().send)(PlayerCommand::Seek(secs));
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "zbus interface method signature is fixed by the macro dispatch (approved, issue #804)"
    )]
    fn set_position(&self, track_id: zvariant::ObjectPath<'_>, position_us: i64) {
        // Per MPRIS spec: ignore if track_id doesn't match current track or position is negative.
        if track_id.as_str() != "/org/mpris/MediaPlayer2/TrackList/Track1" {
            return;
        }
        if position_us < 0 {
            return;
        }
        let source = self.source.lock().unwrap();
        let runtime_us = source.status.lock().unwrap().runtime_ticks * 1_000_000 / TICKS_PER_SECOND;
        if runtime_us > 0 && position_us > runtime_us {
            return;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "seconds↔ticks conversion through f64; no lossless integer-path conversion exists (approved, issue #804)"
        )]
        (source.send)(PlayerCommand::SeekAbsolute(
            position_us as f64 / 1_000_000.0,
        ));
    }

    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn open_uri(&self, uri: &str) {
        let _ = uri;
    }

    #[zbus(property)]
    fn playback_status(&self) -> String {
        let s = self.snapshot.lock().unwrap();
        if !s.active {
            "Stopped".into()
        } else if s.paused {
            "Paused".into()
        } else {
            "Playing".into()
        }
    }

    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn loop_status(&self) -> &'static str {
        "None"
    }

    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn shuffle(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, zvariant::Value<'static>> {
        make_metadata(&self.snapshot.lock().unwrap())
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "seconds↔ticks conversion through f64; no lossless integer-path conversion exists (approved, issue #804)"
        )]
        let volume = self.snapshot.lock().unwrap().volume as f64 / 100.0;
        volume
    }

    #[zbus(property)]
    fn set_volume(&self, vol: f64) {
        (self.source.lock().unwrap().send)(PlayerCommand::SetVolume(saturating_i64_from_f64(
            (vol * 100.0).round(),
        )));
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        self.snapshot.lock().unwrap().position_ticks * 1_000_000 / TICKS_PER_SECOND
    }

    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn minimum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn maximum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn can_go_next(&self) -> bool {
        true
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn can_go_previous(&self) -> bool {
        true
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn can_play(&self) -> bool {
        true
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn can_pause(&self) -> bool {
        true
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn can_seek(&self) -> bool {
        true
    }
    #[zbus(property)]
    #[expect(
        clippy::unused_self,
        reason = "zbus interface macro mandates the &self receiver; the method carries no per-instance state (approved, issue #804)"
    )]
    fn can_control(&self) -> bool {
        true
    }
}

#[cfg(not(test))]
async fn poll_status(
    conn: zbus::Connection,
    source_poll: MprisHandle,
    snapshot_poll: Arc<Mutex<PlayerStatus>>,
) {
    let mut last_status = String::new();
    let mut last_metadata_key = (String::new(), String::new(), String::new(), String::new());
    let mut last_pos_seconds: i64 = -1;
    let mut last_vol: i64 = -1;

    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        // Re-read the source every tick so a rebind takes effect immediately.
        let (status_arc, is_disconnected) = {
            let src = source_poll.lock().unwrap();
            let is_disconnected = src
                .disconnected
                .as_ref()
                .is_some_and(|d| d.load(Ordering::SeqCst));
            (Arc::clone(&src.status), is_disconnected)
        };

        let (cur_status, cur_metadata_key, cur_pos_us, cur_vol) = {
            let raw = status_arc.lock().unwrap().clone();
            let s = effective_status(raw, is_disconnected);
            let st = match (s.active, s.paused) {
                (false, _) => "Stopped",
                (true, true) => "Paused",
                (true, false) => "Playing",
            }
            .to_string();
            let pos_us = s.position_ticks * 1_000_000 / TICKS_PER_SECOND;
            // Include the resolved cache path so artwork appearing mid-track
            // triggers a metadata change instead of remaining absent.
            let art_key = resolve_art_url(
                &s.art_item_id,
                &s.art_album_id,
                crate::config::image_disk_cache_path,
            )
            .unwrap_or_default();
            let result = (
                st,
                (s.title.clone(), s.artist.clone(), s.album.clone(), art_key),
                pos_us,
                s.volume,
            );
            *snapshot_poll.lock().unwrap() = s;
            result
        };

        let Ok(iface_ref) = conn
            .object_server()
            .interface::<_, MediaPlayer2Player>("/org/mpris/MediaPlayer2")
            .await
        else {
            continue;
        };
        let ctxt = iface_ref.signal_context();
        let iface = iface_ref.get().await;

        if cur_status != last_status {
            last_status = cur_status;
            let _ = iface.playback_status_changed(ctxt).await;
        }
        if cur_metadata_key != last_metadata_key {
            last_metadata_key = cur_metadata_key;
            let _ = iface.metadata_changed(ctxt).await;
        }
        let current_pos_seconds = cur_pos_us / 1_000_000;
        if (current_pos_seconds - last_pos_seconds).abs() >= 5 {
            last_pos_seconds = current_pos_seconds;
            let _ = iface.position_changed(ctxt).await;
        }
        if cur_vol != last_vol {
            last_vol = cur_vol;
            let _ = iface.volume_changed(ctxt).await;
        }
    }
}

/// Starts the MPRIS D-Bus service against `status`, forwarding player
/// commands via `send`.
///
/// `disconnected` (#160) is the daemon-connection drop signal for the
/// remote-client case (`RemotePlayer::disconnected_flag()`); pass `None`
/// for the local, non-daemon player, which has no such connection to lose.
/// When set and tripped, published state is forced to `Stopped`/`NoTrack`
/// regardless of what's still cached in `status` -- see `effective_status`.
/// This is a defense-in-depth net: `RemotePlayer::connect_endpoint` also
/// clears `status` directly at the point it detects an "expected" (silent)
/// disconnect, but polling can race that update, so this flag is checked
/// independently on every tick.
///
/// Returns a handle that `rebind` (#175) can later use to re-point this
/// same live registration at a different `status`/`send`/`disconnected`
/// triple -- needed because `App::switch_to_direct_remote` /
/// `restore_local_mode` swap which `Player`/`RemotePlayer` owns playback
/// at runtime, and MPRIS must follow whichever one is current rather than
/// staying wired to whatever was live when `start` was first called.
#[cfg(not(test))]
pub fn start(
    status: Arc<Mutex<PlayerStatus>>,
    send: impl Fn(PlayerCommand) + Send + Sync + 'static,
    disconnected: Option<Arc<std::sync::atomic::AtomicBool>>,
) -> MprisHandle {
    let snapshot = Arc::new(Mutex::new(status.lock().unwrap().clone()));
    let source: MprisHandle = Arc::new(Mutex::new(MprisSource {
        status,
        send: Arc::new(send),
        disconnected,
    }));
    let source_poll = Arc::clone(&source);
    let snapshot_poll = Arc::clone(&snapshot);

    thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("MPRIS tokio error: {e}");
                return;
            }
        };
        rt.block_on(async move {
            let player_iface = MediaPlayer2Player {
                source: Arc::clone(&source_poll),
                snapshot: Arc::clone(&snapshot_poll),
            };
            let conn = match connection::Builder::session()
                .unwrap()
                .name("org.mpris.MediaPlayer2.mbv")
                .unwrap()
                .serve_at("/org/mpris/MediaPlayer2", MediaPlayer2)
                .unwrap()
                .serve_at("/org/mpris/MediaPlayer2", player_iface)
                .unwrap()
                .build()
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("MPRIS D-Bus error: {e}");
                    return;
                }
            };

            poll_status(conn, source_poll, snapshot_poll).await;
        });
    });

    source
}

/// Re-points an already-registered MPRIS service (from `start`) at a
/// different `status`/`send`/`disconnected` triple, without restarting the
/// D-Bus connection or re-claiming the bus name.
///
/// #175: `App::switch_to_direct_remote` and `restore_local_mode` swap which
/// `Player`/`RemotePlayer` currently owns playback; before this existed,
/// MPRIS stayed wired to whatever was live when `start` was first called
/// (almost always the initial local `Player`), so local desktop MPRIS never
/// picked up a remote daemon's playback after a mid-session takeover.
pub fn rebind(
    handle: &MprisHandle,
    status: Arc<Mutex<PlayerStatus>>,
    send: impl Fn(PlayerCommand) + Send + Sync + 'static,
    disconnected: Option<Arc<std::sync::atomic::AtomicBool>>,
) {
    let mut source = handle.lock().unwrap();
    source.status = status;
    source.send = Arc::new(send);
    source.disconnected = disconnected;
}

/// Test-only constructor/inspector pair for `MprisHandle`, used by
/// `src/app.rs`'s tests to inject a lightweight (no real D-Bus/tokio)
/// handle into `App.mpris` and assert `switch_to_direct_remote` /
/// `restore_local_mode` actually call `rebind` on it (#175), without
/// duplicating `MprisSource`'s private fields outside this module.
#[cfg(test)]
pub(crate) fn test_handle(
    status: Arc<Mutex<PlayerStatus>>,
    send: impl Fn(PlayerCommand) + Send + Sync + 'static,
    disconnected: Option<Arc<std::sync::atomic::AtomicBool>>,
) -> MprisHandle {
    Arc::new(Mutex::new(MprisSource {
        status,
        send: Arc::new(send),
        disconnected,
    }))
}

#[cfg(test)]
pub(crate) fn test_status(handle: &MprisHandle) -> Arc<Mutex<PlayerStatus>> {
    Arc::clone(&handle.lock().unwrap().status)
}

#[cfg(test)]
#[path = "mpris_tests.rs"]
mod tests;
