//! The `Metadata` property map of `org.mpris.MediaPlayer2.Player`.
//!
//! Non-test builds only: the map is built for a live D-Bus property read, and
//! the art lookup it uses (`resolve_art_url` in the parent module) is the
//! `file://` URI decision that also drives the polling loop's change
//! detection.

use std::collections::HashMap;

use mbv_ctrl::player::PlayerStatus;
use mbv_emby_model::TICKS_PER_SECOND;
use zbus::zvariant;

use super::resolve_art_url;

pub(super) fn make_metadata(
    s: &PlayerStatus,
    art_path: fn(&str) -> Option<std::path::PathBuf>,
) -> HashMap<String, zvariant::Value<'static>> {
    make_metadata_with_art_resolver(s, art_path)
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
