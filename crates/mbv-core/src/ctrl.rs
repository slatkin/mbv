use serde::{Deserialize, Serialize};

use crate::api::MediaItem;
use crate::config::QueueSource;
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

pub const CTRL_PROTOCOL_VERSION: u32 = 8;
pub const CTRL_CAP_QUEUE_STATE: &str = "queue-state";
pub const CTRL_CAP_START_INDEX: &str = "play-items-start-idx";
pub const CTRL_CAP_STATUS_ONLY: &str = "status-only";

pub type PlaybackRequestId = u64;
pub type PlaybackGeneration = u64;

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

    pub fn compatible_client(auth_token: String, compatibility: CtrlCompatibility) -> Self {
        let mut hello = Self::current_client(auth_token);
        hello.protocol_version = compatibility.client_protocol_version;
        hello
    }

    pub fn validate_peer(&self) -> Result<(), ConnectError> {
        self.compatibility()?;
        self.validate_required_capabilities()
            .map_err(ConnectError::Other)
    }

    pub fn compatibility(&self) -> Result<CtrlCompatibility, ConnectError> {
        CtrlCompatibility::for_peer(self.protocol_version)
    }

    fn validate_required_capabilities(&self) -> Result<(), String> {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CtrlCompatibility {
    pub peer_protocol_version: u32,
    pub client_protocol_version: u32,
    pub supports_queue_append: bool,
}

impl CtrlCompatibility {
    pub fn for_peer(peer_protocol_version: u32) -> Result<Self, ConnectError> {
        match peer_protocol_version {
            CTRL_PROTOCOL_VERSION => Ok(Self {
                peer_protocol_version,
                client_protocol_version: CTRL_PROTOCOL_VERSION,
                supports_queue_append: true,
            }),
            _ => Err(ConnectError::ProtocolMismatch {
                peer_version: peer_protocol_version,
                local_version: CTRL_PROTOCOL_VERSION,
            }),
        }
    }

    pub fn current() -> Self {
        Self::for_peer(CTRL_PROTOCOL_VERSION).expect("local ctrl protocol version is compatible")
    }
}

#[derive(Serialize, Deserialize)]
pub enum CtrlCmd {
    Hello(CtrlHello),
    PlayerCmd(WireCommand),
    AdoptQueue {
        items: Vec<MediaItem>,
        cursor: usize,
        source: QueueSource,
    },
    PlayItems {
        item_ids: Vec<String>,
        start_idx: usize,
        start_ticks: i64,
        source: QueueSource,
    },
    Stop,
    /// Correlated playback control. This is intentionally separate from
    /// `PlayerCmd` so guarded actions cannot silently fall back to the old,
    /// unacknowledged command path.
    PlaybackIntent(PlaybackIntent),
    /// Daemon lifecycle request: coordinated shutdown with durable queue
    /// persistence. Distinct from the player `Stop` command. Only accepted
    /// from local Unix ctrl connections (task 2.1).
    RequestShutdown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaybackIntent {
    pub request_id: PlaybackRequestId,
    pub generation: PlaybackGeneration,
    pub action: PlaybackIntentAction,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlaybackIntentAction {
    Play {
        item_ids: Vec<String>,
        start_idx: usize,
        start_ticks: i64,
        source: QueueSource,
    },
    Stop,
    SetPaused {
        paused: bool,
    },
    Next,
    Previous,
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
    #[serde(rename = "QueueAppend")]
    QueueAppend { items: Vec<MediaItem> },
    #[serde(rename = "PlaylistRemove")]
    QueueRemove(usize),
    #[serde(rename = "PlaylistMove")]
    QueueMove(usize, usize),
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
    ReplaceQueue {
        items: Vec<MediaItem>,
        start_idx: usize,
    },
}

impl From<PlayerCommand> for WireCommand {
    fn from(cmd: PlayerCommand) -> Self {
        match cmd {
            PlayerCommand::TogglePause => WireCommand::TogglePause,
            PlayerCommand::JumpTo(idx) => WireCommand::JumpTo(idx),
            PlayerCommand::QueueAppend { items } => WireCommand::QueueAppend { items },
            PlayerCommand::QueueRemove(idx) => WireCommand::QueueRemove(idx),
            PlayerCommand::QueueMove(from, to) => WireCommand::QueueMove(from, to),
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
            PlayerCommand::ReplaceQueue { items, start_idx } => {
                WireCommand::ReplaceQueue { items, start_idx }
            }
        }
    }
}

impl From<WireCommand> for PlayerCommand {
    fn from(cmd: WireCommand) -> Self {
        match cmd {
            WireCommand::TogglePause => PlayerCommand::TogglePause,
            WireCommand::JumpTo(idx) => PlayerCommand::JumpTo(idx),
            WireCommand::QueueAppend { items } => PlayerCommand::QueueAppend { items },
            WireCommand::QueueRemove(idx) => PlayerCommand::QueueRemove(idx),
            WireCommand::QueueMove(from, to) => PlayerCommand::QueueMove(from, to),
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
            WireCommand::ReplaceQueue { items, start_idx } => {
                PlayerCommand::ReplaceQueue { items, start_idx }
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
    #[serde(rename = "Disconnected")]
    Disconnected {
        reason: DisconnectReason,
    },
    /// A command the daemon received over the ctrl socket was not acted on;
    /// the payload is a human-readable, server-computed reason. Generic by
    /// design so future rejection reasons (not just audio-only mode) can
    /// reuse it — see #90.
    CommandRejected(String),
    PlaybackIntent(PlaybackIntentEvent),
    /// Observed pipe-startup progress for a direct daemon client. The daemon
    /// owns this status; it never represents downstream audibility.
    PipePlaybackStatus(PipePlaybackStatus),
    /// The daemon accepted a coordinated shutdown request after durably
    /// persisting its authoritative queue (task 2.2).
    ShutdownAccepted,
    /// The daemon rejected a coordinated shutdown request. The reason
    /// indicates why (e.g. TCP transport, persistence failure) (task 2.2).
    ShutdownRejected {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipePlaybackStatus {
    pub request_id: PlaybackRequestId,
    pub generation: PlaybackGeneration,
    pub phase: PipePlaybackPhase,
    /// Approximate time until the configured local estimate expires. Only
    /// present for `OutputBuffering`; it is not an observed downstream value.
    pub estimated_remaining_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipePlaybackPhase {
    Resolving,
    PlayerOpening,
    /// mbv observed mpv's `PlaybackRestart` event for this generation.
    OutputStarted,
    OutputBuffering,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackIntentEvent {
    pub request_id: PlaybackRequestId,
    pub generation: PlaybackGeneration,
    pub outcome: PlaybackIntentOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackIntentOutcome {
    Accepted,
    Applied,
    Coalesced {
        canonical_request_id: PlaybackRequestId,
    },
    Superseded,
    Rejected {
        reason: PlaybackIntentRejection,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackIntentRejection {
    EmptyTarget,
    InvalidTarget,
    ResolutionFailed,
    AudioOnly,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisconnectReason {
    #[serde(rename = "TakenOverByEmbyRemote")]
    TakenOverByEmbyRemote,
    /// The daemon is shutting down deliberately (`mbv -q`, tray Quit).
    /// Unlike `TakenOverByEmbyRemote`, this reason means the connection is
    /// about to close, and it is not an Emby-authority notification.
    #[serde(rename = "DaemonShutdown")]
    DaemonShutdown,
}

#[derive(Serialize, Deserialize)]
pub struct CtrlState {
    pub status: PlayerStatus,
    pub items: Vec<MediaItem>,
    pub cursor: usize,
    pub source: QueueSource,
}

/// Typed connection error distinguishing protocol mismatches from other
/// failures so callers can format recovery guidance without inspecting
/// formatted strings (task 2.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectError {
    /// The peer reports a protocol version incompatible with ours.
    ProtocolMismatch {
        peer_version: u32,
        local_version: u32,
    },
    /// Any other connection/handshake failure.
    Other(String),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProtocolMismatch {
                peer_version,
                local_version,
            } => write!(
                f,
                "incompatible daemon protocol version: peer={peer_version} local={local_version}"
            ),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl ConnectError {
    /// Formats a connection error with endpoint-specific guidance (task 7.4).
    /// For `DaemonEndpoint::Local` protocol mismatches, names `mbv -q` as the
    /// recovery action. For TCP/Unix endpoints, keeps the error generic and
    /// endpoint-specific (task 7.5).
    pub fn format_with_endpoint_guidance(
        &self,
        endpoint: &super::remote_player_connect::DaemonEndpoint,
    ) -> String {
        match self {
            Self::ProtocolMismatch {
                peer_version,
                local_version,
            } => {
                if matches!(
                    endpoint,
                    super::remote_player_connect::DaemonEndpoint::Local
                ) {
                    format!(
                        "incompatible daemon protocol version: peer={peer_version} local={local_version}. \
                         The local daemon may be an older version; run `mbv -q` to stop it, then restart."
                    )
                } else {
                    format!(
                        "incompatible daemon protocol version at {endpoint}: peer={peer_version} local={local_version}"
                    )
                }
            }
            Self::Other(msg) => msg.clone(),
        }
    }
}

impl std::error::Error for ConnectError {}

impl From<String> for ConnectError {
    fn from(msg: String) -> Self {
        Self::Other(msg)
    }
}

impl From<&str> for ConnectError {
    fn from(msg: &str) -> Self {
        Self::Other(msg.to_string())
    }
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
            source: QueueSource::Album,
        })
        .unwrap();

        let cmd: CtrlCmd = serde_json::from_str(&json).unwrap();
        match cmd {
            CtrlCmd::PlayItems {
                item_ids,
                start_idx,
                start_ticks,
                source,
            } => {
                assert_eq!(item_ids, vec!["a", "b"]);
                assert_eq!(start_idx, 1);
                assert_eq!(start_ticks, 42);
                assert!(matches!(source, QueueSource::Album));
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
    fn hello_rejects_v2_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 2;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_v3_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 3;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_previous_v4_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 4;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_previous_v5_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 5;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_v7_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version = 7;
        let result = hello.validate_peer();
        assert!(result.is_err());
        match result.unwrap_err() {
            ConnectError::ProtocolMismatch {
                peer_version,
                local_version,
            } => {
                assert_eq!(peer_version, 7);
                assert_eq!(local_version, CTRL_PROTOCOL_VERSION);
            }
            _ => panic!("expected ProtocolMismatch error"),
        }
    }

    #[test]
    fn request_shutdown_round_trips() {
        let cmd = CtrlCmd::RequestShutdown;
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, CtrlCmd::RequestShutdown));
    }

    #[test]
    fn shutdown_accepted_round_trips() {
        let event = CtrlEvent::ShutdownAccepted;
        let json = serde_json::to_string(&event).unwrap();
        let decoded: CtrlEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, CtrlEvent::ShutdownAccepted));
    }

    #[test]
    fn shutdown_rejected_round_trips() {
        let event = CtrlEvent::ShutdownRejected {
            reason: "test reason".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: CtrlEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            CtrlEvent::ShutdownRejected { reason } => {
                assert_eq!(reason, "test reason");
            }
            _ => panic!("expected ShutdownRejected"),
        }
    }

    #[test]
    fn connect_error_protocol_mismatch_display() {
        let err = ConnectError::ProtocolMismatch {
            peer_version: 7,
            local_version: 8,
        };
        let msg = err.to_string();
        assert!(msg.contains("peer=7"));
        assert!(msg.contains("local=8"));
    }

    #[test]
    fn connect_error_other_display() {
        let err = ConnectError::Other("test error".to_string());
        assert_eq!(err.to_string(), "test error");
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
            serde_json::to_string(&WireCommand::QueueAppend { items: vec![] }).unwrap(),
            "{\"QueueAppend\":{\"items\":[]}}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::QueueRemove(2)).unwrap(),
            "{\"PlaylistRemove\":2}"
        );
        assert_eq!(
            serde_json::to_string(&WireCommand::QueueMove(2, 3)).unwrap(),
            "{\"PlaylistMove\":[2,3]}"
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
            serde_json::to_string(&WireCommand::ReplaceQueue {
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

    #[test]
    fn old_stopped_player_event_defaults_progress_report_accepted() {
        let event: CtrlEvent = serde_json::from_str(
            r#"{"Player":{"Stopped":{"idx":0,"position_ticks":123,"played":false,"consume":false,"error":null}}}"#,
        )
        .unwrap();

        match event {
            CtrlEvent::Player(crate::player::PlayerEvent::Stopped {
                progress_report_accepted,
                ..
            }) => assert!(!progress_report_accepted),
            _ => panic!("expected stopped player event"),
        }
    }

    #[test]
    fn old_track_completed_player_event_defaults_progress_report_accepted() {
        let event: CtrlEvent = serde_json::from_str(
            r#"{"Player":{"TrackCompleted":{"idx":1,"position_ticks":456,"played":true,"consume":true}}}"#,
        )
        .unwrap();

        match event {
            CtrlEvent::Player(crate::player::PlayerEvent::TrackCompleted {
                progress_report_accepted,
                ..
            }) => assert!(!progress_report_accepted),
            _ => panic!("expected track completed player event"),
        }
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

    #[test]
    fn playback_intent_round_trips_as_a_distinct_command() {
        let command = CtrlCmd::PlaybackIntent(PlaybackIntent {
            request_id: 7,
            generation: 3,
            action: PlaybackIntentAction::SetPaused { paused: true },
        });

        let json = serde_json::to_string(&command).unwrap();
        let decoded: CtrlCmd = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            decoded,
            CtrlCmd::PlaybackIntent(PlaybackIntent {
                request_id: 7,
                generation: 3,
                action: PlaybackIntentAction::SetPaused { paused: true },
            })
        ));
    }

    #[test]
    fn playback_intent_event_round_trips_structured_rejection() {
        let event = CtrlEvent::PlaybackIntent(PlaybackIntentEvent {
            request_id: 7,
            generation: 3,
            outcome: PlaybackIntentOutcome::Rejected {
                reason: PlaybackIntentRejection::AudioOnly,
            },
        });

        let json = serde_json::to_string(&event).unwrap();
        let decoded: CtrlEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            decoded,
            CtrlEvent::PlaybackIntent(PlaybackIntentEvent {
                outcome: PlaybackIntentOutcome::Rejected {
                    reason: PlaybackIntentRejection::AudioOnly,
                },
                ..
            })
        ));
    }
}
