use super::*;

#[derive(Clone, Debug, Default)]
pub struct SubtitlePrefs {
    pub mode: String, // "Default"|"Always"|"Smart"|"OnlyForced"|"None"|"HearingImpaired"
    pub subtitle_lang: String, // full language name, e.g. "English"
    pub audio_lang: String, // full language name, e.g. "English"
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    /// as a fallback. See `src/mpris.rs::resolve_art_url`.
    #[serde(default)]
    pub art_item_id: String,
    /// Album id for the current track, when it's a grouped audio track
    /// (mirrors the `Audio` + non-empty `album_id` grouping the
    /// queue card already uses in `src/app/render/power/card.rs`, so the
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
                self.title = entry.title.clone();
                self.art_item_id = entry.guid.clone();
            }
            QueueItem::Audiobookshelf(ep) => {
                self.title = ep.title.clone();
                self.art_item_id = ep.episode_id.clone();
            }
            QueueItem::AudiobookshelfBook(book) => {
                self.title = book.title.clone();
                self.art_item_id = book.library_item_id.clone();
            }
        }
    }

    pub fn set_current_item_metadata(&mut self, item: &EmbyItem) {
        self.title = item.display_name();
        self.artist = item.artist.clone();
        self.album = item.album.clone();
        self.art_item_id = item.id.clone();
        // Same audio-album grouping condition as the queue card
        // (src/app/render/power/card.rs) uses for its cache key, so a
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

    pub fn next_idx(&self) -> Option<usize> {
        if !self.active {
            return None;
        }
        let n = self.current_idx + 1;
        (n < self.queue_len).then_some(n)
    }

    pub fn previous_idx(&self) -> Option<usize> {
        if !self.active || self.current_idx == 0 {
            return None;
        }
        Some(self.current_idx - 1)
    }

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
        #[serde(default)]
        run_identity: crate::ctrl::PlaybackGeneration,
        position_ticks: i64,
        played: bool,
        consume: bool,
        #[serde(default)]
        progress_report_accepted: bool,
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
        transition: Option<(
            crate::ctrl::PlaybackRequestId,
            crate::ctrl::PlaybackGeneration,
        )>,
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
        #[serde(default)]
        run_identity: crate::ctrl::PlaybackGeneration,
        position_ticks: i64,
        played: bool,
        consume: bool,
        #[serde(default)]
        progress_report_accepted: bool,
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
    /// Emitted by RemotePlayer when a `UnifiedQueueState` arrives so App can
    /// sync the full canonical queue (tagged QueueItems, slot identity, active
    /// slot, revision) without decomposing into legacy Emby-only shapes.
    UnifiedQueueUpdated(Box<crate::ctrl::UnifiedQueueStateData>),
    /// Correlated result of an owner-authoritative idle queue load.
    UnifiedQueueLoadResult {
        request_id: crate::ctrl::QueueLoadRequestId,
        result: crate::ctrl::QueueLoadResult,
    },
    /// Chapter API: playback entered the intro window.
    IntroStarted {
        intro_end_ticks: i64,
    },
    /// Chapter API: playback passed IntroEnd (or track changed).
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
    /// intent. The confirmed PlayerStatus remains authoritative separately.
    PlaybackIntent(crate::ctrl::PlaybackIntentEvent),
    /// Direct-daemon pipe startup status; absent for local, Emby-attached,
    /// and non-pipe playback routes.
    PipePlaybackStatus(crate::ctrl::PipePlaybackStatus),
    /// Emitted by RemotePlayer when the daemon intentionally disconnects this
    /// ctrl client (actual connection close, not an authority-change notification).
    RemoteDisconnected(String),
    /// Emitted by RemotePlayer when the daemon sends a `Disconnected` notification
    /// for Emby remote authority takeover. Unlike `RemoteDisconnected`, this is
    /// an authority-change notification — the connection stays open.
    EmbyAuthorityTaken(String),
    /// Emitted by RemotePlayer when its connection closes after the daemon
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
    /// Emitted by RemotePlayer when the daemon sends redacted Audiobookshelf
    /// progress. Dormant: delivered for a future browse-reconciliation
    /// consumer, but nothing applies it to queue or browse state yet.
    AudiobookshelfProgress(crate::ctrl::AudiobookshelfProgressEvent),
    /// Book-shaped counterpart to `AudiobookshelfProgress`; keyed by
    /// `library_item_id` only.
    AudiobookshelfBookProgress(crate::ctrl::AudiobookshelfBookProgressEvent),
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
        request_id: crate::ctrl::PlaybackRequestId,
        generation: crate::ctrl::PlaybackGeneration,
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
    /// Relative single-step forward nav; carries no request identity (design D4:
    /// relative nav correlates like natural advancement, not a repeated target).
    Next,
    /// Relative single-step backward nav; carries no request identity (see D4).
    Previous,
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

const LANGS: &[(&[&str], &str)] = &[
    (&["en", "eng"], "English"),
    (&["fr", "fre", "fra"], "French"),
    (&["de", "ger", "deu"], "German"),
    (&["es", "spa"], "Spanish"),
    (&["it", "ita"], "Italian"),
    (&["pt", "por"], "Portuguese"),
    (&["ja", "jpn"], "Japanese"),
    (&["ko", "kor"], "Korean"),
    (&["zh", "chi", "zho"], "Chinese"),
    (&["ru", "rus"], "Russian"),
    (&["ar", "ara"], "Arabic"),
    (&["nl", "nld", "dut"], "Dutch"),
    (&["sv", "swe"], "Swedish"),
    (&["no", "nor"], "Norwegian"),
    (&["da", "dan"], "Danish"),
    (&["fi", "fin"], "Finnish"),
    (&["pl", "pol"], "Polish"),
    (&["cs", "cze", "ces"], "Czech"),
    (&["tr", "tur"], "Turkish"),
];

pub(in crate::player) fn lang_code_to_name(code: &str) -> &'static str {
    let code = code.to_lowercase();
    LANGS
        .iter()
        .find_map(|(codes, name)| codes.contains(&code.as_str()).then_some(*name))
        .unwrap_or("")
}

