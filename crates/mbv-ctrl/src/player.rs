use mbv_emby_model::EmbyItem;
use mbv_ids::ItemId;
use mbv_queue::ExecSlot;
use mbv_queue::{AudiobookshelfItem, QueueItem, QueueSlotId};

#[derive(Clone, Debug, Default)]
pub struct SubtitlePrefs {
    pub mode: String, // "Default"|"Always"|"Smart"|"OnlyForced"|"None"|"HearingImpaired"
    pub subtitle_lang: String, // full language name, e.g. "English"
    pub audio_lang: String, // full language name, e.g. "English"
}

/// How the subtitle preference resolves against the available tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubtitleChoice {
    /// Leave the current selection unchanged.
    Leave,
    /// No subtitles (`sid` = "no").
    Off,
    /// Select a specific track id.
    Track(i64),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "status booleans are independent observed player properties (design analysis, issue #804)"
)]
pub struct PlayerStatus {
    pub position_ticks: i64,
    #[serde(default)]
    pub last_valid_pos: i64,
    pub runtime_ticks: i64,
    pub paused: bool,
    pub volume: i64,
    pub volume_max: i64,
    pub current_idx: usize,
    #[serde(default)]
    pub queue_len: usize,
    /// Monotonic identity for the owner's current queue submission.
    #[serde(default)]
    pub sequence_generation: u64,
    pub active: bool,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    /// Id of the current track's Emby item, used by the root `mbv` crate to
    /// resolve `mpris:artUrl` against the on-disk image cache. Deliberately
    /// NOT a ready-made URL: `mbv-core` has no access to the disk cache
    /// (that lives in the root crate's `config` module) and, per #158's
    /// recorded triage decision, must never build a token-bearing Emby URL
    /// as a fallback. See `mbv-desktop::mpris::resolve_art_url`.
    #[serde(default)]
    pub art_item_id: String,
    /// Album id for the current track, when it's a grouped audio track
    /// (mirrors the `Audio` + non-empty `album_id` grouping the
    /// queue card already uses in `crates/mbv-render/src/components/card.rs`, so the
    /// same disk-cache entry a browsed album card populated can be reused
    /// here). Empty when not applicable.
    #[serde(default)]
    pub art_album_id: String,
    pub audio_tracks: Vec<(i64, String)>,     // (mpv id, label)
    pub sub_tracks: Vec<(i64, String, bool)>, // (mpv id, label, forced)
    #[serde(default)]
    pub sub_track_stream_indexes: Vec<(i64, i64)>, // (mpv id, Emby/ffmpeg stream index)
    pub audio_id: i64,                        // 0 = none/unknown
    pub audio_lang: String, // raw lang code of selected audio track, e.g. "en", "ru"
    pub sub_id: i64,        // 0 = off
    pub sub_lang: String,   // raw lang code of selected sub track, e.g. "en", "eng"
    pub muted: bool,
    pub video_height: i64, // 0 = no video / audio-only
    #[serde(default)]
    pub audio_codec: String, // e.g. "flac", "mp3", "aac"
    #[serde(default)]
    pub video_is_image: bool, // true when the video track is cover art (not real video)
}

