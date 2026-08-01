#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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
    // ── v8 slot-aware queue commands ──────────────────────────────────────
    /// Remove the item identified by `slot_id`. The client's last-known
    /// `revision` must match the daemon's current revision.
    #[serde(rename = "QueueRemoveBySlot")]
    QueueRemoveBySlot {
        slot_id: QueueSlotId,
        revision: QueueRevision,
    },
    /// Move the slot `slot_id` to the positional index `to_position`.
    #[serde(rename = "QueueMoveBySlot")]
    QueueMoveBySlot {
        slot_id: QueueSlotId,
        to_position: usize,
        revision: QueueRevision,
    },
    /// Jump playback to the slot identified by `slot_id`.
    #[serde(rename = "JumpToSlot")]
    JumpToSlot {
        slot_id: QueueSlotId,
    },
    /// Insert `item` at the given `position` for undo restoration. The
    /// daemon assigns a new slot ID and broadcasts it in the next full
    /// state snapshot.
    #[serde(rename = "QueueInsertAt")]
    QueueInsertAt {
        item: MediaItem,
        position: usize,
        revision: QueueRevision,
    },
    /// Transactionally remove the active slot and advance the active
    /// marker, then stop playback.
    #[serde(rename = "QueueRemoveActive")]
    QueueRemoveActive {
        revision: QueueRevision,
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
            // v8 slot-aware commands have no `PlayerCommand` equivalent.
            // The daemon control layer handles them directly on `WireCommand`
            // (see `daemon_control.rs` task 4.x) instead of converting first.
            WireCommand::QueueRemoveBySlot { .. } => {
                unreachable!("v8 QueueRemoveBySlot handled directly by daemon control layer")
            }
            WireCommand::QueueMoveBySlot { .. } => {
                unreachable!("v8 QueueMoveBySlot handled directly by daemon control layer")
            }
            WireCommand::JumpToSlot { .. } => {
                unreachable!("v8 JumpToSlot handled directly by daemon control layer")
            }
            WireCommand::QueueInsertAt { .. } => {
                unreachable!("v8 QueueInsertAt handled directly by daemon control layer")
            }
            WireCommand::QueueRemoveActive { .. } => {
                unreachable!("v8 QueueRemoveActive handled directly by daemon control layer")
            }
        }
    }
}