fn fmt_channels(n: i64) -> &'static str {
    match n {
        1 => "Mono",
        2 => "Stereo",
        6 => "5.1",
        8 => "7.1",
        _ => "",
    }
}

fn is_image_sub(codec: &str) -> bool {
    matches!(
        codec,
        "hdmv_pgs_subtitle" | "pgssub" | "dvd_subtitle" | "dvdsub" | "dvb_subtitle" | "xsub"
    )
}

/// Returns true if `label` begins with or contains the full language name `lang_pref`
/// (case-insensitive). Used to match audio/subtitle track labels against a preferred language.
fn label_matches_lang(label: &str, lang_pref: &str) -> bool {
    if lang_pref.is_empty() {
        return false;
    }
    let l = label.to_lowercase();
    let p = lang_pref.to_lowercase();
    l.starts_with(&p)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TrackInfo {
    pub(super) kind: String,
    pub(super) id: i64,
    pub(super) lang: String,
    pub(super) title: String,
    pub(super) codec: String,
    pub(super) selected: bool,
    pub(super) channels: i64,
    pub(super) forced: bool,
    pub(super) stream_index: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ParsedTracks {
    pub(super) audio_tracks: Vec<(i64, String)>,
    pub(super) sub_tracks: Vec<(i64, String, bool)>,
    pub(super) sub_track_stream_indexes: Vec<(i64, i64)>,
    pub(super) audio_id: i64,
    pub(super) audio_lang: String,
    pub(super) sub_id: i64,
    pub(super) sub_lang: String,
}

pub(super) fn parse_tracks(tracks: &[TrackInfo]) -> ParsedTracks {
    let mut parsed = ParsedTracks::default();
    for (index, track) in tracks.iter().enumerate() {
        let fallback = index as i64 + 1;
        match track.kind.as_str() {
            "audio" => {
                if track.selected {
                    parsed.audio_id = track.id;
                    parsed.audio_lang.clone_from(&track.lang);
                }
                let language = lang_code_to_name(&track.lang);
                let label = if !language.is_empty() {
                    let mut parts = vec![language.to_string(), track.codec.to_uppercase()];
                    let channels = fmt_channels(track.channels);
                    parts.extend((!channels.is_empty()).then(|| channels.to_string()));
                    parts.join(" ")
                } else if !track.title.is_empty() {
                    track.title.clone()
                } else if !track.lang.is_empty() {
                    track.lang.to_uppercase()
                } else {
                    format!("#{fallback}")
                };
                parsed.audio_tracks.push((track.id, label));
            }
            "sub" if !is_image_sub(&track.codec) => {
                if track.selected {
                    parsed.sub_id = track.id;
                    parsed.sub_lang.clone_from(&track.lang);
                }
                let language = lang_code_to_name(&track.lang);
                let base = if !track.title.is_empty() {
                    track.title.clone()
                } else if !language.is_empty() {
                    language.to_string()
                } else if !track.lang.is_empty() {
                    track.lang.to_uppercase()
                } else {
                    format!("#{fallback}")
                };
                let label = if track.forced {
                    format!("{base} (Forced)")
                } else {
                    base
                };
                parsed.sub_tracks.push((track.id, label, track.forced));
                if track.stream_index >= 0 {
                    parsed
                        .sub_track_stream_indexes
                        .push((track.id, track.stream_index));
                }
            }
            _ => {}
        }
    }
    parsed
}

/// Returns `(audio_id, subtitle_id)`. `None` means leave that mpv selection unchanged.
pub(super) fn select_tracks(
    audio_tracks: &[(i64, String)],
    sub_tracks: &[(i64, String, bool)],
    audio_id: i64,
    audio_lang: &str,
    prefs: &SubtitlePrefs,
) -> (Option<i64>, Option<Option<i64>>) {
    (
        preferred_audio_track(audio_tracks, audio_id, &prefs.audio_lang),
        preferred_subtitle_track(sub_tracks, audio_lang, prefs),
    )
}

fn preferred_audio_track(tracks: &[(i64, String)], current_id: i64, language: &str) -> Option<i64> {
    if language.is_empty()
        || tracks
            .iter()
            .find(|(id, _)| *id == current_id)
            .is_some_and(|(_, label)| label_matches_lang(label, language))
    {
        None
    } else {
        tracks
            .iter()
            .find(|(_, label)| label_matches_lang(label, language))
            .map(|(id, _)| *id)
    }
}

fn preferred_subtitle_track(
    tracks: &[(i64, String, bool)],
    audio_lang: &str,
    prefs: &SubtitlePrefs,
) -> Option<Option<i64>> {
    match prefs.mode.as_str() {
        "" | "Default" => None,
        "None" => Some(None),
        "OnlyForced" => Some(only_forced_subtitle(tracks, &prefs.subtitle_lang)),
        "Always" => Some(language_subtitle_or_first(tracks, &prefs.subtitle_lang)),
        "Smart" => Some(smart_subtitle(tracks, audio_lang, &prefs.subtitle_lang)),
        "HearingImpaired" => Some(hearing_impaired_subtitle(tracks, &prefs.subtitle_lang)),
        _ => None,
    }
}

fn only_forced_subtitle(tracks: &[(i64, String, bool)], language: &str) -> Option<i64> {
    tracks
        .iter()
        .find(|(_, label, forced)| *forced && label_matches_lang(label, language))
        .or_else(|| tracks.iter().find(|(_, _, forced)| *forced))
        .map(|(id, _, _)| *id)
}

fn language_subtitle_or_first(tracks: &[(i64, String, bool)], language: &str) -> Option<i64> {
    tracks
        .iter()
        .find(|(_, label, _)| label_matches_lang(label, language))
        .or_else(|| tracks.first())
        .map(|(id, _, _)| *id)
}

fn smart_subtitle(
    tracks: &[(i64, String, bool)],
    audio_lang: &str,
    subtitle_lang: &str,
) -> Option<i64> {
    let audio_name = lang_code_to_name(audio_lang).to_lowercase();
    if !subtitle_lang.is_empty() && audio_name == subtitle_lang.to_lowercase() {
        None
    } else {
        language_subtitle_or_first(tracks, subtitle_lang)
    }
}

fn hearing_impaired_subtitle(tracks: &[(i64, String, bool)], language: &str) -> Option<i64> {
    tracks
        .iter()
        .find(|(_, label, _)| {
            let label = label.to_lowercase();
            label.contains("sdh") || label.contains(" cc") || label.contains("(cc)")
        })
        .or_else(|| {
            tracks
                .iter()
                .find(|(_, label, _)| label_matches_lang(label, language))
        })
        .or_else(|| tracks.first())
        .map(|(id, _, _)| *id)
}

pub(super) fn auto_select_tracks(
    mpv: &Mpv,
    status: &Arc<Mutex<PlayerStatus>>,
    prefs: &SubtitlePrefs,
) {
    refresh_tracks(mpv, status);
    let (audio_tracks, audio_id, audio_lang, sub_tracks) = {
        let status = status.lock().unwrap();
        (
            status.audio_tracks.clone(),
            status.audio_id,
            status.audio_lang.clone(),
            status.sub_tracks.clone(),
        )
    };
    let (audio, subtitle) = select_tracks(&audio_tracks, &sub_tracks, audio_id, &audio_lang, prefs);
    if let Some(id) = audio {
        let _ = mpv.set_property("aid", id);
        status.lock().unwrap().audio_id = id;
    }
    if let Some(id) = subtitle {
        match id {
            Some(id) => {
                let _ = mpv.set_property("sid", id);
            }
            None => {
                let _ = mpv.set_property("sid", "no".to_string());
            }
        }
        status.lock().unwrap().sub_id = id.unwrap_or(0);
    }
    refresh_tracks(mpv, status);
}

pub(super) fn refresh_tracks(mpv: &Mpv, status: &Arc<Mutex<PlayerStatus>>) {
    let count: i64 = match mpv.get_property("track-list/count") {
        Ok(n) => n,
        Err(_) => return,
    };
    let tracks = (0..count)
        .map(|index| {
            let property = |name: &str| format!("track-list/{index}/{name}");
            let kind = mpv.get_property(&property("type")).unwrap_or_default();
            let id = mpv.get_property(&property("id")).unwrap_or(index + 1);
            let lang = mpv.get_property(&property("lang")).unwrap_or_default();
            let title = mpv.get_property(&property("title")).unwrap_or_default();
            let codec = mpv.get_property(&property("codec")).unwrap_or_default();
            let selected = mpv.get_property(&property("selected")).unwrap_or(false);
            let channels = mpv
                .get_property(&property("demux-channel-count"))
                .unwrap_or(0);
            let forced = mpv.get_property(&property("forced")).unwrap_or(false);
            let stream_index = mpv
                .get_property(&property("ff-index"))
                .or_else(|_| mpv.get_property(&property("src-id")))
                .unwrap_or(-1);
            TrackInfo {
                kind,
                id,
                lang,
                title,
                codec,
                selected,
                channels,
                forced,
                stream_index,
            }
        })
        .collect::<Vec<_>>();
    let parsed = parse_tracks(&tracks);
    let mut status = status.lock().unwrap();
    status.audio_tracks = parsed.audio_tracks;
    status.sub_tracks = parsed.sub_tracks;
    status.sub_track_stream_indexes = parsed.sub_track_stream_indexes;
    status.audio_id = parsed.audio_id;
    status.audio_lang = parsed.audio_lang;
    status.sub_id = parsed.sub_id;
    status.sub_lang = parsed.sub_lang;
}

// ── Session infrastructure ────────────────────────────────────────────────────