impl PlayerStatus {
    /// Copy the start item's playback position, runtime, and identity into
    /// this status — the shared "player about to start / just scheduled"
    /// seeding used by every queue path. Callers set `active`.
    pub fn seed_from_item(&mut self, item: &QueueItem, idx: usize, queue_len: usize) {
        self.position_ticks = item.playback_position_ticks();
        self.runtime_ticks = item.runtime_ticks();
        self.paused = false;
        self.current_idx = idx;
        self.queue_len = queue_len;
        match item {
            QueueItem::Emby(emby) => self.set_current_item_metadata(emby),
            QueueItem::Feed(entry) => {
                self.title.clone_from(&entry.title);
                self.art_item_id.clone_from(&entry.guid);
            }
            QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(ep)) => {
                self.title.clone_from(&ep.title);
                self.art_item_id.clone_from(&ep.episode_id);
            }
            QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book)) => {
                self.title.clone_from(&book.title);
                self.art_item_id.clone_from(&book.library_item_id);
            }
        }
    }

    pub fn set_current_item_metadata(&mut self, item: &EmbyItem) {
        self.title = item.display_name();
        self.artist.clone_from(&item.artist);
        self.album.clone_from(&item.album);
        self.art_item_id.clone_from(&item.id);
        // Same audio-album grouping condition as the queue card
        // (crates/mbv-render/src/components/card.rs) uses for its cache key, so a
        // previously browsed/cached album cover is found under the same key.
        self.art_album_id = if item.item_type == "Audio" && !item.album_id.is_empty() {
            item.album_id.clone()
        } else {
            String::new()
        };
    }

    pub fn clear_current_item_metadata(&mut self) {
        self.title.clear();
        self.artist.clear();
        self.album.clear();
        self.art_item_id.clear();
        self.art_album_id.clear();
    }

    #[must_use]
    pub fn subtitle_stream_index_to_mpv_id(&self, stream_index: i64) -> Option<i64> {
        if stream_index < 0 {
            return Some(0);
        }
        if let Some((id, _)) = self
            .sub_track_stream_indexes
            .iter()
            .find(|(_, idx)| *idx == stream_index)
        {
            return Some(*id);
        }
        if self.sub_track_stream_indexes.is_empty() {
            return self
                .sub_tracks
                .iter()
                .find(|(id, _, _)| *id == stream_index)
                .map(|(id, _, _)| *id);
        }
        None
    }

    #[must_use]
    pub fn next_idx(&self) -> Option<usize> {
        if !self.active {
            return None;
        }
        let n = self.current_idx + 1;
        (n < self.queue_len).then_some(n)
    }

    #[must_use]
    pub fn previous_idx(&self) -> Option<usize> {
        if !self.active || self.current_idx == 0 {
            return None;
        }
        Some(self.current_idx - 1)
    }

    #[must_use]
    pub fn toggle_to_reach(&self, paused: bool) -> Option<PlayerCommand> {
        (self.paused != paused).then_some(PlayerCommand::TogglePause)
    }
}

impl Default for PlayerStatus {
    fn default() -> Self {
        PlayerStatus {
            position_ticks: 0,
            last_valid_pos: 0,
            runtime_ticks: 0,
            paused: false,
            volume: 100,
            volume_max: 130,
            current_idx: 0,
            queue_len: 0,
            sequence_generation: 0,
            active: false,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            art_item_id: String::new(),
            art_album_id: String::new(),
            audio_tracks: Vec::new(),
            sub_tracks: Vec::new(),
            sub_track_stream_indexes: Vec::new(),
            audio_id: 0,
            audio_lang: String::new(),
            sub_id: 0,
            sub_lang: String::new(),
            muted: false,
            video_height: 0,
            audio_codec: String::new(),
            video_is_image: false,
        }
    }
}

/// `run_identity` crosses the ctrl wire as the protocol-10
/// `(PlaybackRequestId, generation)` pair. The request slot was always a
/// hardcoded 0 (dead data), so the pair shape stays on the wire for protocol
/// stability while the model carries the bare generation; a bare scalar is
/// also accepted on decode so peers built while the wire briefly carried the
/// scalar still decode.
mod run_identity_wire {
    use serde::de::{Deserializer, SeqAccess, Visitor};
    use serde::ser::SerializeTuple;

