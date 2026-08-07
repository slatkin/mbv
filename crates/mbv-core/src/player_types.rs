#[derive(Clone, Default)]
pub struct SubtitlePrefs {
    pub mode: String, // "Default"|"Always"|"Smart"|"OnlyForced"|"None"|"HearingImpaired"
    pub subtitle_lang: String, // full language name, e.g. "English"
    pub audio_lang: String, // full language name, e.g. "English"
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
    pub fn set_current_item_metadata(&mut self, item: &MediaItem) {
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

#[derive(serde::Serialize, serde::Deserialize)]
pub enum PlayerEvent {
    Stopped {
        idx: usize,
        position_ticks: i64,
        played: bool,
        consume: bool,
        #[serde(default)]
        progress_report_accepted: bool,
        error: Option<String>,
    },
    TrackChanged(usize),
    /// Emitted after the player confirms its paused property transition.
    PausedChanged(bool),
    /// mpv's `PlaybackRestart` event. This confirms mbv's player output
    /// boundary, not sound at a downstream pipe consumer.
    OutputStarted,
    TrackCompleted {
        idx: usize,
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
    #[serde(rename = "PlaylistNextUp")]
    QueueNextUp {
        next_idx: usize,
    },
    /// Emitted by RemotePlayer when CtrlState arrives so App can sync player_tab.
    QueueUpdated {
        items: Vec<crate::api::MediaItem>,
        cursor: usize,
        source: crate::config::QueueSource,
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
    /// Emitted by RemotePlayer when the daemon reports (via
    /// `CtrlEvent::CommandRejected`) that it didn't act on a ctrl-socket
    /// command. The reason string is server-computed and shown to the user
    /// as-is (e.g. via the transient status toast). See #90.
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
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum PlayerCommand {
    TogglePause,
    JumpTo(usize),
    QueueAppend {
        items: Vec<MediaItem>,
    },
    QueueRemove(usize),
    QueueMove(usize, usize),
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
        item: Box<MediaItem>,
    },
    NextUpShow {
        item_id: String,
        show_title: String,
        ep_title: String,
        artist: String,
    },
    NextUpDismiss,
    SkipIntroDismiss,
    ReplaceQueue {
        items: Vec<MediaItem>,
        start_idx: usize,
    },
}

fn lang_code_to_name(code: &str) -> &'static str {
    match code.to_lowercase().as_str() {
        "en" | "eng" => "English",
        "fr" | "fre" | "fra" => "French",
        "de" | "ger" | "deu" => "German",
        "es" | "spa" => "Spanish",
        "it" | "ita" => "Italian",
        "pt" | "por" => "Portuguese",
        "ja" | "jpn" => "Japanese",
        "ko" | "kor" => "Korean",
        "zh" | "chi" | "zho" => "Chinese",
        "ru" | "rus" => "Russian",
        "ar" | "ara" => "Arabic",
        "nl" | "nld" | "dut" => "Dutch",
        "sv" | "swe" => "Swedish",
        "no" | "nor" => "Norwegian",
        "da" | "dan" => "Danish",
        "fi" | "fin" => "Finnish",
        "pl" | "pol" => "Polish",
        "cs" | "cze" | "ces" => "Czech",
        "tr" | "tur" => "Turkish",
        _ => "",
    }
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

fn auto_select_tracks(mpv: &Mpv, status: &Arc<Mutex<PlayerStatus>>, prefs: &SubtitlePrefs) {
    refresh_tracks(mpv, status);

    // Audio: select track matching AudioLanguagePreference
    if !prefs.audio_lang.is_empty() {
        let (audio_tracks, audio_id) = {
            let s = status.lock().unwrap();
            (s.audio_tracks.clone(), s.audio_id)
        };
        let current_matches = audio_tracks
            .iter()
            .find(|(id, _)| *id == audio_id)
            .is_some_and(|(_, l)| label_matches_lang(l, &prefs.audio_lang));
        if !current_matches {
            if let Some((id, _)) = audio_tracks
                .iter()
                .find(|(_, l)| label_matches_lang(l, &prefs.audio_lang))
            {
                let _ = mpv.set_property("aid", *id);
                status.lock().unwrap().audio_id = *id;
            }
        }
    }

    // Subtitle: apply SubtitleMode
    // For "Default" mode, let mpv honour the stream's default/forced flags without interference.
    if prefs.mode == "Default" || prefs.mode.is_empty() {
        refresh_tracks(mpv, status);
        return;
    }

    let sub_tracks: Vec<(i64, String, bool)> = status.lock().unwrap().sub_tracks.clone();
    let audio_lang_name = {
        let raw = status.lock().unwrap().audio_lang.clone();
        lang_code_to_name(&raw).to_lowercase()
    };
    let sub_pref = prefs.subtitle_lang.to_lowercase();

    let sid: Option<i64> = match prefs.mode.as_str() {
        "None" => None,
        "OnlyForced" => sub_tracks
            .iter()
            .find(|(_, l, forced)| *forced && label_matches_lang(l, &prefs.subtitle_lang))
            .or_else(|| sub_tracks.iter().find(|(_, _, forced)| *forced))
            .map(|(id, _, _)| *id),
        "Always" => sub_tracks
            .iter()
            .find(|(_, l, _)| label_matches_lang(l, &prefs.subtitle_lang))
            .or_else(|| sub_tracks.first())
            .map(|(id, _, _)| *id),
        "Smart" => {
            if !sub_pref.is_empty() && audio_lang_name == sub_pref {
                None
            } else {
                sub_tracks
                    .iter()
                    .find(|(_, l, _)| label_matches_lang(l, &prefs.subtitle_lang))
                    .or_else(|| sub_tracks.first())
                    .map(|(id, _, _)| *id)
            }
        }
        "HearingImpaired" => sub_tracks
            .iter()
            .find(|(_, l, _)| {
                let ll = l.to_lowercase();
                ll.contains("sdh") || ll.contains(" cc") || ll.contains("(cc)")
            })
            .or_else(|| {
                sub_tracks
                    .iter()
                    .find(|(_, l, _)| label_matches_lang(l, &prefs.subtitle_lang))
            })
            .or_else(|| sub_tracks.first())
            .map(|(id, _, _)| *id),
        _ => {
            // Unknown mode: treat like Default, don't interfere
            refresh_tracks(mpv, status);
            return;
        }
    };

    match sid {
        None => {
            let _ = mpv.set_property("sid", "no".to_string());
            status.lock().unwrap().sub_id = 0;
        }
        Some(id) => {
            let _ = mpv.set_property("sid", id);
            status.lock().unwrap().sub_id = id;
        }
    }

    refresh_tracks(mpv, status);
}

fn refresh_tracks(mpv: &Mpv, status: &Arc<Mutex<PlayerStatus>>) {
    let count: i64 = match mpv.get_property("track-list/count") {
        Ok(n) => n,
        Err(_) => return,
    };
    let mut audio: Vec<(i64, String)> = Vec::new();
    let mut subs: Vec<(i64, String, bool)> = Vec::new();
    let mut sub_stream_indexes: Vec<(i64, i64)> = Vec::new();
    let mut audio_id: i64 = 0;
    let mut audio_lang: String = String::new();
    let mut sub_id: i64 = 0;
    let mut sub_lang: String = String::new();

    for i in 0..count {
        let ttype: String = mpv
            .get_property(&format!("track-list/{i}/type"))
            .unwrap_or_default();
        let id: i64 = mpv
            .get_property(&format!("track-list/{i}/id"))
            .unwrap_or(i + 1);
        let lang: String = mpv
            .get_property(&format!("track-list/{i}/lang"))
            .unwrap_or_default();
        let title: String = mpv
            .get_property(&format!("track-list/{i}/title"))
            .unwrap_or_default();
        let codec: String = mpv
            .get_property(&format!("track-list/{i}/codec"))
            .unwrap_or_default();
        let sel: bool = mpv
            .get_property(&format!("track-list/{i}/selected"))
            .unwrap_or(false);

        match ttype.as_str() {
            "audio" => {
                if sel {
                    audio_id = id;
                    audio_lang = lang.clone();
                }
                // Build label from lang+codec+channels to avoid scene-branded titles
                let ch: i64 = mpv
                    .get_property(&format!("track-list/{i}/demux-channel-count"))
                    .unwrap_or(0);
                let name = lang_code_to_name(&lang);
                let label = if !name.is_empty() {
                    let mut parts = vec![name.to_string(), codec.to_uppercase()];
                    let ch_str = fmt_channels(ch);
                    if !ch_str.is_empty() {
                        parts.push(ch_str.to_string());
                    }
                    parts.join(" ")
                } else if !title.is_empty() {
                    title
                } else if !lang.is_empty() {
                    lang.to_uppercase()
                } else {
                    format!("#{}", i + 1)
                };
                audio.push((id, label));
            }
            "sub" if !is_image_sub(&codec) => {
                if sel {
                    sub_id = id;
                    sub_lang = lang.clone();
                }
                let forced: bool = mpv
                    .get_property(&format!("track-list/{i}/forced"))
                    .unwrap_or(false);
                let name = lang_code_to_name(&lang);
                let base_label = if !title.is_empty() {
                    title.clone()
                } else if !name.is_empty() {
                    name.to_string()
                } else if !lang.is_empty() {
                    lang.to_uppercase()
                } else {
                    format!("#{}", i + 1)
                };
                let label = if forced {
                    format!("{base_label} (Forced)")
                } else {
                    base_label
                };
                subs.push((id, label, forced));
                let stream_index: i64 = mpv
                    .get_property(&format!("track-list/{i}/ff-index"))
                    .or_else(|_| mpv.get_property(&format!("track-list/{i}/src-id")))
                    .unwrap_or(-1);
                if stream_index >= 0 {
                    sub_stream_indexes.push((id, stream_index));
                }
            }
            _ => {}
        }
    }

    let mut s = status.lock().unwrap();
    s.audio_tracks = audio;
    s.sub_tracks = subs;
    s.sub_track_stream_indexes = sub_stream_indexes;
    s.audio_id = audio_id;
    s.audio_lang = audio_lang;
    s.sub_id = sub_id;
    s.sub_lang = sub_lang;
}

// ── Session infrastructure ────────────────────────────────────────────────────
