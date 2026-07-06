use serde::{Deserialize, Serialize};

use crate::api::MediaItem;
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

pub const CTRL_PROTOCOL_VERSION: u32 = 1;
pub const CTRL_CAP_QUEUE_STATE: &str = "queue-state";
pub const CTRL_CAP_START_INDEX: &str = "play-items-start-idx";
pub const CTRL_CAP_STATUS_ONLY: &str = "status-only";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CtrlHello {
    pub protocol_version: u32,
    pub app_version: String,
    pub capabilities: Vec<String>,
    pub auth_token: Option<String>,
}

impl CtrlHello {
    pub fn current() -> Self {
        Self {
            protocol_version: CTRL_PROTOCOL_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec![
                CTRL_CAP_QUEUE_STATE.to_string(),
                CTRL_CAP_START_INDEX.to_string(),
                CTRL_CAP_STATUS_ONLY.to_string(),
            ],
            auth_token: None,
        }
    }

    pub fn current_client(auth_token: String) -> Self {
        let mut hello = Self::current();
        hello.auth_token = Some(auth_token);
        hello
    }

    pub fn validate_peer(&self) -> Result<(), String> {
        if self.protocol_version != CTRL_PROTOCOL_VERSION {
            return Err(format!(
                "incompatible daemon protocol version: peer={} local={}",
                self.protocol_version, CTRL_PROTOCOL_VERSION
            ));
        }
        for required in [
            CTRL_CAP_QUEUE_STATE,
            CTRL_CAP_START_INDEX,
            CTRL_CAP_STATUS_ONLY,
        ] {
            if !self.capabilities.iter().any(|cap| cap == required) {
                return Err(format!(
                    "peer missing daemon protocol capability: {required}"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub enum CtrlCmd {
    Hello(CtrlHello),
    PlayerCmd(WireCommand),
    PlayItems {
        item_ids: Vec<String>,
        start_idx: usize,
        start_ticks: i64,
    },
    Stop,
}

/// Wire-stable representation of a `PlayerCommand`, serialized across the
/// daemon/TUI process seam. Kept as a distinct type (rather than serializing
/// `PlayerCommand` directly) so that renaming or restructuring in-process
/// player commands cannot silently change the wire protocol: every variant
/// here has an explicit, pinned `serde(rename)` tag, and the conversions
/// to/from `PlayerCommand` are exhaustive matches with no wildcard arm, so
/// adding a new `PlayerCommand` variant is a compile error until this type
/// (and its conversions) are updated too.
#[derive(Serialize, Deserialize)]
pub enum WireCommand {
    #[serde(rename = "TogglePause")]
    TogglePause,
    #[serde(rename = "JumpTo")]
    JumpTo(usize),
    #[serde(rename = "PlaylistRemove")]
    PlaylistRemove(usize),
    #[serde(rename = "SetVolume")]
    SetVolume(i64),
    #[serde(rename = "Seek")]
    Seek(f64),
    #[serde(rename = "SeekAbsolute")]
    SeekAbsolute(f64),
    #[serde(rename = "SetAudio")]
    SetAudio(i64),
    #[serde(rename = "SetSub")]
    SetSub(i64),
    #[serde(rename = "SetSubtitlePrefs")]
    SetSubtitlePrefs {
        mode: String,
        subtitle_lang: String,
        audio_lang: String,
    },
    #[serde(rename = "SetMute")]
    SetMute(bool),
    #[serde(rename = "LoadNew")]
    LoadNew {
        url: String,
        start_pos: f64,
        item: Box<MediaItem>,
    },
    #[serde(rename = "NextUpShow")]
    NextUpShow {
        item_id: String,
        show_title: String,
        ep_title: String,
        artist: String,
    },
    #[serde(rename = "NextUpDismiss")]
    NextUpDismiss,
    #[serde(rename = "SkipIntroDismiss")]
    SkipIntroDismiss,
    #[serde(rename = "ReplacePlaylist")]
    ReplacePlaylist {
        items: Vec<MediaItem>,
        start_idx: usize,
    },
}

impl From<PlayerCommand> for WireCommand {
    fn from(cmd: PlayerCommand) -> Self {
        match cmd {
            PlayerCommand::TogglePause => WireCommand::TogglePause,
            PlayerCommand::JumpTo(idx) => WireCommand::JumpTo(idx),
            PlayerCommand::PlaylistRemove(idx) => WireCommand::PlaylistRemove(idx),
            PlayerCommand::SetVolume(v) => WireCommand::SetVolume(v),
            PlayerCommand::Seek(s) => WireCommand::Seek(s),
            PlayerCommand::SeekAbsolute(s) => WireCommand::SeekAbsolute(s),
            PlayerCommand::SetAudio(i) => WireCommand::SetAudio(i),
            PlayerCommand::SetSub(i) => WireCommand::SetSub(i),
            PlayerCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            } => WireCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            },
            PlayerCommand::SetMute(m) => WireCommand::SetMute(m),
            PlayerCommand::LoadNew {
                url,
                start_pos,
                item,
            } => WireCommand::LoadNew {
                url,
                start_pos,
                item,
            },
            PlayerCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            } => WireCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            },
            PlayerCommand::NextUpDismiss => WireCommand::NextUpDismiss,
            PlayerCommand::SkipIntroDismiss => WireCommand::SkipIntroDismiss,
            PlayerCommand::ReplacePlaylist { items, start_idx } => {
                WireCommand::ReplacePlaylist { items, start_idx }
            }
        }
    }
}

impl From<WireCommand> for PlayerCommand {
    fn from(cmd: WireCommand) -> Self {
        match cmd {
            WireCommand::TogglePause => PlayerCommand::TogglePause,
            WireCommand::JumpTo(idx) => PlayerCommand::JumpTo(idx),
            WireCommand::PlaylistRemove(idx) => PlayerCommand::PlaylistRemove(idx),
            WireCommand::SetVolume(v) => PlayerCommand::SetVolume(v),
            WireCommand::Seek(s) => PlayerCommand::Seek(s),
            WireCommand::SeekAbsolute(s) => PlayerCommand::SeekAbsolute(s),
            WireCommand::SetAudio(i) => PlayerCommand::SetAudio(i),
            WireCommand::SetSub(i) => PlayerCommand::SetSub(i),
            WireCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            } => PlayerCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            },
            WireCommand::SetMute(m) => PlayerCommand::SetMute(m),
            WireCommand::LoadNew {
                url,
                start_pos,
                item,
            } => PlayerCommand::LoadNew {
                url,
                start_pos,
                item,
            },
            WireCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            } => PlayerCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            },
            WireCommand::NextUpDismiss => PlayerCommand::NextUpDismiss,
            WireCommand::SkipIntroDismiss => PlayerCommand::SkipIntroDismiss,
            WireCommand::ReplacePlaylist { items, start_idx } => {
                PlayerCommand::ReplacePlaylist { items, start_idx }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum CtrlEvent {
    Hello(CtrlHello),
    Player(PlayerEvent),
    State(CtrlState),
    StatusOnly(PlayerStatus),
    /// A command the daemon received over the ctrl socket was not acted on;
    /// the payload is a human-readable, server-computed reason. Generic by
    /// design so future rejection reasons (not just audio-only mode) can
    /// reuse it — see #90.
    CommandRejected(String),
}

#[derive(Serialize, Deserialize)]
pub struct CtrlState {
    pub status: PlayerStatus,
    pub items: Vec<MediaItem>,
    pub cursor: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_items_command_preserves_start_index() {
        let json = serde_json::to_string(&CtrlCmd::PlayItems {
            item_ids: vec!["a".to_string(), "b".to_string()],
            start_idx: 1,
            start_ticks: 42,
        })
        .unwrap();

        let cmd: CtrlCmd = serde_json::from_str(&json).unwrap();
        match cmd {
            CtrlCmd::PlayItems {
                item_ids,
                start_idx,
                start_ticks,
            } => {
                assert_eq!(item_ids, vec!["a", "b"]);
                assert_eq!(start_idx, 1);
                assert_eq!(start_ticks, 42);
            }
            _ => panic!("expected PlayItems"),
        }
    }

    #[test]
    fn current_hello_validates() {
        CtrlHello::current().validate_peer().unwrap();
    }

    #[test]
    fn hello_rejects_incompatible_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version += 1;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_missing_capability() {
        let mut hello = CtrlHello::current();
        hello.capabilities.retain(|cap| cap != CTRL_CAP_START_INDEX);
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn current_client_hello_carries_auth_token() {
        let hello = CtrlHello::current_client("token-123".into());
        assert_eq!(hello.auth_token.as_deref(), Some("token-123"));
    }

    // The wire tags below are pinned via `#[serde(rename = "...")]` on
    // `WireCommand` and must not change without a deliberate, explicit
    // decision -- they are independent of whatever `PlayerCommand`'s Rust
    // variant identifiers happen to be at any given time. If one of these
    // assertions fails, the wire protocol just changed; that may be fine,
    // but it should never happen as a side effect of an in-process rename.
    #[test]
    fn wire_command_tags_are_pinned() {
        assert_eq!(
            serde_json::to_string(&WireCommand::TogglePause).unwrap(),
            "\"TogglePause\""
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::JumpTo(3)).unwrap(),
            "{\"JumpTo\":3}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::PlaylistRemove(2)).unwrap(),
            "{\"PlaylistRemove\":2}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::SetVolume(50)).unwrap(),
            "{\"SetVolume\":50}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::Seek(1.5)).unwrap(),
            "{\"Seek\":1.5}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::SeekAbsolute(2.5)).unwrap(),
            "{\"SeekAbsolute\":2.5}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::SetAudio(1)).unwrap(),
            "{\"SetAudio\":1}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::SetSub(0)).unwrap(),
            "{\"SetSub\":0}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::SetMute(true)).unwrap(),
            "{\"SetMute\":true}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::NextUpDismiss).unwrap(),
            "\"NextUpDismiss\""
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::SkipIntroDismiss).unwrap(),
            "\"SkipIntroDismiss\""
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::SetSubtitlePrefs {
                mode: "auto".to_string(),
                subtitle_lang: "eng".to_string(),
                audio_lang: "jpn".to_string(),
            })
            .unwrap(),
            "{\"SetSubtitlePrefs\":{\"mode\":\"auto\",\"subtitle_lang\":\"eng\",\"audio_lang\":\"jpn\"}}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::ReplacePlaylist {
                items: vec![],
                start_idx: 0,
            })
            .unwrap(),
            "{\"ReplacePlaylist\":{\"items\":[],\"start_idx\":0}}"
        );
        // LoadNew and NextUpShow carry a MediaItem / free-form strings, so
        // asserting the full JSON body would just restate MediaItem's field
        // list; instead check the pinned tag key only.
        assert_eq!(
            wire_tag(&WireCommand::LoadNew {
                url: "http://emby.local/stream".into(),
                start_pos: 0.0,
                item: Box::new(stub_media_item()),
            }),
            "LoadNew"
        );
        assert_eq!(
            wire_tag(&WireCommand::NextUpShow {
                item_id: "item1".into(),
                show_title: "Show".into(),
                ep_title: "Ep".into(),
                artist: String::new(),
            }),
            "NextUpShow"
        );
    }

    /// Returns the top-level (externally-tagged) JSON key for a serialized
    /// `WireCommand`, i.e. the pinned wire tag.
    fn wire_tag(cmd: &WireCommand) -> String {
        let json = serde_json::to_string(cmd).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value
            .as_object()
            .and_then(|obj| obj.keys().next())
            .unwrap_or_else(|| panic!("expected a tagged object, got {json}"))
            .clone()
    }

    fn stub_media_item() -> crate::api::MediaItem {
        crate::api::MediaItem {
            id: "item1".into(),
            name: "Test Item".into(),
            item_type: "Episode".into(),
            is_folder: false,
            media_type: "Video".into(),
            collection_type: String::new(),
            runtime_ticks: 0,
            played: false,
            playback_position_ticks: 0,
            series_id: String::new(),
            series_name: String::new(),
            album_id: String::new(),
            album: String::new(),
            index_number: 0,
            parent_index_number: 0,
            unplayed_item_count: 0,
            path: String::new(),
            artist: String::new(),
            sort_name: String::new(),
            production_year: 0,
            end_year: 0,
            overview: String::new(),
            premiere_date: String::new(),
            date_added: String::new(),
            total_count: 0,
            container: String::new(),
            director: String::new(),
            video_info: String::new(),
            audio_info: String::new(),
            genre: String::new(),
            playlist_item_id: String::new(),
        }
    }

    #[test]
    fn wire_command_round_trips_through_json() {
        let json = serde_json::to_string(&WireCommand::SetVolume(77)).unwrap();
        let decoded: WireCommand = serde_json::from_str(&json).unwrap();
        match PlayerCommand::from(decoded) {
            PlayerCommand::SetVolume(v) => assert_eq!(v, 77),
            _ => panic!("expected SetVolume"),
        }
    }

    #[test]
    fn player_command_round_trips_through_wire_command() {
        let wire: WireCommand = PlayerCommand::SeekAbsolute(12.5).into();
        let json = serde_json::to_string(&wire).unwrap();
        let decoded: WireCommand = serde_json::from_str(&json).unwrap();
        match PlayerCommand::from(decoded) {
            PlayerCommand::SeekAbsolute(s) => assert_eq!(s, 12.5),
            _ => panic!("expected SeekAbsolute"),
        }
    }

    #[test]
    fn ctrl_cmd_player_cmd_round_trips_through_json() {
        let json = serde_json::to_string(&CtrlCmd::PlayerCmd(PlayerCommand::SetMute(true).into()))
            .unwrap();
        let cmd: CtrlCmd = serde_json::from_str(&json).unwrap();
        match cmd {
            CtrlCmd::PlayerCmd(wire) => match PlayerCommand::from(wire) {
                PlayerCommand::SetMute(m) => assert!(m),
                _ => panic!("expected SetMute"),
            },
            _ => panic!("expected PlayerCmd"),
        }
    }
}