    pub(super) fn serialize<S, T>(generation: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: serde::Serialize + ?Sized,
    {
        let mut pair = serializer.serialize_tuple(2)?;
        pair.serialize_element(&0u64)?;
        pair.serialize_element(generation)?;
        pair.end()
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<crate::PlaybackGeneration, D::Error> {
        deserializer.deserialize_any(GenerationVisitor)
    }

    struct GenerationVisitor;

    impl<'de> Visitor<'de> for GenerationVisitor {
        type Value = crate::PlaybackGeneration;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a playback generation or a (request id, generation) pair")
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<u64, A::Error> {
            let _request_id: u64 = seq
                .next_element()?
                .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
            let generation: u64 = seq
                .next_element()?
                .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
            Ok(generation)
        }
    }
}

pub const CONNECTION_LOST_MESSAGE: &str = "Lost connection to the daemon's device";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum PlayerEvent {
    Stopped {
        /// Owner-assigned identity of the occurrence that stopped, or `None`
        /// for teardown/error synthetics raised with no live queue slot
        /// (spawn failure, panic, daemon disconnect). Resolved from the
        /// Playback run's mpv-local position before the event leaves the run
        /// (design D2). `#[serde(default)]` so a pre-change peer's
        /// index-shaped `Stopped` still decodes (to `None`).
        #[serde(default)]
        slot_id: Option<QueueSlotId>,
        /// Identity of the Playback run that observed this stop: the owner
        /// submission generation at event construction time.
        #[serde(default, with = "run_identity_wire")]
        run_identity: crate::PlaybackGeneration,
        position_ticks: i64,
        played: bool,
        consume: bool,
        error: Option<String>,
    },
    /// mpv advanced to (or was jumped to) an occurrence. Names the
    /// owner-assigned slot identity, never an ordinal.
    TrackChanged {
        slot_id: QueueSlotId,
        /// Present only when this observation settles a dispatched `JumpTo`
        /// transition (design D4): `(request_id, generation)` of that
        /// request. `None` for natural advancement. Bare-mode dispatch
        /// populates this in Section 3; today it is always `None`.
        #[serde(default)]
        transition: Option<(crate::PlaybackRequestId, crate::PlaybackGeneration)>,
    },
    /// Emitted after the player confirms its paused property transition.
    PausedChanged(bool),
    /// mpv's `PlaybackRestart` event. This confirms mbv's player output
    /// boundary, not sound at a downstream pipe consumer.
    OutputStarted,
    TrackCompleted {
        /// Owner-assigned identity of the completed occurrence (design D7).
        slot_id: QueueSlotId,
        /// Identity of the Playback run that observed this completion: the
        /// owner submission generation at event construction time.
        #[serde(default, with = "run_identity_wire")]
        run_identity: crate::PlaybackGeneration,
        position_ticks: i64,
        played: bool,
        consume: bool,
    },
    NextUpThreshold {
        series_id: ItemId,
        season: i64,
        episode: i64,
    },
    NextUpPlay,
    /// Rust identifier renamed from `PlaylistNextUp` (see #104); the wire tag
    /// is pinned via `serde(rename)` so daemon/TUI processes at different
    /// versions during an upgrade still speak the same JSON tag. `PlayerEvent`
    /// has no `WireCommand`-style adapter (unlike `PlayerCommand`, see #81),
    /// so this pin lives directly on the variant.
    /// Look-ahead hint for the *next* occurrence, not an address of an
    /// existing one: `next_idx` stays ordinal (it is only ever read to peek
    /// the upcoming item for the Next-Up card, never to mutate or activate a
    /// slot), per the presentation-coordinate carve-out in the stable-slot
    /// spec rule.
    #[serde(rename = "PlaylistNextUp")]
    QueueNextUp {
        next_idx: usize,
    },
    /// Emitted by `RemotePlayer` when a `UnifiedQueueState` arrives so App can
    /// sync the full canonical queue (tagged `QueueItems`, slot identity, active
    /// slot, revision) without decomposing into legacy Emby-only shapes.
    UnifiedQueueUpdated(Box<crate::UnifiedQueueStateData>),
    /// Result of an owner-authoritative queue operation.
    QueueOpResult {
        op: crate::QueueOpId,
        outcome: crate::QueueOpOutcome,
    },
    /// Correlated result of an owner-authoritative idle queue load.
    UnifiedQueueLoadResult {
        request_id: crate::QueueLoadRequestId,
        result: crate::QueueLoadResult,
    },
    /// Chapter API: playback entered the intro window.
    IntroStarted {
        intro_end_ticks: i64,
    },
    /// Chapter API: playback passed `IntroEnd` (or track changed).
    IntroEnded,
    /// Chapter API: user clicked the "Skip Intro" button in MPV.
    SkipIntroPlay,
    /// mpv exited on its own (user pressed q inside mpv, or mpv crashed).
    MpvQuit,
    /// Emitted when a Player owner cannot act on a command, including local
    /// slot-addressed commands and daemon ctrl-socket commands. The reason
    /// string is owner-computed and shown to the user as-is.
    CommandRejected(String),
    /// Correlated lifecycle update for a guarded direct-daemon playback
    /// intent. The confirmed `PlayerStatus` remains authoritative separately.
    PlaybackIntent(crate::PlaybackIntentEvent),
    /// Direct-daemon pipe startup status; absent for local, Emby-attached,
    /// and non-pipe playback routes.
    PipePlaybackStatus(crate::PipePlaybackStatus),
    /// Emitted by `RemotePlayer` when the daemon intentionally disconnects this
    /// ctrl client (actual connection close, not an authority-change notification).
    RemoteDisconnected(String),
    /// Emitted by `RemotePlayer` when the daemon sends a `Disconnected` notification
    /// for Emby remote authority takeover. Unlike `RemoteDisconnected`, this is
    /// an authority-change notification — the connection stays open.
    EmbyAuthorityTaken(String),
    /// Emitted by `RemotePlayer` when its connection closes after the daemon
    /// announced a deliberate shutdown (`DisconnectReason::DaemonShutdown`).
    /// Unlike `RemoteDisconnected`, this is not a crash: the client SHALL
    /// print one line, restore the terminal, and exit rather than offer
    /// recovery.
    DaemonShutdownAnnounced,
    /// Emitted when an external tool modifies mpv's playlist outside of mbv's
    /// control (e.g. by writing to the mpv IPC socket), causing mbv's queue
    /// mirror to become stale. The detail describes what was detected. The UI
    /// shows this as a warning toast.
    QueueDesynced(String),
    /// The Owner asks this Client to prepare for a Pin swap: save the launch
    /// snapshot now and answer `CtrlCmd::SwapPrepared` (tray-pin-swap design
    /// D2). Sent only to a Client that advertised `pin-swap`.
    SwapPrepare,
    /// The Owner tells this Client to exit now as swapped out: skip the
    /// launch-state save and the coordinated shutdown request (tray-pin-swap
    /// design D2/D6). Sent only to a Client that advertised `pin-swap`.
    SwapQuit,
    /// Emitted by `RemotePlayer` when the daemon sends redacted Audiobookshelf
    /// progress. Dormant: delivered for a future browse-reconciliation
    /// consumer, but nothing applies it to queue or browse state yet.
    AudiobookshelfProgress(crate::AudiobookshelfProgressEvent),
    /// Book-shaped counterpart to `AudiobookshelfProgress`; keyed by
    /// `library_item_id` only.
    AudiobookshelfBookProgress(crate::AudiobookshelfBookProgressEvent),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum PlayerCommand {
    TogglePause,
    /// Jump to an existing queue occurrence by its owner-assigned slot
    /// identity, carrying the request identity of the dispatched explicit
    /// jump (design D4). In-process only — never crosses the ctrl wire: a
    /// stale ordinal cannot be repaired remotely, so the daemon boundary
    /// rejects any inbound legacy `WireCommand::JumpTo` before conversion.
    JumpTo {
        slot_id: QueueSlotId,
        request_id: crate::PlaybackRequestId,
        generation: crate::PlaybackGeneration,
        /// Resume position for the target slot, resolved by the sender from
        /// the canonical queue at dispatch time. On the playlist (non-active-
        /// file) path, mpv only honors a playlist entry's baked `start=`
        /// option the first time it loads, so re-selecting an
        /// already-played-partway entry needs an explicit re-seek; this
        /// carries the value for it. `#[serde(default)]` even though this
        /// command never crosses the ctrl wire (see below), for consistency
        /// with the rest of this enum's evolution.
        #[serde(default)]
        resume_ticks: Option<i64>,
    },
    QueueAppend {
        items: Vec<ExecSlot>,
    },
    /// Remove an existing queue occurrence by its owner-assigned slot
    /// identity (never an ordinal — the occurrence may have moved since the
    /// caller resolved it).
    QueueRemove(QueueSlotId),
    /// Move an existing occurrence, identified by slot identity, to an
    /// ordinal destination position. The destination stays ordinal: it names
    /// a gap in the post-move sequence, resolved immediately by the receiver,
    /// and never crosses back out.
    QueueMove(QueueSlotId, usize),
    SetVolume(i64),
    Seek(f64),
    SeekAbsolute(f64),
    SetAudio(i64),
    SetSub(i64), // 0 = off
    SetSubtitlePrefs {
        mode: String,
        subtitle_lang: String,
        audio_lang: String,
    },
    SetMute(bool),
    LoadNew {
        url: String,
        start_pos: f64,
        item: Box<EmbyItem>,
    },
    NextUpShow {
        item_id: String,
        show_title: String,
        ep_title: String,
        artist: String,
    },
    NextUpDismiss,
    SkipIntroDismiss,
    /// Item-generic queue submission: replace the current queue with `items` and
    /// start playback from `start_idx`. Handles both Emby and Feed items through
    /// the same lifecycle path — source URL and reporting branch on `QueueItem`
    /// variant; everything else is shared.
    SubmitQueue {
        items: Vec<ExecSlot>,
        start_idx: usize,
    },
}
