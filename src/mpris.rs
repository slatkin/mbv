use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zbus::{connection, interface, zvariant};

use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::{PlayerCommand, PlayerStatus};

struct MediaPlayer2;

#[interface(name = "org.mpris.MediaPlayer2")]
impl MediaPlayer2 {
    fn quit(&self) {}
    fn raise(&self) {}

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Emby Browser"
    }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec![]
    }
    #[zbus(property)]
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
/// (`main.rs`, `app/mod.rs`) only ever move this opaque handle around and
/// pass it back into `rebind` -- they never touch `MprisSource`'s fields
/// directly, which stay module-private.
pub(crate) type MprisHandle = Arc<Mutex<MprisSource>>;

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
/// `src/app/render/{power,library}/*`) is most likely to have already
/// populated them under -- checked cheaply via `std::path::Path::is_file`,
/// no network I/O. Covers every write site that keys on an album/item id:
/// Power View's card (`:P`) and album-level card (`:pwr_al`), and the
/// Library view's row/grid/album cache (`:lib`). `album_id` mirrors the
/// audio-album grouping the Power View queue card already uses: tracks on
/// the same album share one cache entry keyed by album id rather than
/// track id.
fn art_cache_key_candidates(item_id: &str, album_id: &str) -> Vec<String> {
    use crate::config::{
        IMAGE_CACHE_SUFFIX_LIBRARY, IMAGE_CACHE_SUFFIX_POWER_ALBUM,
        IMAGE_CACHE_SUFFIX_POWER_PRIMARY,
    };
    let mut keys = Vec::new();
    if !album_id.is_empty() {
        keys.push(format!("{album_id}:{IMAGE_CACHE_SUFFIX_POWER_PRIMARY}"));
        keys.push(format!("{album_id}:{IMAGE_CACHE_SUFFIX_LIBRARY}"));
        keys.push(format!("{album_id}:{IMAGE_CACHE_SUFFIX_POWER_ALBUM}"));
    }
    if !item_id.is_empty() {
        keys.push(format!("{item_id}:{IMAGE_CACHE_SUFFIX_POWER_PRIMARY}"));
        keys.push(format!("{item_id}:{IMAGE_CACHE_SUFFIX_LIBRARY}"));
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
fn effective_status(mut s: PlayerStatus, disconnected: bool) -> PlayerStatus {
    if disconnected {
        s.active = false;
    }
    s
}

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
    fn status_and_sender(
        &self,
    ) -> (
        Arc<Mutex<PlayerStatus>>,
        Arc<dyn Fn(PlayerCommand) + Send + Sync>,
    ) {
        let source = self.source.lock().unwrap();
        (source.status.clone(), source.send.clone())
    }
}

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
        let (status, send) = self.status_and_sender();
        if let Some(idx) = status.lock().unwrap().next_idx() {
            send(PlayerCommand::JumpTo(idx));
        };
    }

    fn previous(&self) {
        let (status, send) = self.status_and_sender();
        if let Some(idx) = status.lock().unwrap().previous_idx() {
            send(PlayerCommand::JumpTo(idx));
        };
    }

    fn seek(&self, offset_us: i64) {
        let secs = offset_us as f64 / 1_000_000.0;
        // Clamp seek to reasonable bounds (avoid seeking hours into the future).
        if secs.abs() > 86400.0 {
            return;
        }
        (self.source.lock().unwrap().send)(PlayerCommand::Seek(secs));
    }

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
        (source.send)(PlayerCommand::SeekAbsolute(
            position_us as f64 / 1_000_000.0,
        ));
    }

    fn open_uri(&self, _uri: &str) {}

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
    fn loop_status(&self) -> &str {
        "None"
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn shuffle(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, zvariant::Value<'static>> {
        make_metadata(&self.snapshot.lock().unwrap())
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.snapshot.lock().unwrap().volume as f64 / 100.0
    }

    #[zbus(property)]
    async fn set_volume(&self, vol: f64) {
        (self.source.lock().unwrap().send)(PlayerCommand::SetVolume((vol * 100.0).round() as i64));
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        self.snapshot.lock().unwrap().position_ticks * 1_000_000 / TICKS_PER_SECOND
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
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
    let source_poll = source.clone();
    let snapshot_poll = snapshot.clone();

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
                source: source_poll.clone(),
                snapshot: snapshot_poll.clone(),
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

            let mut last_status = String::new();
            let mut last_metadata_key =
                (String::new(), String::new(), String::new(), String::new());
            let mut last_pos_s: i64 = -1;
            let mut last_vol: i64 = -1;

            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;

                // Re-read the current status/disconnect-flag pair from
                // `source_poll` on every tick (rather than closing over a
                // fixed `Arc` once, before the loop) so a `rebind` call
                // takes effect on the very next poll, not just for a
                // freshly-registered D-Bus service (#175).
                let (status_arc, is_disconnected) = {
                    let src = source_poll.lock().unwrap();
                    let is_disconnected = src
                        .disconnected
                        .as_ref()
                        .is_some_and(|d| d.load(Ordering::SeqCst));
                    (src.status.clone(), is_disconnected)
                };

                let (cur_status, cur_metadata_key, cur_pos_us, cur_vol) = {
                    // Single clone out of the mutex for the whole tick: `s`
                    // is consumed to build the tuple below and then moved
                    // (not cloned again) into `snapshot_poll` last.
                    let raw = status_arc.lock().unwrap().clone();
                    let s = effective_status(raw, is_disconnected);
                    let st = if !s.active {
                        "Stopped".to_string()
                    } else if s.paused {
                        "Paused".to_string()
                    } else {
                        "Playing".to_string()
                    };
                    let pos_us = s.position_ticks * 1_000_000 / TICKS_PER_SECOND;
                    // Resolve the actual cached-art result (not just the raw
                    // item/album id) into the change-detection key: if art
                    // shows up in the cache after this track already started
                    // publishing metadata (e.g. a Power View browse populates
                    // the cache mid-track), the resolved value flips from ""
                    // to a real path and `metadata_changed` fires, instead of
                    // silently staying stuck on the id-only key that never
                    // changes for the rest of the track.
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

                // Emit position every ~5s.
                let cur_pos_s = cur_pos_us / 1_000_000;
                if (cur_pos_s - last_pos_s).abs() >= 5 {
                    last_pos_s = cur_pos_s;
                    let _ = iface.position_changed(ctxt).await;
                }

                if cur_vol != last_vol {
                    last_vol = cur_vol;
                    let _ = iface.volume_changed(ctxt).await;
                }
            }
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
/// `src/app/mod.rs`'s tests to inject a lightweight (no real D-Bus/tokio)
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
    handle.lock().unwrap().status.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string_value(metadata: &HashMap<String, zvariant::Value<'static>>, key: &str) -> String {
        metadata
            .get(key)
            .unwrap_or_else(|| panic!("missing metadata key {key}"))
            .downcast_ref::<String>()
            .unwrap_or_else(|_| panic!("metadata key {key} is not a string"))
    }

    #[test]
    fn active_metadata_includes_cover_art_as_file_uri_when_cached() {
        // #158 regression coverage: mpris:artUrl must be a local file://
        // URI, never an Emby URL (which would carry the API token onto the
        // session bus).
        let metadata = make_metadata_with_art_resolver(
            &PlayerStatus {
                active: true,
                title: "Song".to_string(),
                artist: "Artist".to_string(),
                album: "Album".to_string(),
                art_item_id: "track-1".to_string(),
                ..PlayerStatus::default()
            },
            |key| {
                assert_eq!(key, "track-1:P", "expected the track-id cache key first");
                Some(std::path::PathBuf::from("/cache/images/track-1_P"))
            },
        );

        assert_eq!(
            string_value(&metadata, "mpris:artUrl"),
            "file:///cache/images/track-1_P"
        );
        assert!(metadata.contains_key("xesam:artist"));
        assert_eq!(string_value(&metadata, "xesam:album"), "Album");
    }

    #[test]
    fn active_metadata_prefers_album_cache_key_for_grouped_audio_tracks() {
        let metadata = make_metadata_with_art_resolver(
            &PlayerStatus {
                active: true,
                title: "Song".to_string(),
                art_item_id: "track-1".to_string(),
                art_album_id: "album-9".to_string(),
                ..PlayerStatus::default()
            },
            |key| (key == "album-9:P").then(|| std::path::PathBuf::from("/cache/images/album-9_P")),
        );

        assert_eq!(
            string_value(&metadata, "mpris:artUrl"),
            "file:///cache/images/album-9_P"
        );
    }

    #[test]
    fn active_metadata_omits_art_url_when_not_cached() {
        // Per the #158 triage decision: when cached art isn't available,
        // omit mpris:artUrl entirely rather than falling back to a
        // token-bearing Emby URL.
        let metadata = make_metadata_with_art_resolver(
            &PlayerStatus {
                active: true,
                title: "Song".to_string(),
                art_item_id: "track-1".to_string(),
                ..PlayerStatus::default()
            },
            |_key| None,
        );

        assert!(!metadata.contains_key("mpris:artUrl"));
    }

    #[test]
    fn inactive_metadata_omits_track_details_and_never_touches_the_cache() {
        let metadata = make_metadata_with_art_resolver(
            &PlayerStatus {
                artist: "Artist".to_string(),
                album: "Album".to_string(),
                art_item_id: "track-1".to_string(),
                ..PlayerStatus::default()
            },
            |_key| panic!("art cache should never be consulted for an inactive/no-track state"),
        );

        assert!(!metadata.contains_key("mpris:artUrl"));
        assert!(!metadata.contains_key("xesam:artist"));
        assert!(!metadata.contains_key("xesam:album"));
    }

    #[test]
    fn art_cache_key_candidates_prefers_album_then_track_id() {
        assert_eq!(
            art_cache_key_candidates("track-1", "album-9"),
            vec![
                "album-9:P",
                "album-9:lib",
                "album-9:pwr_al",
                "track-1:P",
                "track-1:lib"
            ]
        );
        assert_eq!(
            art_cache_key_candidates("track-1", ""),
            vec!["track-1:P", "track-1:lib"]
        );
        assert!(art_cache_key_candidates("", "").is_empty());
    }

    #[test]
    fn effective_status_forces_inactive_when_disconnected() {
        // #160: once the daemon connection drops, published MPRIS state
        // must go to Stopped/NoTrack even if `status` still has stale
        // "still playing" data in it.
        let playing = PlayerStatus {
            active: true,
            title: "Song".to_string(),
            art_item_id: "track-1".to_string(),
            ..PlayerStatus::default()
        };

        let untouched = effective_status(playing.clone(), false);
        assert!(untouched.active);

        let forced = effective_status(playing, true);
        assert!(!forced.active);
        // make_metadata should now take the inactive/NoTrack branch.
        let metadata = make_metadata_with_art_resolver(&forced, |_| {
            panic!("art cache should never be consulted once disconnected")
        });
        assert!(!metadata.contains_key("xesam:title"));
        assert!(!metadata.contains_key("mpris:artUrl"));
    }

    #[test]
    fn rebind_repoints_a_handle_at_a_new_status_and_sender() {
        // #175: `App::switch_to_direct_remote` / `restore_local_mode` call
        // `rebind` to re-point an already-registered MPRIS service at
        // whichever `Player`/`RemotePlayer` now owns playback. This test
        // exercises `rebind` directly (no real D-Bus/tokio involved) --
        // it's the smallest reproduction of the propagation break: before
        // `rebind` existed, nothing updated the `Arc<Mutex<PlayerStatus>>`
        // MPRIS's polling loop was watching after such a swap.
        let status_a = Arc::new(Mutex::new(PlayerStatus {
            active: false,
            ..PlayerStatus::default()
        }));
        let status_b = Arc::new(Mutex::new(PlayerStatus {
            active: true,
            title: "Remote Song".to_string(),
            ..PlayerStatus::default()
        }));
        let sent = Arc::new(Mutex::new(Vec::<PlayerCommand>::new()));

        let handle: MprisHandle = Arc::new(Mutex::new(MprisSource {
            status: status_a.clone(),
            send: Arc::new(|_: PlayerCommand| {}),
            disconnected: None,
        }));

        let sent_for_rebind = sent.clone();
        let disconnected_b = Arc::new(std::sync::atomic::AtomicBool::new(false));
        rebind(
            &handle,
            status_b.clone(),
            move |cmd| sent_for_rebind.lock().unwrap().push(cmd),
            Some(disconnected_b.clone()),
        );

        let source = handle.lock().unwrap();
        assert!(
            Arc::ptr_eq(&source.status, &status_b),
            "rebind must repoint the handle's status at the new source, not stay on the old one"
        );
        assert!(!Arc::ptr_eq(&source.status, &status_a));
        assert!(source
            .disconnected
            .as_ref()
            .is_some_and(|d| Arc::ptr_eq(d, &disconnected_b)));
        (source.send)(PlayerCommand::TogglePause);
        drop(source);
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert!(matches!(sent[0], PlayerCommand::TogglePause));
    }
}
