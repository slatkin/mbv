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

struct MediaPlayer2Player {
    status: Arc<Mutex<PlayerStatus>>,
    /// Snapshot updated every 500ms by the polling loop so all property reads
    /// within one D-Bus call batch see consistent state.
    snapshot: Arc<Mutex<PlayerStatus>>,
    cmd_tx: Arc<dyn Fn(PlayerCommand) + Send + Sync>,
}

/// Candidate on-disk image-cache keys for a track's cover art, in the order
/// the existing UI card-image cache (`src/app/images.rs`) is most likely to
/// have already populated them under -- checked cheaply via
/// `std::path::Path::is_file`, no network I/O. `album_id` mirrors the
/// audio-album grouping the Power View queue card
/// (`src/app/render/power/card.rs`) already uses: tracks on the same album
/// share one cache entry keyed by album id rather than track id.
fn art_cache_key_candidates(item_id: &str, album_id: &str) -> Vec<String> {
    let mut keys = Vec::new();
    if !album_id.is_empty() {
        keys.push(format!("{album_id}:P"));
    }
    if !item_id.is_empty() {
        keys.push(format!("{item_id}:P"));
        keys.push(format!("{item_id}:lib"));
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

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MediaPlayer2Player {
    fn play(&self) {
        if let Some(cmd) = self.status.lock().unwrap().toggle_to_reach(false) {
            (self.cmd_tx)(cmd);
        }
    }

    fn pause(&self) {
        if let Some(cmd) = self.status.lock().unwrap().toggle_to_reach(true) {
            (self.cmd_tx)(cmd);
        }
    }

    fn play_pause(&self) {
        (self.cmd_tx)(PlayerCommand::TogglePause);
    }

    fn stop(&self) {
        (self.cmd_tx)(PlayerCommand::TogglePause);
    }

    fn next(&self) {
        if let Some(idx) = self.status.lock().unwrap().next_idx() {
            (self.cmd_tx)(PlayerCommand::JumpTo(idx));
        }
    }

    fn previous(&self) {
        if let Some(idx) = self.status.lock().unwrap().previous_idx() {
            (self.cmd_tx)(PlayerCommand::JumpTo(idx));
        }
    }

    fn seek(&self, offset_us: i64) {
        let secs = offset_us as f64 / 1_000_000.0;
        // Clamp seek to reasonable bounds (avoid seeking hours into the future).
        if secs.abs() > 86400.0 {
            return;
        }
        (self.cmd_tx)(PlayerCommand::Seek(secs));
    }

    fn set_position(&self, track_id: zvariant::ObjectPath<'_>, position_us: i64) {
        // Per MPRIS spec: ignore if track_id doesn't match current track or position is negative.
        if track_id.as_str() != "/org/mpris/MediaPlayer2/TrackList/Track1" {
            return;
        }
        if position_us < 0 {
            return;
        }
        let runtime_us = self.status.lock().unwrap().runtime_ticks * 1_000_000 / TICKS_PER_SECOND;
        if runtime_us > 0 && position_us > runtime_us {
            return;
        }
        (self.cmd_tx)(PlayerCommand::SeekAbsolute(
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
        (self.cmd_tx)(PlayerCommand::SetVolume((vol * 100.0).round() as i64));
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
pub fn start(
    status: Arc<Mutex<PlayerStatus>>,
    send: impl Fn(PlayerCommand) + Send + Sync + 'static,
    disconnected: Option<Arc<std::sync::atomic::AtomicBool>>,
) {
    let send = Arc::new(send);
    let status_poll = status.clone();
    let snapshot = Arc::new(Mutex::new(status.lock().unwrap().clone()));
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
                status,
                snapshot: snapshot_poll.clone(),
                cmd_tx: send,
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

                // Snapshot PlayerStatus once per poll cycle so all property
                // reads see consistent state. Force Stopped/NoTrack once the
                // daemon connection has dropped (#160) -- `status` itself
                // isn't guaranteed to reflect that promptly (an "expected"
                // disconnect, e.g. an Emby Remote takeover, never sends a
                // Stopped PlayerEvent; see RemotePlayer::connect_endpoint).
                let is_disconnected = disconnected
                    .as_ref()
                    .is_some_and(|d| d.load(Ordering::SeqCst));
                let (cur_status, cur_metadata_key, cur_pos_us, cur_vol) = {
                    let raw = status_poll.lock().unwrap().clone();
                    let s = effective_status(raw, is_disconnected);
                    *snapshot_poll.lock().unwrap() = s.clone();
                    let st = if !s.active {
                        "Stopped".to_string()
                    } else if s.paused {
                        "Paused".to_string()
                    } else {
                        "Playing".to_string()
                    };
                    let pos_us = s.position_ticks * 1_000_000 / TICKS_PER_SECOND;
                    (
                        st,
                        (
                            s.title.clone(),
                            s.artist.clone(),
                            s.album.clone(),
                            s.art_item_id.clone(),
                        ),
                        pos_us,
                        s.volume,
                    )
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
            vec!["album-9:P", "track-1:P", "track-1:lib"]
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
}
