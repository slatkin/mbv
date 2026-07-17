use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use crate::api::{EmbyClient, MediaItem, TICKS_PER_SECOND};
use crate::playback_queue::{PlaybackQueue, QueueSlotId};
use libmpv2::{
    events::{Event, PropertyData},
    mpv_end_file_reason, EndFileReason, Format, Mpv,
};

fn mpv_err_str(e: &libmpv2::Error) -> String {
    if let libmpv2::Error::Raw(code) = e {
        format!("Raw({}) [{}]", code, libmpv2_sys::mpv_error_str(*code))
    } else {
        format!("{e:?}")
    }
}

fn mpv_title_opt(title: &str) -> String {
    // Use mpv's %N% length-prefix format so the value is passed verbatim —
    // no escaping needed, handles commas, backslashes, and any other character.
    format!("force-media-title=%{}%{}", title.len(), title)
}

fn send_ep_info(mpv: &Mpv, item: &crate::api::MediaItem) {
    let val =
        if item.item_type == "Episode" && item.parent_index_number > 0 && item.index_number > 0 {
            format!(
                "Season {}  Episode {}",
                item.parent_index_number, item.index_number
            )
        } else {
            String::new()
        };
    let _ = mpv.set_property("user-data/mbv/ep-tag", val.as_str());
}

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
    /// (mirrors the `Audio` + non-empty `album_id` grouping the Power View
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
        // Same audio-album grouping condition as the Power View queue card
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
    TrackCompleted {
        idx: usize,
        position_ticks: i64,
        played: bool,
        consume: bool,
        #[serde(default)]
        progress_report_accepted: bool,
    },
    NextUpThreshold {
        series_id: String,
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
    /// Emitted by RemotePlayer when the daemon intentionally disconnects this
    /// ctrl client, for example because another controller took over.
    RemoteDisconnected(String),
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

struct ProgressGuard {
    stop_tx: mpsc::Sender<()>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ProgressGuard {
    fn stop_and_join(&mut self, budget: Duration) {
        let _ = self.stop_tx.send(());
        if let Some(h) = self.handle.take() {
            let start = std::time::Instant::now();
            let result = crate::bounded::run_with_hard_bound(
                move || {
                    let _ = h.join();
                    Ok::<(), String>(())
                },
                budget,
            );
            let elapsed = start.elapsed();
            match result {
                Ok(()) => {
                    log::info!(target: "player", "progress_join: joined in {}ms (budget={}ms)",
                    elapsed.as_millis(), budget.as_millis())
                }
                Err(e) => {
                    log::warn!(target: "player", "progress_join: {e} after {}ms (budget={}ms)",
                    elapsed.as_millis(), budget.as_millis())
                }
            }
        }
    }
}

struct MpvSessionConfig {
    headless: bool,
    use_mpv_config: bool,
    no_scripts: bool,
    always_skip_intro: bool,
    audio_pipe_path: Option<String>,
    audio_pipe_samplerate: u32,
    audio_pipe_bitdepth: u8,
}

fn user_mpv_config_dir() -> Option<PathBuf> {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(config_home).join("mpv"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("mpv"))
}

fn is_mpv_ipc_config_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return false;
    }
    let option = trimmed.strip_prefix("--").unwrap_or(trimmed);
    let key_end = option
        .find(|c: char| c == '=' || c.is_whitespace())
        .unwrap_or(option.len());
    &option[..key_end] == "input-ipc-server"
}

fn sanitized_mpv_conf(user_conf: Option<&Path>, ipc_path: &str) -> String {
    let mut sanitized = String::new();
    if let Some(path) = user_conf {
        if let Ok(text) = fs::read_to_string(path) {
            for line in text.lines() {
                if !is_mpv_ipc_config_line(line) {
                    sanitized.push_str(line);
                    sanitized.push('\n');
                }
            }
        }
    }
    sanitized.push_str("input-ipc-server=");
    sanitized.push_str(ipc_path);
    sanitized.push('\n');
    sanitized
}

#[cfg(unix)]
fn symlink_mpv_config_entry(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dest)
}

#[cfg(not(unix))]
fn symlink_mpv_config_entry(src: &Path, dest: &Path) -> std::io::Result<()> {
    let meta = fs::metadata(src)?;
    if meta.is_dir() {
        fs::create_dir(dest)
    } else {
        fs::copy(src, dest).map(|_| ())
    }
}

fn reset_private_mpv_config_dir(private_dir: &Path) -> Result<(), String> {
    match fs::symlink_metadata(private_dir) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {
            fs::remove_dir_all(private_dir).map_err(|e| {
                format!(
                    "failed to remove private mpv config dir '{}': {e}",
                    private_dir.display()
                )
            })?;
        }
        Ok(_) => {
            fs::remove_file(private_dir).map_err(|e| {
                format!(
                    "failed to remove private mpv config path '{}': {e}",
                    private_dir.display()
                )
            })?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(format!(
                "failed to inspect private mpv config dir '{}': {e}",
                private_dir.display()
            ));
        }
    }
    fs::create_dir_all(private_dir).map_err(|e| {
        format!(
            "failed to create private mpv config dir '{}': {e}",
            private_dir.display()
        )
    })
}

fn prepare_mpv_config_dir(use_mpv_config: bool, ipc_path: &str) -> Result<PathBuf, String> {
    let private_dir = crate::config::mpv_config_dir();
    reset_private_mpv_config_dir(&private_dir)?;

    let user_dir = use_mpv_config.then(user_mpv_config_dir).flatten();
    if let Some(user_dir) = &user_dir {
        match fs::read_dir(user_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name == OsStr::new("mpv.conf") || name == OsStr::new("input.conf") {
                        continue;
                    }
                    let src = entry.path();
                    let dest = private_dir.join(&name);
                    if let Err(e) = symlink_mpv_config_entry(&src, &dest) {
                        log::warn!(target: "player", "mpv config: failed to link {} into private config dir: {e}", src.display());
                    }
                }
            }
            Err(e) => {
                log::warn!(target: "player", "mpv config: cannot read user config dir {}: {e}", user_dir.display());
            }
        }
    }

    let user_conf = user_dir
        .as_ref()
        .map(|dir| dir.join("mpv.conf"))
        .filter(|path| path.exists());
    let conf = sanitized_mpv_conf(user_conf.as_deref(), ipc_path);
    fs::write(private_dir.join("mpv.conf"), conf).map_err(|e| {
        format!(
            "failed to write private mpv.conf in '{}': {e}",
            private_dir.display()
        )
    })?;

    Ok(private_dir)
}

// Ensures `path` exists as a FIFO, creating it via mkfifo(3) if it doesn't
// already exist. Refuses to touch a path that exists but isn't a FIFO.
fn ensure_pipe(path: &str) -> Result<(), String> {
    use std::os::unix::fs::FileTypeExt;
    match std::fs::metadata(path) {
        Ok(meta) if meta.file_type().is_fifo() => Ok(()),
        Ok(_) => Err(format!("audio pipe path '{path}' exists and is not a FIFO")),
        Err(_) => {
            let cpath = std::ffi::CString::new(path).map_err(|e| e.to_string())?;
            let rc = unsafe { libc::mkfifo(cpath.as_ptr(), 0o644) };
            if rc != 0 {
                Err(format!(
                    "mkfifo({path}) failed: {}",
                    std::io::Error::last_os_error()
                ))
            } else {
                Ok(())
            }
        }
    }
}

// Shared between the event loop thread and the progress reporter thread.
// All mutable fields are Arc-wrapped so transitions are visible to both.
#[derive(Clone)]
struct SessionReporter {
    client: Arc<EmbyClient>,
    ws_tx: Option<crate::ws::WsSender>,
    // (item_id, msid, sid) in a single lock so progress and event-loop threads never
    // observe a torn triple during item transitions.
    ids: Arc<Mutex<(String, String, String)>>,
    // Shared with progress thread so it knows whether to send progress or just ping.
    is_audio: Arc<AtomicBool>,
    status: Arc<Mutex<PlayerStatus>>,
}

impl SessionReporter {
    fn new(
        client: Arc<EmbyClient>,
        ws_tx: Option<crate::ws::WsSender>,
        item_id: String,
        msid: String,
        sid: String,
        is_audio: bool,
        status: Arc<Mutex<PlayerStatus>>,
    ) -> Self {
        SessionReporter {
            client,
            ws_tx,
            ids: Arc::new(Mutex::new((item_id, msid, sid))),
            is_audio: Arc::new(AtomicBool::new(is_audio)),
            status,
        }
    }

    // Sends progress via websocket when connected, otherwise falls back to HTTP.
    // Recovers from poisoned mutexes so the progress thread never panics while
    // holding a lock.
    fn report_progress(&self, event_name: &str) {
        let (id, msid, sid) = self.ids.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let (pos, runtime, paused) = {
            let s = self.status.lock().unwrap_or_else(|e| e.into_inner());
            (s.position_ticks, s.runtime_ticks, s.paused)
        };
        if let Some(ref tx) = self.ws_tx {
            if tx.is_connected() {
                self.client
                    .report_progress_ws(&id, &msid, pos, runtime, paused, &sid, event_name, tx);
                return;
            }
        }
        self.client
            .report_progress_http(&id, &msid, pos, paused, &sid, event_name);
    }

    // Zeroes position for audio items so Emby doesn't resume audio from mid-track.
    fn report_stopped(&self, last_valid_pos: i64) -> bool {
        let (id, msid, sid) = self.ids.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let is_audio = self.is_audio.load(Ordering::Relaxed);
        let pos = if is_audio { 0 } else { last_valid_pos };
        let runtime_ticks = self
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .runtime_ticks;
        if let Some(ref tx) = self.ws_tx {
            if tx.is_connected() {
                let _ = tx.flush(Duration::from_secs(1));
            }
        }
        log::info!(target: "player", "report_stopped: item={id} is_audio={is_audio} last_valid_pos={}s sending pos={}s",
            last_valid_pos / TICKS_PER_SECOND, pos / TICKS_PER_SECOND);
        self.client
            .report_stopped(&id, &msid, pos, &sid, runtime_ticks)
    }

    fn report_stopped_for_shutdown(&self, last_valid_pos: i64, timeout: Duration) -> bool {
        let (id, msid, sid) = self.ids.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let is_audio = self.is_audio.load(Ordering::Relaxed);
        let pos = if is_audio { 0 } else { last_valid_pos };
        let runtime_ticks = self
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .runtime_ticks;
        if let Some(ref tx) = self.ws_tx {
            if tx.is_connected() {
                let _ = tx.flush(timeout.min(Duration::from_secs(1)));
            }
        }
        log::info!(target: "player", "report_stopped shutdown: item={id} is_audio={is_audio} last_valid_pos={}s sending pos={}s timeout={}ms",
            last_valid_pos / TICKS_PER_SECOND, pos / TICKS_PER_SECOND, timeout.as_millis());
        self.client
            .report_stopped_for_shutdown(&id, &msid, pos, &sid, runtime_ticks, timeout)
    }

    fn report_ping(&self) {
        let sid = self.ids.lock().unwrap_or_else(|e| e.into_inner()).2.clone();
        self.client.report_ping(&sid);
    }

    // get_playback_info + report_start for a new item, updating tracking ids
    // *before* the network call so the progress reporter thread never sends
    // stale IDs to Emby.
    // Returns (ext_sub_urls, success).
    fn start_item(&self, item: &MediaItem) -> (Vec<String>, bool) {
        let info = self.client.get_playback_info(&item.id);
        // Update ids before report_start so the progress reporter (which reads
        // ids on a 10-second timer) always sees the new item.
        {
            let mut ids = self.ids.lock().unwrap_or_else(|e| e.into_inner());
            ids.0 = item.id.clone();
            ids.1 = info.media_source_id.clone();
            ids.2 = info.session_id.clone();
        }
        self.is_audio.store(item.is_audio(), Ordering::Relaxed);
        let ok = self
            .client
            .report_start(item, &info.media_source_id, &info.session_id);
        (info.external_subtitle_urls, ok)
    }

    // report_stopped for current item then start_item for the new one.
    // Returns ext_sub_urls for the new item. Logs warnings on API failures.
    fn transition_to(&self, new_item: &MediaItem, last_valid_pos: i64) -> Vec<String> {
        let stop_ok = self.report_stopped(last_valid_pos);
        if !stop_ok {
            log::warn!(target: "player", "transition_to: report_stopped failed for prev item");
        }
        let (ext_sub_urls, start_ok) = self.start_item(new_item);
        if !start_ok {
            log::warn!(target: "player", "transition_to: report_start failed for item={}", new_item.id);
        }
        ext_sub_urls
    }
}

fn init_mpv(config: &MpvSessionConfig) -> Result<(Mpv, bool), String> {
    let ipc_path = crate::config::mpv_ipc_path();
    let private_config_dir = prepare_mpv_config_dir(config.use_mpv_config, &ipc_path)?;
    let ipc_existed = Path::new(&ipc_path).exists();
    if ipc_existed {
        let _ = std::fs::remove_file(&ipc_path);
        log::info!(target: "player", "init: removed stale ipc socket {}", ipc_path);
    }
    log::info!(target: "player", "init: ipc={} (existed={})", ipc_path, ipc_existed);

    let no_scripts = config.no_scripts;
    let use_mpv_config = config.use_mpv_config;
    let mut init_err: Option<String> = None;
    let mpv = match Mpv::with_initializer(|init| {
        macro_rules! opt {
            ($k:expr, $v:expr) => {{
                let r = init.set_option($k, $v);
                if let Err(ref e) = r {
                    init_err = Some(format!(
                        "[player] set_option('{}') failed: {}",
                        $k,
                        mpv_err_str(e)
                    ));
                }
                r?;
            }};
        }
        opt!("config", "yes");
        // Use an mbv-owned config dir so user mpv.conf cannot override
        // input-ipc-server during mpv_initialize() and clobber a live mpv socket.
        opt!("config-dir", private_config_dir.to_str().unwrap_or(""));
        opt!("input-ipc-server", ipc_path.as_str());
        opt!("input-default-bindings", "yes");
        opt!("input-vo-keyboard", "yes");
        opt!("wayland-app-id", "mbv");
        opt!("demuxer-max-bytes", "50M");
        opt!("demuxer-max-back-bytes", "10M");
        opt!("gapless-audio", "weak");
        if no_scripts || !use_mpv_config {
            opt!("load-scripts", "no");
            opt!("osc", "no");
            opt!("osd-bar", "no");
        }
        if !no_scripts && !use_mpv_config {
            let script = crate::config::osc_script_path();
            if script.exists() {
                opt!("scripts", script.to_str().unwrap_or(""));
                let fonts = crate::config::osc_fonts_dir();
                opt!("osd-fonts-dir", fonts.to_str().unwrap_or(""));
            }
        }
        Ok(())
    }) {
        Ok(m) => m,
        Err(e) => {
            let msg =
                init_err.unwrap_or_else(|| format!("[player] mpv init error: {}", mpv_err_str(&e)));
            return Err(msg);
        }
    };

    unsafe {
        let log_level = if cfg!(debug_assertions) {
            c"warn"
        } else {
            c"error"
        };
        libmpv2_sys::mpv_request_log_messages(mpv.ctx.as_ptr(), log_level.as_ptr() as _);
    }

    // Set after init so user's mpv.conf cannot override these.
    if config.headless {
        let _ = mpv.set_property("vo", "null");
        let _ = mpv.set_property("force-window", "no");
    }
    let mut startup_pause_armed = false;
    if let Some(path) = &config.audio_pipe_path {
        match ensure_pipe(path) {
            Ok(()) => {
                let rate = config.audio_pipe_samplerate.to_string();
                let (bitdepth, audio_format) = match config.audio_pipe_bitdepth {
                    16 => (16u8, "s16"),
                    24 => (24u8, "s24"),
                    _ => (32u8, "s32"),
                };
                let mut failed = Vec::new();
                if let Err(e) = mpv.set_property("ao", "pcm") {
                    failed.push(format!("ao: {}", mpv_err_str(&e)));
                }
                if let Err(e) = mpv.set_property("ao-pcm-file", path.as_str()) {
                    failed.push(format!("ao-pcm-file: {}", mpv_err_str(&e)));
                }
                if let Err(e) = mpv.set_property("ao-pcm-waveheader", "no") {
                    failed.push(format!("ao-pcm-waveheader: {}", mpv_err_str(&e)));
                }
                // Force a fixed <bitdepth>-bit/stereo/<rate> PCM format so the byte
                // stream always matches a single Snapcast `sampleformat`
                // declaration, no matter the source file's native format.
                // 32-bit remains the default for headroom, but narrower
                // bit depths improve compatibility with some Snapclients.
                if let Err(e) = mpv.set_property("audio-format", audio_format) {
                    failed.push(format!("audio-format: {}", mpv_err_str(&e)));
                }
                if let Err(e) = mpv.set_property("audio-channels", "stereo") {
                    failed.push(format!("audio-channels: {}", mpv_err_str(&e)));
                }
                if let Err(e) = mpv.set_property("audio-samplerate", rate.as_str()) {
                    failed.push(format!("audio-samplerate: {}", mpv_err_str(&e)));
                }
                if let Err(e) =
                    mpv.set_property("audio-swresample-o", "resampler=soxr,precision=28")
                {
                    failed.push(format!("audio-swresample-o: {}", mpv_err_str(&e)));
                }
                if failed.is_empty() {
                    startup_pause_armed = true;
                    log::info!(target: "player", "audio pipe: writing {rate}Hz/{bitdepth}-bit/stereo PCM to {path} (blocks until a reader attaches)");
                } else {
                    log::warn!(target: "player", "audio pipe: failed to configure pcm output for {path}: {}", failed.join(", "));
                }
            }
            Err(e) => log::warn!(target: "player", "audio pipe disabled for this session: {e}"),
        }
    }
    if startup_pause_armed {
        if let Err(e) = mpv.set_property("pause", true) {
            log::warn!(
                target: "player",
                "audio pipe: failed to pre-pause startup: {}",
                mpv_err_str(&e)
            );
            startup_pause_armed = false;
        }
    }

    Ok((mpv, startup_pause_armed))
}

fn init_volume(mpv: &Mpv, status: &Arc<Mutex<PlayerStatus>>, initial_volume: u8) {
    let mut st = status.lock().unwrap();
    let raw_max = mpv.get_property::<i64>("volume-max").unwrap_or(130);
    st.volume_max = raw_max * raw_max / 100;
    let v = (initial_volume as i64).clamp(0, st.volume_max);
    let raw = (10.0 * (v as f64).sqrt()).round() as i64;
    let _ = mpv.set_property("volume", raw as f64);
    st.volume = v;
}

fn observe_properties(mpv: &Mpv, use_mpv_config: bool) {
    let _ = mpv.observe_property("time-pos", Format::Double, 0);
    let _ = mpv.observe_property("pause", Format::Flag, 1);
    let _ = mpv.observe_property("volume", Format::Double, 2);
    let _ = mpv.observe_property("sid", Format::String, 3);
    let _ = mpv.observe_property("mute", Format::Flag, 4);
    let _ = mpv.observe_property("aid", Format::String, 5);
    let _ = mpv.observe_property("video-params/h", Format::Int64, 6);
    let _ = mpv.observe_property("audio-codec-name", Format::String, 7);
    let _ = mpv.observe_property("current-tracks/video/image", Format::Flag, 8);
    let _ = mpv.observe_property("playlist-pos", Format::Int64, 9);
    let _ = mpv.observe_property("playlist-count", Format::Int64, 10);
    if use_mpv_config {
        let _ = mpv.command("keybind", &["MOUSE_MOVE", "script-message mouse-moved"]);
    }
}

fn spawn_progress_reporter(reporter: SessionReporter) -> ProgressGuard {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let interval = Duration::from_secs(reporter.client.config.progress_interval_secs);
    let handle = thread::spawn(move || loop {
        match stop_rx.recv_timeout(interval) {
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                reporter.report_progress("TimeUpdate");
                reporter.report_ping();
            }
        }
    });
    ProgressGuard {
        stop_tx,
        handle: Some(handle),
    }
}

fn load_intro_times(client: &EmbyClient, item_id: &str) -> (i64, i64) {
    if client.chapter_api_available {
        client.get_intro_times(item_id).unwrap_or((0, 0))
    } else {
        (0, 0)
    }
}

fn handle_intro(
    ticks: i64,
    start: i64,
    end: i64,
    show_fired: &mut bool,
    hide_fired: &mut bool,
    always_skip: bool,
    mpv: &Mpv,
    event_tx: &mpsc::Sender<PlayerEvent>,
) {
    if end <= start {
        return;
    }
    if !*show_fired && ticks >= start {
        *show_fired = true;
        if ticks < end {
            let end_secs = end as f64 / TICKS_PER_SECOND as f64;
            if always_skip {
                let _ = mpv.set_property("time-pos", end_secs);
            } else {
                let _ = event_tx.send(PlayerEvent::IntroStarted {
                    intro_end_ticks: end,
                });
                let _ = mpv.command("script-message", &["mbv-skip-intro", &end_secs.to_string()]);
            }
        } else {
            *hide_fired = true;
        }
    }
    if !*hide_fired && ticks >= end {
        *hide_fired = true;
        let _ = event_tx.send(PlayerEvent::IntroEnded);
        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
    }
}

// ── PlaybackSession ────────────────────────────────────────────────────────

/// Where index `idx` ends up after moving the entry at `from` to `to`
/// (both 0-based positions in the same list, `from != to`).
pub(crate) fn shift_index_for_move(idx: usize, from: usize, to: usize) -> usize {
    if idx == from {
        to
    } else if from < idx && idx <= to {
        idx - 1
    } else if to <= idx && idx < from {
        idx + 1
    } else {
        idx
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum PlaybackOrigin {
    Standalone,
    Queue,
}

struct PlaybackSession {
    origin: PlaybackOrigin,
    config: MpvSessionConfig,
    reporter: SessionReporter,
    event_tx: mpsc::Sender<PlayerEvent>,
    status: Arc<Mutex<PlayerStatus>>,
    subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    is_queue_mode: Arc<AtomicBool>,
    server_url: String,
    token: String,
    queue: PlaybackQueue,
    ext_sub_urls: Vec<String>,
    // loop state
    current_idx: usize,
    forced_slot_id: Option<QueueSlotId>,
    quit_at: Option<Instant>,
    last_seek_at: Option<Instant>,
    last_valid_pos: i64,
    tracks_initialized: bool,
    pending_load: u8,
    pending_initial_jump: bool,
    stop_reported: bool,
    stop_report_accepted: bool,
    stopped_event_sent: bool,
    mark_played_id: Option<String>,
    osd_title: String,
    last_mouse_osd: Option<Instant>,
    pending_resume_secs: Option<f64>,
    series_id: String,
    season: i64,
    episode: i64,
    next_up_fired: bool,
    next_up_armed: bool,
    queue_next_up_fired: bool,
    queue_next_up_armed: bool,
    next_up_jump: bool,
    stopped_near_end: bool,
    shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    startup_pause_release_pending: bool,
    startup_pause_events_to_skip: u8,
    // intro
    intro_start: i64,
    intro_end: i64,
    intro_show: bool,
    intro_hide: bool,
}

impl PlaybackSession {
    fn queue_len(&self) -> usize {
        self.queue.slots().len()
    }

    fn slot_id_at(&self, idx: usize) -> Option<QueueSlotId> {
        self.queue.slots().get(idx).map(|slot| slot.slot_id)
    }

    fn item_at(&self, idx: usize) -> Option<&MediaItem> {
        self.queue.slots().get(idx).map(|slot| &slot.item)
    }

    fn active_item(&self) -> Option<&MediaItem> {
        self.queue.active_slot().map(|slot| &slot.item)
    }

    fn active_slot_id(&self) -> Option<QueueSlotId> {
        self.queue.active_slot_id()
    }

    fn set_origin(&self, origin: PlaybackOrigin) {
        self.is_queue_mode
            .store(origin == PlaybackOrigin::Queue, Ordering::Relaxed);
    }

    fn report_stopped_for_current_context(&self) -> bool {
        if let Some(timeout) = *self.shutdown_report_timeout.lock().unwrap() {
            self.reporter
                .report_stopped_for_shutdown(self.last_valid_pos, timeout)
        } else {
            self.reporter.report_stopped(self.last_valid_pos)
        }
    }

    fn report_stopped_for_end_file(&self, reason: EndFileReason) -> bool {
        match end_file_stop_report_context(reason) {
            StopReportContext::Ordinary => self.reporter.report_stopped(self.last_valid_pos),
            StopReportContext::ShutdownAware => self.report_stopped_for_current_context(),
        }
    }

    /// Budget for `ProgressGuard::stop_and_join`. During a real quit
    /// (`shutdown_report_timeout` set via `Player::stop_for_shutdown`),
    /// this is deliberately *half* of `quit_timeout_secs`, not the full
    /// value: `report_stopped_for_shutdown` (see
    /// `report_stopped_for_current_context`) keeps the full
    /// `quit_timeout_secs` as its own budget per the spec's resolved
    /// design (it's the session-terminating call and the one worth
    /// protecting most), so giving this secondary, non-network-critical
    /// join the same full budget would leave the outer teardown bound
    /// with only a thin, constant margin over the worst case of the two
    /// nested calls combined — see `App::teardown`'s `outer_bound` for
    /// the composition this budget feeds into. Outside of shutdown
    /// (ordinary track transitions), there is no time pressure, so a
    /// generous fixed budget (matching the shared agent's own ~30s worst
    /// case) just guards against a truly stuck thread without adding
    /// latency to the common fast case.
    fn progress_join_budget(&self) -> Duration {
        match *self.shutdown_report_timeout.lock().unwrap() {
            Some(quit_timeout) => quit_timeout / 2,
            None => Duration::from_secs(30),
        }
    }

    /// Clears any pending-quit state so a `LoadNew`/`ReplaceQueue` command
    /// that arrives while a quit is in flight fully cancels it — not just
    /// `quit_at`, but also the shutdown-scoped report budget set by
    /// `Player::stop_for_shutdown`. Without resetting
    /// `shutdown_report_timeout` here too, a cancelled quit would leave it
    /// `Some` for the rest of this `PlaybackSession`'s lifetime (nothing
    /// else clears it once set), so every subsequent track transition
    /// would silently keep using the tight shutdown budget/no-retry
    /// behavior via `progress_join_budget`/`report_stopped_for_current_context`
    /// instead of the ordinary one — no crash, just quietly degraded
    /// reliability for the rest of the session.
    fn cancel_pending_quit(&mut self) {
        self.quit_at = None;
        *self.shutdown_report_timeout.lock().unwrap() = None;
    }

    fn sync_status_position(&self) {
        let mut s = self.status.lock().unwrap();
        s.current_idx = self.current_idx;
        s.queue_len = self.queue_len();
    }

    fn refresh_current_idx_from_queue(&mut self) {
        if let Some(slot_id) = self.active_slot_id() {
            if let Some(idx) = self.queue.slot_index(slot_id) {
                self.current_idx = idx;
            }
        } else if self.queue_len() == 0 {
            self.current_idx = 0;
        } else {
            self.current_idx = self.current_idx.min(self.queue_len() - 1);
        }
        self.sync_status_position();
    }

    fn set_active_index(&mut self, idx: usize) -> bool {
        let Some(slot_id) = self.slot_id_at(idx) else {
            return false;
        };
        if !matches!(
            self.queue.set_active_slot(slot_id),
            crate::playback_queue::QueueMutationResult::Applied(())
        ) {
            return false;
        }
        self.current_idx = idx;
        self.sync_status_position();
        true
    }

    fn reset_next_up_state(&mut self) {
        self.next_up_fired = false;
        self.next_up_armed = false;
        self.queue_next_up_fired = false;
        self.queue_next_up_armed = false;
        self.next_up_jump = false;
    }

    fn load_active_item_state(&mut self) {
        let Some(item) = self.active_item().cloned() else {
            self.osd_title.clear();
            self.pending_resume_secs = None;
            self.last_valid_pos = 0;
            self.series_id.clear();
            self.season = 0;
            self.episode = 0;
            self.intro_start = 0;
            self.intro_end = 0;
            self.intro_show = false;
            self.intro_hide = false;
            return;
        };

        self.osd_title = item.display_name();
        self.last_valid_pos = if item.is_audio() {
            0
        } else {
            item.playback_position_ticks
        };
        self.pending_resume_secs = if self.origin == PlaybackOrigin::Standalone {
            // Standalone fresh-start (cmd_load_new) already sets the mpv `start`
            // property to the resume position before calling this; setting
            // pending_resume_secs too would trigger a redundant absolute seek
            // in on_playback_restart that also suppresses the first progress
            // report for ~500ms. Queue playback and mid-session slot activation
            // always run with Queue origin, so they are unaffected.
            None
        } else if !item.is_audio() && item.should_resume() {
            Some(item.resume_seconds())
        } else {
            None
        };
        if item.item_type == "Episode" {
            self.series_id = item.series_id.clone();
            self.season = item.parent_index_number;
            self.episode = item.index_number;
        } else {
            self.series_id.clear();
            self.season = 0;
            self.episode = 0;
        }
        let (intro_start, intro_end) = load_intro_times(&self.reporter.client, &item.id);
        self.set_intro(intro_start, intro_end, item.playback_position_ticks);
    }

    // `startup_pause_for_pipe` (added for audio-pipe startup-pause handling)
    // pushed this past clippy's 10-argument default; grouping these into a
    // params struct is a reasonable follow-up but out of scope here.
    #[allow(clippy::too_many_arguments)]
    fn new(
        items: Vec<MediaItem>,
        start_idx: usize,
        origin: PlaybackOrigin,
        reporter: SessionReporter,
        config: MpvSessionConfig,
        startup_pause_for_pipe: bool,
        status: Arc<Mutex<PlayerStatus>>,
        event_tx: mpsc::Sender<PlayerEvent>,
        subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
        is_queue_mode: Arc<AtomicBool>,
        shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
        server_url: String,
        token: String,
        ext_sub_urls: Vec<String>,
    ) -> Self {
        let start_idx = start_idx.min(items.len().saturating_sub(1));
        let queue = PlaybackQueue::from_items(items, Some(start_idx));
        let initial_item = queue
            .active_slot()
            .map(|slot| slot.item.clone())
            .expect("PlaybackSession::new requires at least one item");
        let initial_pos = if initial_item.is_audio() {
            0
        } else {
            initial_item.playback_position_ticks
        };
        let (intro_start, intro_end) = load_intro_times(&reporter.client, &initial_item.id);
        let past = intro_end > 0 && initial_pos >= intro_end;
        let pending_resume_secs = if !initial_item.is_audio() && initial_item.should_resume() {
            Some(initial_item.resume_seconds())
        } else {
            None
        };
        log::info!(
            target: "player",
            "playback init origin={origin:?} idx={start_idx} item_pos={}s pending_resume={pending_resume_secs:?}s",
            initial_pos / crate::api::TICKS_PER_SECOND
        );
        let osd_title = initial_item.display_name();
        let series_id = if initial_item.item_type == "Episode" {
            initial_item.series_id.clone()
        } else {
            String::new()
        };
        let session = PlaybackSession {
            origin,
            config,
            reporter,
            event_tx,
            status,
            subtitle_prefs,
            is_queue_mode,
            server_url,
            token,
            queue,
            ext_sub_urls,
            current_idx: start_idx,
            forced_slot_id: None,
            quit_at: None,
            last_seek_at: None,
            last_valid_pos: initial_pos,
            tracks_initialized: false,
            pending_load: 0,
            pending_initial_jump: start_idx > 0,
            stop_reported: false,
            stop_report_accepted: false,
            stopped_event_sent: false,
            mark_played_id: None,
            last_mouse_osd: None,
            series_id,
            season: initial_item.parent_index_number,
            episode: initial_item.index_number,
            next_up_fired: false,
            next_up_armed: false,
            queue_next_up_fired: false,
            queue_next_up_armed: false,
            next_up_jump: false,
            stopped_near_end: false,
            shutdown_report_timeout,
            startup_pause_release_pending: startup_pause_for_pipe,
            startup_pause_events_to_skip: if startup_pause_for_pipe { 2 } else { 0 },
            intro_start,
            intro_end,
            intro_show: past,
            intro_hide: past,
            osd_title,
            pending_resume_secs,
        };
        session.set_origin(origin);
        session
    }

    fn set_intro(&mut self, start: i64, end: i64, pos: i64) {
        self.intro_start = start;
        self.intro_end = end;
        let past = end > 0 && pos >= end;
        self.intro_show = past;
        self.intro_hide = past;
    }

    fn handle_command(
        &mut self,
        cmd: PlayerCommand,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) -> bool {
        let mut cancel_stop = false;
        match cmd {
            PlayerCommand::NextUpShow {
                item_id,
                show_title,
                ep_title,
                artist,
            } => {
                log::warn!(target: "player", "next-up: sending script-message mbv-next-up id={item_id} show={show_title} ep={ep_title}");
                let r = mpv.command(
                    "script-message",
                    &["mbv-next-up", &item_id, &show_title, &ep_title, &artist],
                );
                log::warn!(target: "player", "next-up: script-message result={r:?}");
            }
            PlayerCommand::TogglePause => {
                let p = self.status.lock().unwrap().paused;
                let _ = mpv.set_property("pause", !p);
            }
            PlayerCommand::JumpTo(idx) => {
                if let Some(slot_id) = self.slot_id_at(idx) {
                    // mpv playlist indices are adapter coordinates; pin the
                    // target slot identity before asking mpv to move.
                    self.forced_slot_id = Some(slot_id);
                    if let Err(e) = mpv.set_property("playlist-pos", idx as i64) {
                        self.forced_slot_id = None;
                        log::warn!(target: "player", "jump-to idx={idx} failed: {}", mpv_err_str(&e));
                    } else {
                        // Selecting a track should always start it playing, even if
                        // mpv was paused on the previous track — otherwise the new
                        // track loads silently "stuck" paused (see issue: Enter on a
                        // queue item, or a remote Next/Previous command, while paused).
                        let _ = mpv.set_property("pause", false);
                    }
                }
            }
            PlayerCommand::QueueAppend { items } => {
                self.cmd_append_queue(items, mpv);
            }
            PlayerCommand::QueueRemove(idx) => {
                if let Some(slot_id) = self.slot_id_at(idx) {
                    let active_slot_id = self.active_slot_id();
                    let _ = mpv.command("playlist-remove", &[&idx.to_string()]);
                    if active_slot_id == Some(slot_id) {
                        let _ = self.queue.remove_active_slot_confirmed(slot_id);
                    } else {
                        let _ = self.queue.remove_slot(slot_id);
                    }
                    self.refresh_current_idx_from_queue();
                    if self.forced_slot_id == Some(slot_id) {
                        self.forced_slot_id = None;
                    }
                    if active_slot_id == Some(slot_id) {
                        // Currently playing track removed — clear reporter item_id to prevent
                        // stale progress reports until on_end_file transitions to the next track.
                        let mut ids = self.reporter.ids.lock().unwrap();
                        ids.0.clear();
                    }
                }
            }
            PlayerCommand::QueueMove(from, to) => {
                if from < self.queue_len() && to < self.queue_len() && from != to {
                    // mpv's playlist-move index2 names the *pre-move* slot the
                    // entry should end up next to, not its post-move index: for
                    // from < to the entry actually lands at to - 1, not to (mpv
                    // manual's own "paradox" note, confirmed against mpv 0.41).
                    // Passing to + 1 (one past the end when to == n - 1, which
                    // mpv also accepts as "move to end") makes mpv's result
                    // match this struct's from/to bookkeeping below.
                    let mpv_to = if from < to { to + 1 } else { to };
                    let _ = mpv.command("playlist-move", &[&from.to_string(), &mpv_to.to_string()]);
                    if let Some(slot_id) = self.slot_id_at(from) {
                        let had_active_slot = self.active_slot_id().is_some();
                        let _ = self.queue.move_slot(slot_id, to);
                        if had_active_slot {
                            self.refresh_current_idx_from_queue();
                        } else {
                            self.current_idx = shift_index_for_move(self.current_idx, from, to);
                            self.sync_status_position();
                        }
                    }
                }
            }
            PlayerCommand::NextUpDismiss => {
                let _ = mpv.command("script-message", &["mbv-next-up-dismiss"]);
            }
            PlayerCommand::SkipIntroDismiss => {
                let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
            }
            PlayerCommand::ReplaceQueue {
                items: new_items,
                start_idx,
            } => {
                self.cmd_replace_queue(new_items, start_idx, mpv, progress);
                cancel_stop = true;
            }
            PlayerCommand::SetVolume(v) => {
                let vol_max = self.status.lock().unwrap().volume_max;
                let v = v.clamp(0, vol_max);
                let raw = (10.0 * (v as f64).sqrt()).round() as i64;
                let _ = mpv.set_property("volume", raw as f64);
                self.status.lock().unwrap().volume = v;
                let _ = mpv.command("show-text", &[&format!("Volume: {v}%"), "1500"]);
            }
            PlayerCommand::Seek(secs) => {
                let _ = mpv.command("seek", &[&secs.to_string(), "relative"]);
                self.last_seek_at = Some(Instant::now());
            }
            PlayerCommand::SeekAbsolute(secs) => {
                let _ = mpv.command("seek", &[&secs.to_string(), "absolute"]);
                self.last_seek_at = Some(Instant::now());
            }
            PlayerCommand::SetAudio(id) => {
                if id > 0 {
                    let _ = mpv.set_property("aid", id);
                } else {
                    let _ = mpv.set_property("aid", "no".to_string());
                }
                self.status.lock().unwrap().audio_id = id;
                refresh_tracks(mpv, &self.status);
            }
            PlayerCommand::SetSub(id) => {
                if id == 0 {
                    let _ = mpv.set_property("sid", "no".to_string());
                } else {
                    let _ = mpv.set_property("sid", id);
                }
                refresh_tracks(mpv, &self.status);
                self.status.lock().unwrap().sub_id = id;
            }
            PlayerCommand::SetSubtitlePrefs {
                mode,
                subtitle_lang,
                audio_lang,
            } => {
                {
                    let mut p = self.subtitle_prefs.lock().unwrap();
                    p.mode = mode;
                    p.subtitle_lang = subtitle_lang;
                    p.audio_lang = audio_lang;
                }
                let prefs = self.subtitle_prefs.lock().unwrap().clone();
                auto_select_tracks(mpv, &self.status, &prefs);
            }
            PlayerCommand::SetMute(m) => {
                let _ = mpv.set_property("mute", m);
                self.status.lock().unwrap().muted = m;
            }
            PlayerCommand::LoadNew {
                url,
                start_pos,
                item,
            } => {
                self.cmd_load_new(url, start_pos, item, mpv, progress);
                cancel_stop = true;
            }
        }
        cancel_stop
    }

    fn cmd_replace_queue(
        &mut self,
        new_items: Vec<MediaItem>,
        start_idx: usize,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        self.cancel_pending_quit();
        if new_items.is_empty() {
            self.stop_report_accepted = self.reporter.report_stopped(self.last_valid_pos);
            self.stop_reported = true;
            let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
            let _ = mpv.command("playlist-clear", &[]);
            self.origin = PlaybackOrigin::Queue;
            self.set_origin(self.origin);
            self.queue = PlaybackQueue::default();
            self.current_idx = 0;
            self.sync_status_position();
            self.last_valid_pos = 0;
            self.pending_initial_jump = false;
            self.pending_load = 0;
            self.tracks_initialized = false;
            self.forced_slot_id = None;
            self.reset_next_up_state();
            self.stopped_event_sent = false;
            self.mark_played_id = None;
            self.stopped_near_end = false;
            self.osd_title.clear();
            self.pending_resume_secs = None;
            self.series_id.clear();
            self.season = 0;
            self.episode = 0;
            return;
        }
        // report_stopped for current item; is_audio zeroing handled inside.
        self.stop_report_accepted = self.reporter.report_stopped(self.last_valid_pos);
        self.stop_reported = true;
        // Replacing the playlist should always start playing it, even if mpv
        // was left paused on the previous item (reused-window fast path).
        let _ = mpv.set_property("pause", false);

        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
        // Remove all old playlist entries except the current one so that
        // the subsequent loadfile "replace" starts from a clean slate.
        // Without this, old entries remain and playlist-pos = start_idx
        // lands on a stale file instead of new_items[start_idx].
        let _ = mpv.command("playlist-clear", &[]);

        let start_idx = start_idx.min(new_items.len() - 1);
        for (i, item) in new_items.iter().enumerate() {
            let ep = if item.is_audio() { "Audio" } else { "Videos" };
            let url = format!(
                "{}/{}/{}/stream?static=true&api_key={}",
                self.server_url, ep, item.id, self.token
            );
            let mode = if i == 0 { "replace" } else { "append-play" };
            let title_opt = mpv_title_opt(&item.display_name());
            if let Err(e) = mpv.command("loadfile", &[url.as_str(), mode, "-1", title_opt.as_str()])
            {
                log::warn!(target: "player", "ReplaceQueue loadfile error: {}", mpv_err_str(&e));
            }
        }
        let active_item = new_items[start_idx].clone();
        let _ = mpv.set_property("start", "0");
        send_ep_info(mpv, &active_item);
        // loadfile "replace" displaces the current file (EndFile #1).
        // If start_idx > 0 we also set playlist-pos which displaces item[0] (EndFile #2).
        // Use = not += so a stale pending_load from a prior operation never stacks.
        // Clear pending_initial_jump too since any in-flight initial jump is superseded.
        self.pending_initial_jump = false;
        self.pending_load = if start_idx > 0 { 2 } else { 1 };
        if start_idx > 0 {
            let _ = mpv.set_property("playlist-pos", start_idx as i64);
        }

        self.origin = PlaybackOrigin::Queue;
        self.set_origin(self.origin);
        self.queue = PlaybackQueue::from_items(new_items, Some(start_idx));
        self.current_idx = start_idx;
        self.load_active_item_state();
        self.tracks_initialized = false;
        // stop_reported stays true until pending_load drains to 0 in on_end_file,
        // preventing a duplicate report_stopped for the displaced file's EndFile(Quit).
        self.forced_slot_id = None;
        self.reset_next_up_state();
        self.stopped_event_sent = false;
        self.mark_played_id = None;
        self.stopped_near_end = false;
        log::info!(target: "player", "playlist queue-replace idx={start_idx} pending_resume={:?}s", self.pending_resume_secs);
        {
            let mut s = self.status.lock().unwrap();
            s.position_ticks = active_item.playback_position_ticks;
            s.runtime_ticks = active_item.runtime_ticks;
            s.current_idx = self.current_idx;
            s.queue_len = self.queue_len();
            s.set_current_item_metadata(&active_item);
        }

        // Stop progress reporter during transition to prevent stale reports,
        // then restart for the new item.
        progress.stop_and_join(self.progress_join_budget());
        let (urls, ok) = self.reporter.start_item(&active_item);
        self.ext_sub_urls = urls;
        if !ok {
            log::warn!(target: "player", "start_item failed for playlist replace item={}", active_item.id);
        }
        *progress = spawn_progress_reporter(self.reporter.clone());
    }

    fn append_items_to_queue(&mut self, items: Vec<MediaItem>) {
        for item in items {
            self.queue.append(item);
        }
        self.status.lock().unwrap().queue_len = self.queue_len();
    }

    fn cmd_append_queue(&mut self, new_items: Vec<MediaItem>, mpv: &Mpv) {
        if new_items.is_empty() {
            return;
        }

        for item in &new_items {
            let ep = if item.is_audio() { "Audio" } else { "Videos" };
            let url = format!(
                "{}/{}/{}/stream?static=true&api_key={}",
                self.server_url, ep, item.id, self.token
            );
            let title_opt = mpv_title_opt(&item.display_name());
            if let Err(e) = mpv.command(
                "loadfile",
                &[url.as_str(), "append-play", "-1", title_opt.as_str()],
            ) {
                log::warn!(target: "player", "QueueAppend loadfile error: {}", mpv_err_str(&e));
            }
        }

        self.append_items_to_queue(new_items);
    }

    fn cmd_load_new(
        &mut self,
        url: String,
        start_pos: f64,
        item: Box<MediaItem>,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) {
        self.cancel_pending_quit();
        self.origin = PlaybackOrigin::Standalone;
        self.set_origin(self.origin);
        // Loading a new item should always start playing it, even if mpv
        // was left paused on the previous item (reused-window fast path).
        let _ = mpv.set_property("pause", false);

        // Stop progress reporter during transition to prevent stale reports.
        progress.stop_and_join(self.progress_join_budget());
        self.ext_sub_urls = self.reporter.transition_to(&item, self.last_valid_pos);
        *progress = spawn_progress_reporter(self.reporter.clone());

        self.queue = PlaybackQueue::from_items(vec![item.as_ref().clone()], Some(0));
        self.current_idx = 0;
        self.load_active_item_state();
        self.tracks_initialized = false;
        self.stop_reported = false;
        self.stop_report_accepted = false;
        self.pending_load = 1;
        self.pending_initial_jump = false;
        self.forced_slot_id = None;
        self.reset_next_up_state();
        self.stopped_event_sent = false;
        self.mark_played_id = None;
        self.stopped_near_end = false;
        {
            let mut st = self.status.lock().unwrap();
            st.runtime_ticks = item.runtime_ticks;
            st.position_ticks = item.playback_position_ticks;
            st.current_idx = 0;
            st.queue_len = 1;
            st.set_current_item_metadata(&item);
        }

        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
        let _ = mpv.command("script-message", &["mbv-next-up-dismiss"]);

        if start_pos > 0.0 {
            let _ = mpv.set_property("start", format!("{start_pos:.0}"));
        } else {
            let _ = mpv.set_property("start", "0");
        }
        let title_opt = mpv_title_opt(&item.display_name());
        log::info!(target: "player", "loadfile url={url} opts={title_opt:?}");
        if let Err(e) = mpv.command(
            "loadfile",
            &[url.as_str(), "replace", "-1", title_opt.as_str()],
        ) {
            log::warn!(target: "player", "loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e));
        }
        send_ep_info(mpv, &item);
    }

    fn on_time_pos(&mut self, pos_secs: f64, mpv: &Mpv) {
        let ticks = (pos_secs * TICKS_PER_SECOND as f64) as i64;
        {
            let mut st = self.status.lock().unwrap();
            st.position_ticks = ticks;
            // Don't update last_valid_pos while a resume seek is pending: mpv fires
            // time-pos=0 before the seek lands, which would overwrite the correct position.
            if pos_secs > 0.0 && self.pending_resume_secs.is_none() {
                if self.last_valid_pos == 0 {
                    log::info!(target: "player", "playlist last_valid_pos first non-zero: {}s idx={}", ticks / TICKS_PER_SECOND, self.current_idx);
                }
                self.last_valid_pos = ticks;
                st.last_valid_pos = ticks;
            }
        }

        if self.origin == PlaybackOrigin::Queue {
            // Playlist next-up: match Emby Web's timing from videoosd.js.
            // 60 s before end. Minimum episode: 10 min. Minimum remaining when shown: 20 s.
            const MIN_RUNTIME_TICKS: i64 = 600 * TICKS_PER_SECOND;
            const MIN_REMAIN_TICKS: i64 = 20 * TICKS_PER_SECOND;
            if self.current_idx + 1 < self.queue_len() {
                let runtime = self.status.lock().unwrap().runtime_ticks;
                if runtime > 0 {
                    let show_at = runtime - 60 * TICKS_PER_SECOND;
                    let remaining = runtime - ticks;
                    if self.queue_next_up_fired && ticks < show_at {
                        self.queue_next_up_fired = false;
                        self.queue_next_up_armed = false;
                    }
                    if !self.queue_next_up_fired && runtime >= MIN_RUNTIME_TICKS {
                        if remaining >= MIN_REMAIN_TICKS && ticks >= show_at {
                            self.queue_next_up_fired = true;
                            let _ = self.event_tx.send(PlayerEvent::QueueNextUp {
                                next_idx: self.current_idx + 1,
                            });
                        } else if !self.queue_next_up_armed
                            && ticks > 0
                            && ticks < TICKS_PER_SECOND * 5
                        {
                            self.queue_next_up_armed = true;
                            log::info!(target: "player", "queue next-up armed idx={}", self.current_idx + 1);
                        }
                    }
                }
            }
        } else if !self.next_up_fired {
            const NEXT_UP_TICKS: i64 = 60 * TICKS_PER_SECOND;
            if self.series_id.is_empty() {
                if !self.next_up_armed && ticks > 0 && ticks < TICKS_PER_SECOND * 5 {
                    self.next_up_armed = true;
                    log::warn!(target: "player", "next-up disabled: no series_id (Episode item without SeriesId in fetch)");
                }
            } else {
                let runtime = self.status.lock().unwrap().runtime_ticks;
                if runtime > NEXT_UP_TICKS && ticks > runtime - NEXT_UP_TICKS {
                    self.next_up_fired = true;
                    log::warn!(target: "player", "next-up: threshold reached series={}", self.series_id);
                    let _ = self.event_tx.send(PlayerEvent::NextUpThreshold {
                        series_id: self.series_id.clone(),
                        season: self.season,
                        episode: self.episode,
                    });
                } else if !self.next_up_armed && ticks > 0 && ticks < TICKS_PER_SECOND * 5 {
                    self.next_up_armed = true;
                    log::info!(target: "player", "next-up: armed series={} runtime={}s", self.series_id, runtime / TICKS_PER_SECOND);
                }
            }
        }

        handle_intro(
            ticks,
            self.intro_start,
            self.intro_end,
            &mut self.intro_show,
            &mut self.intro_hide,
            self.config.always_skip_intro,
            mpv,
            &self.event_tx,
        );
    }

    fn on_playlist_pos_changed(&mut self, pos: i64) {
        if pos < 0 {
            return;
        }
        let pos = pos as usize;
        if self.pending_initial_jump || self.pending_load > 0 || self.forced_slot_id.is_some() {
            log::debug!(
                target: "player",
                "ignoring transient playlist-pos={pos} while queue transition is pending"
            );
            return;
        }
        if pos >= self.queue_len() {
            log::warn!(
                target: "player",
                "ignoring out-of-range playlist-pos={pos} for queue len {}",
                self.queue_len()
            );
            return;
        }
        let _ = self.set_active_index(pos);
    }

    fn on_playlist_count_changed(&mut self, count: usize) {
        if count == self.queue_len() {
            return;
        }
        let old_n = self.queue_len();
        if count < old_n {
            let removed = old_n - count;
            log::warn!(target: "player", "playlist-count dropped from {} to {}: {} item(s) removed externally", old_n, count, removed);
            let removed_slot_ids: Vec<_> = self
                .queue
                .slots()
                .iter()
                .skip(count)
                .map(|slot| slot.slot_id)
                .collect();
            for slot_id in removed_slot_ids {
                if self.active_slot_id() == Some(slot_id) {
                    let _ = self.queue.remove_active_slot_confirmed(slot_id);
                } else {
                    let _ = self.queue.remove_slot(slot_id);
                }
            }
            self.refresh_current_idx_from_queue();
            let _ = self.event_tx.send(PlayerEvent::QueueDesynced(format!(
                "Queue desynced: {removed} item(s) removed externally"
            )));
        } else {
            let added = count - old_n;
            log::warn!(target: "player", "playlist-count increased from {} to {}: {} item(s) added externally", old_n, count, added);
            // We cannot reconstruct the added MediaItems from mpv's playlist,
            // so we keep the queue as-is. Clamp current_idx to the last
            // known item in case the external tool also changed position.
            if self.current_idx >= self.queue_len() && self.queue_len() > 0 {
                self.current_idx = self.queue_len() - 1;
            }
            self.sync_status_position();
            let _ = self.event_tx.send(PlayerEvent::QueueDesynced(format!(
                "Queue desynced: {added} item(s) added externally"
            )));
        }
    }

    fn on_playback_restart(&mut self, mpv: &Mpv) {
        {
            let h: i64 = mpv.get_property("video-params/h").unwrap_or(0);
            let is_img: bool = mpv
                .get_property("current-tracks/video/image")
                .unwrap_or(false);
            let codec: String = mpv.get_property("audio-codec-name").unwrap_or_default();
            let mut st = self.status.lock().unwrap();
            st.video_height = h;
            st.audio_codec = codec.to_lowercase();
            st.video_is_image = is_img;
        }
        if self.pending_initial_jump {
            // mpv ignored playlist-pos before the event loop started; now that
            // playback is live (first PlaybackRestart), the jump is honored.
            self.pending_initial_jump = false;
            self.pending_load += 1;
            let _ = mpv.set_property("playlist-pos", self.current_idx as i64);
            // Skip normal handling; wait for the next PlaybackRestart (for start_idx item).
            return;
        }
        if self.startup_pause_release_pending {
            self.startup_pause_release_pending = false;
            log::info!(
                target: "player",
                "audio pipe: startup gate cleared on PlaybackRestart (playlist)"
            );
            let _ = mpv.set_property("pause", false);
        }
        let mut event_name = "TimeUpdate";
        if !self.tracks_initialized {
            let prefs = self.subtitle_prefs.lock().unwrap().clone();
            for url in &self.ext_sub_urls {
                if let Err(e) = mpv.command("sub-add", &[url.as_str()]) {
                    log::warn!(target: "player", "sub-add failed: {url}: {e:?}");
                }
            }
            auto_select_tracks(mpv, &self.status, &prefs);
            self.tracks_initialized = true;
            if let Some(item) = self.active_item().cloned() {
                send_ep_info(mpv, &item);
            }
            if let Some(secs) = self.pending_resume_secs.take() {
                log::info!(target: "player", "playlist pending_resume cleared: seeking to {secs:.0}s idx={}", self.current_idx);
                let _ = mpv.command("seek", &[&format!("{secs:.0}"), "absolute"]);
                self.last_seek_at = Some(Instant::now());
            } else {
                log::info!(target: "player", "playlist pending_resume cleared: no resume (starting from 0) idx={}", self.current_idx);
            }
            if self.config.use_mpv_config {
                let _ = mpv.command("show-text", &[&self.osd_title, "3000"]);
            }
        } else {
            if self.origin == PlaybackOrigin::Standalone {
                self.next_up_fired = false;
                self.next_up_armed = false;
                event_name = "Seek";
            }
            if self.last_seek_at.take().is_some() && self.config.use_mpv_config {
                let _ = mpv.command("show-text", &[&self.osd_title, "2000"]);
            }
        }
        let seek_settled = self
            .last_seek_at
            .is_none_or(|t| t.elapsed() > Duration::from_millis(500));
        if self.quit_at.is_none() && seek_settled {
            self.last_seek_at = None;
            if self.origin == PlaybackOrigin::Standalone {
                self.reporter.report_progress(event_name);
            } else if !self.reporter.is_audio.load(Ordering::Relaxed) {
                self.reporter.report_progress("TimeUpdate");
            }
        }
    }

    // Returns true if the event loop should `continue`.
    fn on_end_file(
        &mut self,
        reason: EndFileReason,
        mpv: &Mpv,
        progress: &mut ProgressGuard,
    ) -> bool {
        if self.quit_at.is_some() {
            return true;
        }
        if self.pending_load > 0 {
            self.pending_load -= 1;
            // Once all pending EndFiles from a ReplaceQueue are drained, the new item's
            // lifecycle begins — reset stop_reported so on_end_file/on_shutdown can report it.
            if self.pending_load == 0 {
                self.stop_reported = false;
                self.stop_report_accepted = false;
            }
            return true;
        }

        if reason == mpv_end_file_reason::Error {
            log::warn!(target: "player", "EndFile: playback error (file may be unreadable or format unsupported)");
        }

        let completed_is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
        let runtime = self.status.lock().unwrap().runtime_ticks;

        if self.origin == PlaybackOrigin::Queue && reason == mpv_end_file_reason::Quit {
            let natural_end = reason == mpv_end_file_reason::Eof && runtime > 0;
            let near_end = !natural_end
                && !completed_is_audio
                && runtime > 0
                && self.last_valid_pos * 20 / runtime >= 19;
            log::warn!(target: "player", "quit path: last_valid_pos={} runtime={} pending_resume={} stop_reported={}",
                self.last_valid_pos, runtime, self.pending_resume_secs.is_some(), self.stop_reported);
            if !self.stop_reported {
                progress.stop_and_join(self.progress_join_budget());
                self.stop_report_accepted = self.report_stopped_for_end_file(reason);
                self.stop_reported = true;
            }
            if (natural_end || near_end) && !completed_is_audio {
                let id = self.reporter.ids.lock().unwrap().0.clone();
                if let Err(e) = self.reporter.client.mark_played(&id) {
                    log::warn!(target: "player", "mark_played failed id={id}: {e}; scheduling retry");
                    retry_mark_played(self.reporter.client.clone(), id);
                }
            }
            self.stopped_near_end = near_end;
            return true; // wait for Shutdown to fire PlayerEvent::Stopped
        }

        if self.origin == PlaybackOrigin::Standalone {
            let natural_end = reason == mpv_end_file_reason::Eof && runtime > 0;

            progress.stop_and_join(self.progress_join_budget());
            self.stop_report_accepted = self.report_stopped_for_end_file(reason);
            self.stop_reported = true;

            if natural_end {
                let id = self.reporter.ids.lock().unwrap().0.clone();
                if !completed_is_audio {
                    match self.reporter.client.mark_played(&id) {
                        Ok(()) => log::info!(target: "player", "mark_played ok id={id}"),
                        Err(e) => {
                            log::warn!(target: "player", "mark_played failed id={id}: {e}; will retry");
                            self.mark_played_id = Some(id.clone());
                        }
                    }
                }
                let _ = self.event_tx.send(PlayerEvent::Stopped {
                    idx: 0,
                    position_ticks: 0,
                    played: !completed_is_audio,
                    consume: false,
                    progress_report_accepted: self.stop_report_accepted,
                    error: None,
                });
                self.stopped_event_sent = true;
            }
            return false;
        }

        let completed_idx = self.current_idx;
        log::warn!(target: "player", "advance path: reason={reason:?} last_valid_pos={} runtime={} pending_resume={}",
            self.last_valid_pos, self.status.lock().unwrap().runtime_ticks, self.pending_resume_secs.is_some());
        // H11: bounds-check completed_idx — QueueRemove can shrink the list
        // while the current track is finishing.
        let Some(completed_item) = self.item_at(completed_idx).cloned() else {
            log::warn!(target: "player", "on_end_file: completed_idx={completed_idx} out of bounds (len={}), stopping",
                self.queue_len());
            progress.stop_and_join(self.progress_join_budget());
            self.status.lock().unwrap().active = false;
            self.stop_report_accepted = self.reporter.report_stopped(self.last_valid_pos);
            let _ = self.event_tx.send(PlayerEvent::Stopped {
                idx: completed_idx.min(self.queue_len().saturating_sub(1)),
                position_ticks: self.last_valid_pos,
                played: false,
                consume: false,
                progress_report_accepted: self.stop_report_accepted,
                error: None,
            });
            return false;
        };
        let natural = reason == mpv_end_file_reason::Eof && completed_item.runtime_ticks > 0;
        let near_end = is_near_end(
            completed_is_audio,
            natural,
            self.last_valid_pos,
            completed_item.runtime_ticks,
        );
        let was_next_up = std::mem::replace(&mut self.next_up_jump, false);
        let track_finished = natural || near_end || was_next_up;
        // played_out drives mark-played/Emby watched-status and stays video-only;
        // consume_track drives queue auto-removal and is type-agnostic — the app layer
        // gates it per-type against consume_videos/consume_audio.
        let played_out = track_finished && !completed_is_audio;
        let consume_track = track_finished;
        log::info!(target: "consume", "on_end_file decision: idx={completed_idx} reason={reason:?} \
            natural={natural} near_end={near_end} was_next_up={was_next_up} \
            completed_is_audio={completed_is_audio} last_valid_pos={} runtime={} \
            => played_out={played_out} consume_track={consume_track}",
            self.last_valid_pos, completed_item.runtime_ticks);
        let completed_pos =
            queue_completed_pos(completed_is_audio, natural, near_end, self.last_valid_pos);

        let next_idx = self
            .forced_slot_id
            .take()
            .and_then(|slot_id| self.queue.slot_index(slot_id))
            .unwrap_or(self.current_idx + 1);

        if next_idx >= self.queue_len() {
            progress.stop_and_join(self.progress_join_budget());
            self.status.lock().unwrap().active = false;
            self.stop_report_accepted = self.reporter.report_stopped(completed_pos);
            if played_out {
                let id = completed_item.id.clone();
                if let Err(e) = self.reporter.client.mark_played(&id) {
                    log::warn!(target: "player", "mark_played failed id={id}: {e}; scheduling retry");
                    retry_mark_played(self.reporter.client.clone(), id);
                }
            }
            let _ = self.event_tx.send(PlayerEvent::Stopped {
                idx: completed_idx,
                position_ticks: completed_pos,
                played: played_out,
                consume: consume_track,
                progress_report_accepted: self.stop_report_accepted,
                error: None,
            });
            return false; // signals run() to return
        }

        // Update UI to the next track immediately, before slow network calls.
        // next_idx < queue_len() was already checked above, so set_active_index
        // (which only fails when the index is out of bounds) cannot fail here.
        let advanced = self.set_active_index(next_idx);
        debug_assert!(
            advanced,
            "set_active_index({next_idx}) must succeed: already bounds-checked against queue_len={}",
            self.queue_len()
        );
        let next_item = self
            .active_item()
            .cloned()
            .expect("active item must exist after successful set_active_index");
        self.load_active_item_state();
        self.tracks_initialized = false;
        {
            let mut s = self.status.lock().unwrap();
            s.position_ticks = 0;
            s.runtime_ticks = next_item.runtime_ticks;
            s.current_idx = self.current_idx;
            s.queue_len = self.queue_len();
            s.set_current_item_metadata(&next_item);
        }

        let stop_report_accepted = self.reporter.report_stopped(completed_pos);
        if played_out {
            let id = completed_item.id.clone();
            if let Err(e) = self.reporter.client.mark_played(&id) {
                log::warn!(target: "player", "mark_played failed id={id}: {e}; scheduling retry");
                retry_mark_played(self.reporter.client.clone(), id);
            }
        }

        let _ = mpv.set_property("start", "0");
        self.queue_next_up_fired = false;
        self.queue_next_up_armed = false;
        send_ep_info(mpv, &next_item);
        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);

        // Stop progress reporter during transition to prevent stale reports.
        progress.stop_and_join(self.progress_join_budget());
        let (urls, ok) = self.reporter.start_item(&next_item);
        self.ext_sub_urls = urls;
        if !ok {
            log::warn!(target: "player", "start_item failed for playlist track-transition item={}", next_item.id);
        }
        *progress = spawn_progress_reporter(self.reporter.clone());

        log::info!(target: "player", "playlist track-transition idx={} pending_resume={:?}s", self.current_idx, self.pending_resume_secs);

        let _ = self.event_tx.send(PlayerEvent::TrackCompleted {
            idx: completed_idx,
            position_ticks: completed_pos,
            played: played_out,
            consume: consume_track,
            progress_report_accepted: stop_report_accepted,
        });
        let _ = self
            .event_tx
            .send(PlayerEvent::TrackChanged(self.current_idx));
        false
    }

    fn on_shutdown(&mut self, progress: &mut ProgressGuard) {
        log::warn!(target: "player", "shutdown: last_valid_pos={} stop_reported={} pending_resume={}",
            self.last_valid_pos, self.stop_reported, self.pending_resume_secs.is_some());
        if !self.stop_reported {
            progress.stop_and_join(self.progress_join_budget());
            self.stop_report_accepted = self.report_stopped_for_current_context();
            self.stop_reported = true;
        }
        let client = self.reporter.client.clone();
        if self.origin == PlaybackOrigin::Standalone {
            // Retry mark_played in a detached thread so Shutdown never blocks.
            if let Some(mid) = self.mark_played_id.take() {
                retry_mark_played(client.clone(), mid);
            }
            let runtime = self.status.lock().unwrap().runtime_ticks;
            let is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
            let near_end = !is_audio && runtime > 0 && self.last_valid_pos * 20 / runtime >= 19;
            if near_end {
                let id = self.reporter.ids.lock().unwrap().0.clone();
                retry_mark_played(client.clone(), id);
            }
            self.status.lock().unwrap().active = false;
            if !self.stopped_event_sent {
                let _ = self.event_tx.send(PlayerEvent::Stopped {
                    idx: 0,
                    position_ticks: self.last_valid_pos,
                    played: near_end,
                    consume: false,
                    progress_report_accepted: self.stop_report_accepted,
                    error: None,
                });
            }
            // mpv exited on its own (not via our stop command, e.g. the user
            // closed the mpv window directly) — despite the event name,
            // App::handle_player_event's PlayerEvent::MpvQuit arm does not
            // quit the app; it just clears some UI state and returns false.
            if self.quit_at.is_none() {
                let _ = self.event_tx.send(PlayerEvent::MpvQuit);
            }
            return;
        }
        self.status.lock().unwrap().active = false;
        // played and consume are deliberately the same value here: stopped_near_end
        // is already video-only (see is_near_end's !is_audio gate), so a quit/cancel
        // near the end of an audio item never sets either — consistent with on_end_file's
        // normal advance path, where only natural/next-up (not near-end) triggers audio consume.
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            idx: self.current_idx,
            position_ticks: self.last_valid_pos,
            played: self.stopped_near_end,
            consume: self.stopped_near_end,
            progress_report_accepted: self.stop_report_accepted,
            error: None,
        });
        // mpv exited on its own (not via our stop command) — tell the app to quit.
        if self.quit_at.is_none() {
            let _ = self.event_tx.send(PlayerEvent::MpvQuit);
        }
    }

    fn run(
        mut self,
        mpv: Mpv,
        stop_rx: mpsc::Receiver<()>,
        cmd_rx: mpsc::Receiver<PlayerCommand>,
        mut progress: ProgressGuard,
    ) {
        let event_tx_panic = self.event_tx.clone();
        let current_idx_panic = self.current_idx;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            loop {
                let mut cancel_stop = false;
                while let Ok(cmd) = cmd_rx.try_recv() {
                    cancel_stop |= self.handle_command(cmd, &mpv, &mut progress);
                }

                if !cancel_stop && self.quit_at.is_none() && stop_rx.try_recv().is_ok() {
                    let _ = mpv.command("quit", &[]);
                    self.quit_at = Some(Instant::now());
                }

                if self
                    .quit_at
                    .is_some_and(|t| t.elapsed() > Duration::from_secs(2))
                {
                    if !self.stop_reported {
                        progress.stop_and_join(self.progress_join_budget());
                        self.stop_report_accepted = self.report_stopped_for_current_context();
                        self.stop_reported = true;
                    }
                    let runtime = self.status.lock().unwrap().runtime_ticks;
                    let is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
                    let (played, consume) = quit_timeout_stop_flags(
                        self.origin,
                        is_audio,
                        self.last_valid_pos,
                        runtime,
                        self.stopped_near_end,
                    );
                    self.status.lock().unwrap().active = false;
                    let _ = self.event_tx.send(PlayerEvent::Stopped {
                        idx: self.current_idx,
                        position_ticks: self.last_valid_pos,
                        played,
                        consume,
                        progress_report_accepted: self.stop_report_accepted,
                        error: None,
                    });
                    return;
                }

                match mpv.wait_event(0.5) {
                    Some(Ok(Event::PropertyChange {
                        name: "volume",
                        change: PropertyData::Double(vol),
                        ..
                    })) => {
                        self.status.lock().unwrap().volume = (vol * vol / 100.0) as i64;
                    }
                    Some(Ok(Event::PropertyChange {
                        change: PropertyData::Double(pos_secs),
                        ..
                    })) => {
                        self.on_time_pos(pos_secs, &mpv);
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "pause",
                        change: PropertyData::Flag(paused),
                        ..
                    })) => {
                        self.status.lock().unwrap().paused = paused;
                        if self.startup_pause_events_to_skip > 0 {
                            self.startup_pause_events_to_skip -= 1;
                            continue;
                        }
                        if self.quit_at.is_none() {
                            let event_name = if paused { "Pause" } else { "Unpause" };
                            self.reporter.report_progress(event_name);
                        }
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "sid",
                        change: PropertyData::Str(s),
                        ..
                    })) => {
                        let id = s.parse::<i64>().unwrap_or(0);
                        log::info!(target: "player", "sid PropertyChange: raw={s:?} parsed={id}");
                        self.status.lock().unwrap().sub_id = id;
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "aid",
                        change: PropertyData::Str(_),
                        ..
                    })) => {
                        refresh_tracks(&mpv, &self.status);
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "mute",
                        change: PropertyData::Flag(m),
                        ..
                    })) => {
                        self.status.lock().unwrap().muted = m;
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "video-params/h",
                        change: PropertyData::Int64(h),
                        ..
                    })) => {
                        log::info!(target: "player", "video-params/h (playlist): h={h}");
                        self.status.lock().unwrap().video_height = h;
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "video-params/h",
                        change,
                        ..
                    })) => {
                        log::warn!(target: "player", "video-params/h (playlist) unexpected type: {:?}", change);
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "audio-codec-name",
                        change: PropertyData::Str(s),
                        ..
                    })) => {
                        self.status.lock().unwrap().audio_codec = s.to_lowercase();
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "current-tracks/video/image",
                        change: PropertyData::Flag(is_img),
                        ..
                    })) => {
                        log::info!(target: "player", "video/image (playlist): is_img={is_img}");
                        self.status.lock().unwrap().video_is_image = is_img;
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "playlist-pos",
                        change: PropertyData::Int64(pos),
                        ..
                    })) => {
                        self.on_playlist_pos_changed(pos);
                    }
                    Some(Ok(Event::PropertyChange {
                        name: "playlist-count",
                        change: PropertyData::Int64(count),
                        ..
                    })) => {
                        if self.pending_load == 0 {
                            self.on_playlist_count_changed(count as usize);
                        }
                    }
                    Some(Ok(Event::PlaybackRestart)) => {
                        self.on_playback_restart(&mpv);
                    }
                    Some(Ok(Event::EndFile(reason))) => {
                        let should_continue = self.on_end_file(reason, &mpv, &mut progress);
                        // on_end_file returns false both for "continue normally" and for
                        // "end of playlist — return from thread". Detect end-of-playlist
                        // by checking active flag which on_end_file sets to false.
                        if !should_continue && !self.status.lock().unwrap().active {
                            return;
                        }
                        if should_continue {
                            continue;
                        }
                    }
                    Some(Ok(Event::LogMessage {
                        prefix,
                        level,
                        text,
                        ..
                    })) => {
                        let t = text.trim_end();
                        if !t.is_empty() {
                            log::warn!(target: "mpv", "[{}/{}] {}", prefix, level, t);
                        }
                    }
                    Some(Ok(Event::ClientMessage(args)))
                        if args.first().copied() == Some("mbv-next-up-play") =>
                    {
                        log::info!(target: "player", "next-up: mbv-next-up-play received from Lua");
                        self.next_up_jump = true;
                        let _ = self.event_tx.send(PlayerEvent::NextUpPlay);
                    }
                    Some(Ok(Event::ClientMessage(args)))
                        if args.first().copied() == Some("mbv-skip-intro-play") =>
                    {
                        let _ = self.event_tx.send(PlayerEvent::SkipIntroPlay);
                    }
                    Some(Ok(Event::ClientMessage(args)))
                        if self.config.use_mpv_config
                            && args.first().copied() == Some("mouse-moved") =>
                    {
                        let show = self
                            .last_mouse_osd
                            .is_none_or(|t: Instant| t.elapsed() > Duration::from_secs(3));
                        if show {
                            let _ = mpv.command("show-text", &[&self.osd_title, "2000"]);
                            self.last_mouse_osd = Some(Instant::now());
                        }
                    }
                    Some(Ok(Event::Shutdown)) => {
                        self.on_shutdown(&mut progress);
                        return;
                    }
                    Some(Err(e)) => {
                        log::warn!(target: "player", "event error: {}", mpv_err_str(&e));
                    }
                    _ => {}
                }
            }
        })); // end catch_unwind
        if let Err(panic) = result {
            let msg = panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            log::error!(target: "player", "PlaybackSession panicked: {msg}");
            let _ = event_tx_panic.send(PlayerEvent::Stopped {
                idx: current_idx_panic,
                position_ticks: 0,
                played: false,
                consume: false,
                progress_report_accepted: false,
                error: Some(msg),
            });
        }
    }
}

// ── QuitHandle ───────────────────────────────────────────────────────────────

/// Clonable handle to stop the local player from any thread. Calling stop()
/// sends mpv a quit command (closes the window immediately); the player thread
/// then reports stopped to Emby before exiting.
#[derive(Clone)]
pub struct QuitHandle {
    stop_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
}

impl QuitHandle {
    pub fn stop(&self) {
        if let Some(tx) = self.stop_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }

    pub fn stop_for_shutdown(&self, timeout: Duration) {
        *self.shutdown_report_timeout.lock().unwrap() = Some(timeout);
        self.stop();
    }
}

// ── Player ────────────────────────────────────────────────────────────────────

pub struct Player {
    server_url: String,
    token: String,
    show_audio_window: bool,
    use_mpv_config: bool,
    no_scripts: bool,
    #[allow(dead_code)]
    pub always_play_next: bool,
    pub always_skip_intro: bool,
    pub subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    is_queue_mode: Arc<AtomicBool>,
    current_is_headless: Arc<AtomicBool>,
    pub event_tx: mpsc::Sender<PlayerEvent>,
    stop_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    pub cmd_tx: Arc<Mutex<Option<mpsc::Sender<PlayerCommand>>>>,
    pub status: Arc<Mutex<PlayerStatus>>,
    thread_handle: Mutex<Option<thread::JoinHandle<()>>>,
    ws_tx: Option<crate::ws::WsSender>,
}

impl Player {
    pub fn new(
        server_url: String,
        token: String,
        show_audio_window: bool,
        use_mpv_config: bool,
        no_scripts: bool,
        always_play_next: bool,
        always_skip_intro: bool,
        subtitle_prefs: SubtitlePrefs,
        event_tx: mpsc::Sender<PlayerEvent>,
        ws_tx: Option<crate::ws::WsSender>,
    ) -> Self {
        Player {
            server_url,
            token,
            show_audio_window,
            use_mpv_config,
            no_scripts,
            always_play_next,
            always_skip_intro,
            subtitle_prefs: Arc::new(Mutex::new(subtitle_prefs)),
            is_queue_mode: Arc::new(AtomicBool::new(false)),
            current_is_headless: Arc::new(AtomicBool::new(false)),
            event_tx,
            stop_tx: Arc::new(Mutex::new(None)),
            shutdown_report_timeout: Arc::new(Mutex::new(None)),
            cmd_tx: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(PlayerStatus::default())),
            thread_handle: Mutex::new(None),
            ws_tx,
        }
    }

    pub fn join(&self) {
        let handle = self.thread_handle.lock().unwrap().take();
        if let Some(h) = handle {
            let _ = h.join();
        }
    }

    // Join the player thread but give up after `timeout`. Used on SIGHUP/SIGTERM
    // so the process always exits even if an HTTP call is hanging.
    pub fn join_or_timeout(&self, timeout: std::time::Duration) {
        let handle = self.thread_handle.lock().unwrap().take();
        if let Some(h) = handle {
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            std::thread::spawn(move || {
                let _ = h.join();
                let _ = tx.send(());
            });
            let _ = rx.recv_timeout(timeout);
        }
    }

    /// Returns `true` if the command was sent, `false` if the player thread is gone.
    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        if let Some(tx) = self.cmd_tx.lock().unwrap().as_ref() {
            tx.send(cmd).is_ok()
        } else {
            false
        }
    }

    #[cfg(test)]
    pub(crate) fn spy_on_commands(&self) -> mpsc::Receiver<PlayerCommand> {
        let (tx, rx) = mpsc::channel();
        *self.cmd_tx.lock().unwrap() = Some(tx);
        rx
    }

    pub fn next(&self) -> bool {
        match self.status.lock().unwrap().next_idx() {
            Some(idx) => self.send_command(PlayerCommand::JumpTo(idx)),
            None => false,
        }
    }

    pub fn previous(&self) -> bool {
        match self.status.lock().unwrap().previous_idx() {
            Some(idx) => self.send_command(PlayerCommand::JumpTo(idx)),
            None => false,
        }
    }

    pub fn set_paused(&self, paused: bool) -> bool {
        match self.status.lock().unwrap().toggle_to_reach(paused) {
            Some(cmd) => self.send_command(cmd),
            None => false,
        }
    }

    /// Seed queue/status state without starting playback. Used when a freshly
    /// spawned local daemon should inherit a queue snapshot before any thin
    /// client connects, while an already-running daemon keeps its live state.
    pub fn set_initial_queue(&self, items: &[MediaItem], cursor: usize) {
        let mut st = self.status.lock().unwrap();
        if items.is_empty() {
            st.position_ticks = 0;
            st.runtime_ticks = 0;
            st.paused = false;
            st.current_idx = 0;
            st.queue_len = 0;
            st.active = false;
            st.clear_current_item_metadata();
            return;
        }

        let cursor = cursor.min(items.len().saturating_sub(1));
        st.position_ticks = items[cursor].playback_position_ticks;
        st.runtime_ticks = items[cursor].runtime_ticks;
        st.paused = false;
        st.current_idx = cursor;
        st.queue_len = items.len();
        st.active = false;
        st.set_current_item_metadata(&items[cursor]);
    }

    // Pipe mode always forces headless (no video window), regardless of item
    // type. Reads `audio_pipe_enabled` from `client.config` (rather than a
    // field cached on `Player`) so a setting toggled mid-session takes effect
    // on the very next play() call instead of requiring an app restart.
    fn headless_for(&self, client: &EmbyClient, is_audio: bool) -> bool {
        client.config.audio_pipe_enabled || (!self.show_audio_window && is_audio)
    }

    pub fn play(&self, item: &MediaItem, client: Arc<EmbyClient>, initial_volume: u8) {
        // Reuse the existing mpv window only when the headless state matches:
        // video→video and audio→audio reuse; video→audio and audio→video always
        // spawn a new process so the window visibility is correct.
        let new_is_headless = self.headless_for(&client, item.is_audio());
        if self.status.lock().unwrap().active
            && (self.current_is_headless.load(Ordering::Relaxed) == new_is_headless)
        {
            let ep = if item.is_audio() { "Audio" } else { "Videos" };
            let url = format!(
                "{}/{}/{}/stream?static=true&api_key={}",
                self.server_url, ep, item.id, self.token
            );
            let start_pos = if item.should_resume() {
                item.resume_seconds()
            } else {
                0.0
            };
            {
                let mut st = self.status.lock().unwrap();
                st.position_ticks = item.playback_position_ticks;
                st.runtime_ticks = item.runtime_ticks;
                st.paused = false;
                st.current_idx = 0;
                st.queue_len = 1;
                st.set_current_item_metadata(item);
            }
            self.send_command(PlayerCommand::LoadNew {
                url,
                start_pos,
                item: Box::new(item.clone()),
            });
            return;
        }

        self.stop();
        self.join();

        let item = item.clone();
        let is_audio = item.is_audio();
        let headless = new_is_headless;
        let item_pos = if is_audio {
            0
        } else {
            item.playback_position_ticks
        };
        let start_pos = if is_audio || !item.should_resume() {
            0.0
        } else {
            item.resume_seconds()
        };
        let ep = if is_audio { "Audio" } else { "Videos" };
        let url = format!(
            "{}/{}/{}/stream?static=true&api_key={}",
            self.server_url, ep, item.id, self.token
        );
        let title = item.display_name();

        let config = MpvSessionConfig {
            headless,
            use_mpv_config: self.use_mpv_config,
            no_scripts: self.no_scripts,
            always_skip_intro: self.always_skip_intro,
            audio_pipe_path: client.config.audio_pipe_target(),
            audio_pipe_samplerate: client.config.audio_pipe_samplerate,
            audio_pipe_bitdepth: client.config.audio_pipe_bitdepth,
        };
        let status = self.status.clone();
        let event_tx = self.event_tx.clone();
        let ws_tx = self.ws_tx.clone();
        let subtitle_prefs = self.subtitle_prefs.clone();
        let is_queue_mode = self.is_queue_mode.clone();
        let shutdown_report_timeout = self.shutdown_report_timeout.clone();
        let server_url = self.server_url.clone();
        let token = self.token.clone();
        self.current_is_headless.store(headless, Ordering::Relaxed);

        {
            let mut st = status.lock().unwrap();
            st.position_ticks = item_pos;
            st.runtime_ticks = item.runtime_ticks;
            st.paused = false;
            st.current_idx = 0;
            st.queue_len = 1;
            st.active = true;
            st.set_current_item_metadata(&item);
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *self.stop_tx.lock().unwrap() = Some(stop_tx);
        *self.shutdown_report_timeout.lock().unwrap() = None;
        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        *self.cmd_tx.lock().unwrap() = Some(cmd_tx);

        let handle = thread::spawn(move || {
            is_queue_mode.store(false, Ordering::Relaxed);

            let (mpv, startup_pause_for_pipe) = match init_mpv(&config) {
                Ok(v) => v,
                Err(e) => {
                    log::error!(target: "player", "{}", e);
                    return;
                }
            };
            init_volume(&mpv, &status, initial_volume);

            if start_pos > 0.0 {
                let _ = mpv.set_property("start", format!("{:.0}", start_pos));
            }
            let title_opt = mpv_title_opt(&title);
            log::info!(target: "player", "loadfile url={url} opts={title_opt:?}");
            if let Err(e) = mpv.command(
                "loadfile",
                &[url.as_str(), "replace", "-1", title_opt.as_str()],
            ) {
                log::warn!(target: "player", "loadfile error: {} | url={url} opts={title_opt:?}", mpv_err_str(&e));
                return;
            }
            send_ep_info(&mpv, &item);
            observe_properties(&mpv, config.use_mpv_config);

            let info = client.get_playback_info(&item.id);
            client.report_start(&item, &info.media_source_id, &info.session_id);
            let reporter = SessionReporter::new(
                client,
                ws_tx,
                item.id.clone(),
                info.media_source_id,
                info.session_id,
                is_audio,
                status.clone(),
            );
            let progress = spawn_progress_reporter(reporter.clone());
            let session = PlaybackSession::new(
                vec![item.clone()],
                0,
                PlaybackOrigin::Standalone,
                reporter,
                config,
                startup_pause_for_pipe,
                status,
                event_tx,
                subtitle_prefs,
                is_queue_mode.clone(),
                shutdown_report_timeout,
                server_url,
                token,
                info.external_subtitle_urls,
            );
            session.run(mpv, stop_rx, cmd_rx, progress);
        });
        *self.thread_handle.lock().unwrap() = Some(handle);
    }

    pub fn play_queue(
        &self,
        items: Vec<MediaItem>,
        start_idx: usize,
        client: Arc<EmbyClient>,
        initial_volume: u8,
    ) {
        if items.is_empty() {
            return;
        }

        let all_audio = items
            .iter()
            .all(|i| i.media_type == "Audio" || i.item_type == "Audio");
        let new_is_headless = self.headless_for(&client, all_audio);

        // If playlist loop already running and headless state matches, replace in
        // place (no window close). Mismatched state (e.g. video→audio-only or
        // vice-versa) always spawns a new process so visibility is correct.
        if self.status.lock().unwrap().active
            && self.is_queue_mode.load(Ordering::Relaxed)
            && (self.current_is_headless.load(Ordering::Relaxed) == new_is_headless)
        {
            let start_idx = start_idx.min(items.len() - 1);
            {
                let mut st = self.status.lock().unwrap();
                st.position_ticks = items[start_idx].playback_position_ticks;
                st.runtime_ticks = items[start_idx].runtime_ticks;
                st.paused = false;
                st.current_idx = start_idx;
                st.queue_len = items.len();
                st.set_current_item_metadata(&items[start_idx]);
            }
            self.send_command(PlayerCommand::ReplaceQueue { items, start_idx });
            return;
        }

        self.stop();
        self.join();

        let start_idx = start_idx.min(items.len() - 1);
        let headless = new_is_headless;

        let config = MpvSessionConfig {
            headless,
            use_mpv_config: self.use_mpv_config,
            no_scripts: self.no_scripts,
            always_skip_intro: self.always_skip_intro,
            audio_pipe_path: client.config.audio_pipe_target(),
            audio_pipe_samplerate: client.config.audio_pipe_samplerate,
            audio_pipe_bitdepth: client.config.audio_pipe_bitdepth,
        };
        let status = self.status.clone();
        let event_tx = self.event_tx.clone();
        let ws_tx = self.ws_tx.clone();
        let subtitle_prefs = self.subtitle_prefs.clone();
        let is_queue_mode = self.is_queue_mode.clone();
        let shutdown_report_timeout = self.shutdown_report_timeout.clone();
        let server_url = self.server_url.clone();
        let token = self.token.clone();
        self.current_is_headless.store(headless, Ordering::Relaxed);

        {
            let mut st = status.lock().unwrap();
            st.position_ticks = 0;
            st.runtime_ticks = items[start_idx].runtime_ticks;
            st.paused = false;
            st.current_idx = start_idx;
            st.queue_len = items.len();
            st.active = true;
            st.set_current_item_metadata(&items[start_idx]);
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *self.stop_tx.lock().unwrap() = Some(stop_tx);
        *self.shutdown_report_timeout.lock().unwrap() = None;
        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        *self.cmd_tx.lock().unwrap() = Some(cmd_tx);

        let handle = thread::spawn(move || {
            is_queue_mode.store(true, Ordering::Relaxed);

            let (mpv, startup_pause_for_pipe) = match init_mpv(&config) {
                Ok(v) => v,
                Err(e) => {
                    log::error!(target: "player", "{}", e);
                    return;
                }
            };
            init_volume(&mpv, &status, initial_volume);

            // Load the full playlist into mpv so every index matches items[i] directly.
            for (i, item) in items.iter().enumerate() {
                let ep = if item.is_audio() { "Audio" } else { "Videos" };
                let url = format!(
                    "{}/{}/{}/stream?static=true&api_key={}",
                    server_url, ep, item.id, token
                );
                let mode = if i == 0 { "replace" } else { "append-play" };
                let title_opt = mpv_title_opt(&item.display_name());
                if let Err(e) =
                    mpv.command("loadfile", &[url.as_str(), mode, "-1", title_opt.as_str()])
                {
                    log::warn!(target: "player", "loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e));
                    if i == 0 {
                        // First file failed: nothing queued, exit cleanly.
                        status.lock().unwrap().active = false;
                        return;
                    }
                    // Subsequent file failed: skip it, keep playing what loaded.
                }
            }
            send_ep_info(&mpv, &items[start_idx]);
            observe_properties(&mpv, config.use_mpv_config);

            let info = client.get_playback_info(&items[start_idx].id);
            client.report_start(&items[start_idx], &info.media_source_id, &info.session_id);
            let reporter = SessionReporter::new(
                client,
                ws_tx,
                items[start_idx].id.clone(),
                info.media_source_id,
                info.session_id,
                items[start_idx].is_audio(),
                status.clone(),
            );
            let progress = spawn_progress_reporter(reporter.clone());
            let session = PlaybackSession::new(
                items,
                start_idx,
                PlaybackOrigin::Queue,
                reporter,
                config,
                startup_pause_for_pipe,
                status,
                event_tx,
                subtitle_prefs,
                is_queue_mode.clone(),
                shutdown_report_timeout,
                server_url,
                token,
                info.external_subtitle_urls,
            );
            session.run(mpv, stop_rx, cmd_rx, progress);
        });
        *self.thread_handle.lock().unwrap() = Some(handle);
    }

    pub fn stop(&self) {
        if let Some(tx) = self.stop_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
        // Don't clear cmd_tx here: a LoadNew command sent after stop() must still
        // reach the thread so it can cancel the quit and load the new file instead.
    }

    pub fn stop_for_shutdown(&self, timeout: Duration) {
        *self.shutdown_report_timeout.lock().unwrap() = Some(timeout);
        self.stop();
    }
}

// ── PlayerProxy ─────────────────────────────────────────────────────────────
// Wraps either a local Player or a RemotePlayer so App can use a single type.

enum PlayerProxyInner {
    Local(Player),
    Remote(crate::remote_player::RemotePlayer),
}

pub struct PlayerProxy {
    pub always_play_next: bool,
    pub status: Arc<Mutex<PlayerStatus>>,
    pub subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    inner: PlayerProxyInner,
}

impl PlayerProxy {
    /// Test helper for root-crate integration tests that need a local player
    /// proxy without starting a real mpv session.
    #[cfg(feature = "test-support")]
    pub fn stub(status: Arc<Mutex<PlayerStatus>>) -> Self {
        let (tx, _rx) = std::sync::mpsc::channel();
        let player = Player::new(
            String::new(),
            String::new(),
            false,
            false,
            false,
            false,
            false,
            SubtitlePrefs::default(),
            tx,
            None,
        );
        let subtitle_prefs = player.subtitle_prefs.clone();
        PlayerProxy {
            always_play_next: false,
            status,
            subtitle_prefs,
            inner: PlayerProxyInner::Local(player),
        }
    }

    /// Like [`stub`](Self::stub), but with a real background thread wired up
    /// as the local `Player`'s `thread_handle`, which sleeps for `sleep`
    /// before finishing. Lets root-crate tests prove teardown code that
    /// calls `join_or_timeout` actually returns within its bound instead of
    /// blocking for the full `sleep` duration — the previously-unbounded
    /// normal in-app quit path (#202) is exactly this scenario with a
    /// hanging player thread.
    #[cfg(feature = "test-support")]
    pub fn stub_with_hung_thread(status: Arc<Mutex<PlayerStatus>>, sleep: Duration) -> Self {
        let (tx, _rx) = std::sync::mpsc::channel();
        let player = Player::new(
            String::new(),
            String::new(),
            false,
            false,
            false,
            false,
            false,
            SubtitlePrefs::default(),
            tx,
            None,
        );
        let handle = std::thread::spawn(move || {
            std::thread::sleep(sleep);
        });
        *player.thread_handle.lock().unwrap() = Some(handle);
        let subtitle_prefs = player.subtitle_prefs.clone();
        PlayerProxy {
            always_play_next: false,
            status,
            subtitle_prefs,
            inner: PlayerProxyInner::Local(player),
        }
    }

    /// Wires a fresh command channel into the local player and returns the
    /// receiving end, so a test can assert on what `send_command` actually sent
    /// without a real mpv thread running.
    /// Test helper that exposes the next command sent through a local proxy.
    #[cfg(feature = "test-support")]
    pub fn spy_on_commands(&self) -> mpsc::Receiver<PlayerCommand> {
        let (tx, rx) = mpsc::channel();
        if let PlayerProxyInner::Local(p) = &self.inner {
            *p.cmd_tx.lock().unwrap() = Some(tx);
        }
        rx
    }

    pub fn local(player: Player, always_play_next: bool) -> Self {
        let status = player.status.clone();
        let subtitle_prefs = player.subtitle_prefs.clone();
        PlayerProxy {
            always_play_next,
            status,
            subtitle_prefs,
            inner: PlayerProxyInner::Local(player),
        }
    }

    pub fn remote(remote: crate::remote_player::RemotePlayer, always_play_next: bool) -> Self {
        let status = remote.status.clone();
        let subtitle_prefs = remote.subtitle_prefs.clone();
        PlayerProxy {
            always_play_next,
            status,
            subtitle_prefs,
            inner: PlayerProxyInner::Remote(remote),
        }
    }

    pub fn play(
        &self,
        item: &MediaItem,
        source: crate::config::QueueSource,
        client: Arc<EmbyClient>,
        initial_volume: u8,
    ) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.play(item, client, initial_volume),
            PlayerProxyInner::Remote(r) => r.play(item, source, client, initial_volume),
        }
    }

    pub fn play_queue(
        &self,
        items: Vec<MediaItem>,
        start_idx: usize,
        source: crate::config::QueueSource,
        client: Arc<EmbyClient>,
        initial_volume: u8,
    ) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.play_queue(items, start_idx, client, initial_volume),
            PlayerProxyInner::Remote(r) => {
                r.play_queue(items, start_idx, source, client, initial_volume)
            }
        }
    }

    pub fn stop(&self) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.stop(),
            PlayerProxyInner::Remote(r) => r.stop(),
        }
    }

    pub fn stop_for_shutdown(&self, timeout: Duration) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.stop_for_shutdown(timeout),
            PlayerProxyInner::Remote(_) => {}
        }
    }

    pub fn join(&self) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.join(),
            PlayerProxyInner::Remote(r) => r.join(),
        }
    }

    /// Tears down the underlying connection if this proxy is currently
    /// remote (#233): a no-op for `Local` (there's no socket to close).
    /// Call this on the *old* `PlayerProxy` before overwriting it with a
    /// freshly connected one -- a remote-to-remote swap that skips this
    /// leaks the old connection's reader thread (see
    /// `RemotePlayer::disconnect`'s doc comment for why `Drop` alone isn't
    /// enough).
    pub fn disconnect_remote(&self) {
        if let PlayerProxyInner::Remote(r) = &self.inner {
            r.disconnect();
        }
    }

    pub fn join_or_timeout(&self, timeout: std::time::Duration) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.join_or_timeout(timeout),
            PlayerProxyInner::Remote(_) => {}
        }
    }

    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.send_command(cmd),
            PlayerProxyInner::Remote(r) => r.send_command(cmd),
        }
    }

    pub fn supports_queue_append(&self) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => true,
            PlayerProxyInner::Remote(r) => r.supports_queue_append(),
        }
    }

    pub fn next(&self) -> bool {
        match self.status.lock().unwrap().next_idx() {
            Some(idx) => self.send_command(PlayerCommand::JumpTo(idx)),
            None => false,
        }
    }

    pub fn previous(&self) -> bool {
        match self.status.lock().unwrap().previous_idx() {
            Some(idx) => self.send_command(PlayerCommand::JumpTo(idx)),
            None => false,
        }
    }

    pub fn set_paused(&self, paused: bool) -> bool {
        match self.status.lock().unwrap().toggle_to_reach(paused) {
            Some(cmd) => self.send_command(cmd),
            None => false,
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.inner, PlayerProxyInner::Remote(_))
    }

    /// Returns a clone of the raw local `Player`'s command channel, or
    /// `None` when this proxy currently wraps a `RemotePlayer`.
    ///
    /// Intended for callers (e.g. the stay-alive tray, `src/tray.rs`) that
    /// must drive playback through the in-process `Player` mpsc *only* --
    /// never through `send_command`/`next`/`previous`/`set_paused` above,
    /// which forward to `RemotePlayer::send_command` (a ctrl-socket call)
    /// when this proxy is `Remote`. Capture this once, while the proxy is
    /// known to be `Local`, and keep the returned `Arc` rather than
    /// re-deriving it later: `App::switch_to_direct_remote` /
    /// `restore_local_mode` can swap `PlayerProxy`'s inner variant at
    /// runtime, but they never replace the underlying local `Player`
    /// object itself (it is suspended/resumed, not dropped), so the `Arc`
    /// returned here stays valid and keeps targeting the same in-process
    /// player for the caller's whole lifetime -- immune to later
    /// local/remote swaps on `self`.
    pub fn local_cmd_tx(&self) -> Option<Arc<Mutex<Option<mpsc::Sender<PlayerCommand>>>>> {
        match &self.inner {
            PlayerProxyInner::Local(p) => Some(p.cmd_tx.clone()),
            PlayerProxyInner::Remote(_) => None,
        }
    }

    pub fn is_remote_disconnected(&self) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => false,
            PlayerProxyInner::Remote(r) => r.is_disconnected(),
        }
    }

    /// Shared disconnect flag for the current inner target, or `None` for a
    /// local `Player` (which has no daemon connection to lose). Mirrors
    /// `RemotePlayer::disconnected_flag()` -- used by callers (MPRIS
    /// wiring, see #175) that need to rebind to whichever target currently
    /// owns playback without caring whether that's local or remote.
    pub fn disconnected_flag(&self) -> Option<Arc<AtomicBool>> {
        match &self.inner {
            PlayerProxyInner::Local(_) => None,
            PlayerProxyInner::Remote(r) => Some(r.disconnected_flag()),
        }
    }

    /// A `'static`, cheaply-cloneable command sender for whichever target
    /// this proxy currently wraps, independent of `self`'s lifetime.
    ///
    /// Exists so callers that need to hand a command path to something
    /// that outlives `self` (e.g. MPRIS's polling thread, see #175) don't
    /// have to hand-roll the local/remote match themselves -- and so that
    /// rebinding after `App::switch_to_direct_remote` /
    /// `restore_local_mode` always routes through whatever `self.inner`
    /// currently is at the moment this is called, not whatever it was when
    /// some earlier closure was built.
    pub fn command_sender(&self) -> Arc<dyn Fn(PlayerCommand) + Send + Sync> {
        match &self.inner {
            PlayerProxyInner::Local(p) => {
                let cmd_tx = p.cmd_tx.clone();
                Arc::new(move |cmd: PlayerCommand| {
                    if let Some(tx) = cmd_tx.lock().unwrap().as_ref() {
                        let _ = tx.send(cmd);
                    }
                })
            }
            PlayerProxyInner::Remote(r) => {
                let remote = r.clone();
                Arc::new(move |cmd: PlayerCommand| {
                    remote.send_command(cmd);
                })
            }
        }
    }

    /// Returns a clonable stop handle for use from other threads (e.g. the
    /// quit watchdog). None in remote mode — the daemon owns the player.
    pub fn quit_handle(&self) -> Option<QuitHandle> {
        match &self.inner {
            PlayerProxyInner::Local(p) => Some(QuitHandle {
                stop_tx: p.stop_tx.clone(),
                shutdown_report_timeout: p.shutdown_report_timeout.clone(),
            }),
            PlayerProxyInner::Remote(_) => None,
        }
    }
}

/// Retry mark_played in a detached thread with exponential backoff.
/// Max 3 attempts (initial + 2 retries), delays: 500ms, 2s.
fn retry_mark_played(client: Arc<EmbyClient>, item_id: String) {
    std::thread::spawn(move || {
        let delays = [500, 2000]; // ms
        for (i, delay_ms) in delays.iter().enumerate() {
            std::thread::sleep(Duration::from_millis(*delay_ms));
            match client.mark_played(&item_id) {
                Ok(()) => {
                    log::info!(target: "player", "mark_played retry {} ok id={item_id}", i + 1);
                    return;
                }
                Err(e) => {
                    log::warn!(target: "player", "mark_played retry {} failed id={item_id}: {e}", i + 1);
                }
            }
        }
    });
}

// True when the track ended close enough to its natural end to count as played.
// Threshold: position ≥ 95% of runtime (19/20 integer check avoids floating point).
pub(crate) fn is_near_end(
    is_audio: bool,
    natural: bool,
    last_valid_pos: i64,
    runtime_ticks: i64,
) -> bool {
    !natural && !is_audio && runtime_ticks > 0 && last_valid_pos * 20 / runtime_ticks >= 19
}

// Position to report to Emby when a playlist track ends.
// Zero means "treat as fully played / reset resume point".
// was_next_up alone does NOT zero the position — the user may have dismissed
// or ignored the overlay, and we must preserve where they actually were.
pub(crate) fn queue_completed_pos(
    is_audio: bool,
    natural: bool,
    near_end: bool,
    last_valid_pos: i64,
) -> i64 {
    if is_audio || natural || near_end {
        0
    } else {
        last_valid_pos
    }
}

fn quit_timeout_stop_flags(
    origin: PlaybackOrigin,
    is_audio: bool,
    last_valid_pos: i64,
    runtime_ticks: i64,
    stopped_near_end: bool,
) -> (bool, bool) {
    match origin {
        PlaybackOrigin::Standalone => (
            is_near_end(is_audio, false, last_valid_pos, runtime_ticks),
            false,
        ),
        PlaybackOrigin::Queue => (stopped_near_end, stopped_near_end),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopReportContext {
    Ordinary,
    ShutdownAware,
}

fn end_file_stop_report_context(reason: EndFileReason) -> StopReportContext {
    if reason == mpv_end_file_reason::Quit {
        StopReportContext::ShutdownAware
    } else {
        StopReportContext::Ordinary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::tests::SYS_ENV_LOCK;

    struct MpvConfigTestEnv {
        root: PathBuf,
        runtime_dir: PathBuf,
        user_mpv: PathBuf,
        old_runtime: Option<std::ffi::OsString>,
        old_config: Option<std::ffi::OsString>,
        old_system: Option<std::ffi::OsString>,
    }

    impl MpvConfigTestEnv {
        fn new(name: &str) -> Self {
            let mut root = std::env::temp_dir();
            root.push(format!(
                "mbv-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let runtime_dir = root.join("runtime");
            let xdg_config = root.join("xdg-config");
            let user_mpv = xdg_config.join("mpv");
            let old_runtime = std::env::var_os("XDG_RUNTIME_DIR");
            let old_config = std::env::var_os("XDG_CONFIG_HOME");
            let old_system = std::env::var_os("MBV_SYSTEM");

            std::env::set_var("XDG_RUNTIME_DIR", &runtime_dir);
            std::env::set_var("XDG_CONFIG_HOME", &xdg_config);
            std::env::remove_var("MBV_SYSTEM");

            Self {
                root,
                runtime_dir,
                user_mpv,
                old_runtime,
                old_config,
                old_system,
            }
        }

        fn restore_env(key: &str, previous: &Option<std::ffi::OsString>) {
            if let Some(value) = previous {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    impl Drop for MpvConfigTestEnv {
        fn drop(&mut self) {
            Self::restore_env("XDG_RUNTIME_DIR", &self.old_runtime);
            Self::restore_env("XDG_CONFIG_HOME", &self.old_config);
            Self::restore_env("MBV_SYSTEM", &self.old_system);
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    // ── private mpv config isolation ─────────────────────────────────────────

    #[test]
    fn sanitized_mpv_conf_removes_active_ipc_options_and_appends_mbv_ipc() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        let env = MpvConfigTestEnv::new("sanitize-mpv-conf");
        std::fs::create_dir_all(&env.user_mpv).unwrap();
        let conf_path = env.user_mpv.join("mpv.conf");
        std::fs::write(
            &conf_path,
            "\
volume=75
input-ipc-server=/tmp/user.sock
--input-ipc-server=/tmp/other.sock
 input-ipc-server /tmp/spaced.sock
# input-ipc-server=/tmp/commented.sock
",
        )
        .unwrap();

        let sanitized = sanitized_mpv_conf(Some(&conf_path), "/tmp/mbv.sock");

        assert!(sanitized.contains("volume=75\n"));
        assert!(!sanitized.contains("/tmp/user.sock"));
        assert!(!sanitized.contains("/tmp/other.sock"));
        assert!(!sanitized.contains("/tmp/spaced.sock"));
        assert!(sanitized.contains("# input-ipc-server=/tmp/commented.sock\n"));
        assert!(sanitized.ends_with("input-ipc-server=/tmp/mbv.sock\n"));
    }

    #[test]
    fn prepare_mpv_config_dir_symlinks_user_entries_but_not_mpv_or_input_conf() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        let env = MpvConfigTestEnv::new("private-mpv-config");
        std::fs::create_dir_all(env.user_mpv.join("scripts")).unwrap();
        std::fs::create_dir_all(env.user_mpv.join("script-opts")).unwrap();
        std::fs::write(
            env.user_mpv.join("mpv.conf"),
            "volume=65\ninput-ipc-server=/tmp/user.sock\n",
        )
        .unwrap();
        std::fs::write(env.user_mpv.join("input.conf"), "q quit\n").unwrap();

        let private_dir = prepare_mpv_config_dir(true, "/tmp/mbv.sock").unwrap();
        let conf = std::fs::read_to_string(private_dir.join("mpv.conf")).unwrap();

        assert_eq!(private_dir, env.runtime_dir.join("mpv-config"));
        assert!(conf.contains("volume=65\n"));
        assert!(!conf.contains("/tmp/user.sock"));
        assert!(conf.contains("input-ipc-server=/tmp/mbv.sock\n"));
        assert!(std::fs::symlink_metadata(private_dir.join("scripts"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(std::fs::symlink_metadata(private_dir.join("script-opts"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(!private_dir.join("input.conf").exists());
    }

    #[test]
    fn prepare_mpv_config_dir_ignores_user_config_when_disabled() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        let env = MpvConfigTestEnv::new("private-mpv-config-disabled");
        std::fs::create_dir_all(env.user_mpv.join("scripts")).unwrap();
        std::fs::write(env.user_mpv.join("mpv.conf"), "volume=65\n").unwrap();

        let private_dir = prepare_mpv_config_dir(false, "/tmp/mbv.sock").unwrap();
        let conf = std::fs::read_to_string(private_dir.join("mpv.conf")).unwrap();

        assert_eq!(conf, "input-ipc-server=/tmp/mbv.sock\n");
        assert!(!private_dir.join("scripts").exists());
    }

    // ── shift_index_for_move ──────────────────────────────────────────────────

    #[test]
    fn shift_index_for_move_moves_the_tracked_index_itself() {
        assert_eq!(shift_index_for_move(1, 1, 3), 3);
        assert_eq!(shift_index_for_move(3, 3, 1), 1);
    }

    #[test]
    fn shift_index_for_move_shifts_indices_between_from_and_to() {
        // Moving 1 -> 3 closes the gap it left, shifting everything in (1, 3] down.
        assert_eq!(shift_index_for_move(2, 1, 3), 1);
        assert_eq!(shift_index_for_move(3, 1, 3), 2);
        // Moving 3 -> 1 opens a gap at 1, shifting everything in [1, 3) up.
        assert_eq!(shift_index_for_move(1, 3, 1), 2);
        assert_eq!(shift_index_for_move(2, 3, 1), 3);
    }

    #[test]
    fn shift_index_for_move_leaves_unrelated_indices_alone() {
        assert_eq!(shift_index_for_move(0, 1, 3), 0);
        assert_eq!(shift_index_for_move(4, 1, 3), 4);
    }

    // ── mpv_title_opt ────────────────────────────────────────────────────────

    #[test]
    fn title_opt_plain() {
        assert_eq!(mpv_title_opt("Inception"), "force-media-title=%9%Inception");
    }

    #[test]
    fn title_opt_comma() {
        assert_eq!(
            mpv_title_opt("Cardiff, Claire (2)"),
            "force-media-title=%19%Cardiff, Claire (2)"
        );
    }

    #[test]
    fn title_opt_backslash() {
        assert_eq!(mpv_title_opt("A\\B"), "force-media-title=%3%A\\B");
    }

    #[test]
    fn title_opt_empty() {
        assert_eq!(mpv_title_opt(""), "force-media-title=%0%");
    }

    // ── PlayerCommand serde (IPC protocol integrity) ─────────────────────────

    fn make_media_item(id: &str) -> crate::api::MediaItem {
        crate::api::MediaItem {
            id: id.into(),
            name: "Test Episode".into(),
            item_type: "Episode".into(),
            is_folder: false,
            media_type: "Video".into(),
            collection_type: String::new(),
            runtime_ticks: 3600 * crate::api::TICKS_PER_SECOND,
            played: false,
            playback_position_ticks: 0,
            series_id: "series1".into(),
            series_name: "Show".into(),
            album_id: String::new(),
            album: String::new(),
            index_number: 2,
            parent_index_number: 1,
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

    fn make_queue_session_for_pos_tests(
        start_idx: usize,
    ) -> (PlaybackSession, Arc<Mutex<PlayerStatus>>) {
        let items = vec![
            make_media_item("ep1"),
            make_media_item("ep2"),
            make_media_item("ep3"),
        ];
        let status = Arc::new(Mutex::new(PlayerStatus {
            active: true,
            current_idx: start_idx,
            queue_len: items.len(),
            runtime_ticks: items[start_idx].runtime_ticks,
            title: items[start_idx].display_name(),
            ..Default::default()
        }));
        let client = Arc::new(EmbyClient::new(crate::config::Config::default()));
        let reporter = SessionReporter::new(
            client,
            None,
            items[start_idx].id.clone(),
            "msid".into(),
            "sid".into(),
            false,
            status.clone(),
        );
        let (event_tx, _event_rx) = mpsc::channel();
        let session = PlaybackSession::new(
            items,
            start_idx,
            PlaybackOrigin::Queue,
            reporter,
            MpvSessionConfig {
                headless: false,
                use_mpv_config: false,
                no_scripts: true,
                always_skip_intro: false,
                audio_pipe_path: Some("/tmp/mbv-test-pipe".into()),
                audio_pipe_samplerate: 48_000,
                audio_pipe_bitdepth: 16,
            },
            false,
            status.clone(),
            event_tx,
            Arc::new(Mutex::new(SubtitlePrefs::default())),
            Arc::new(AtomicBool::new(true)),
            Arc::new(Mutex::new(None)),
            "http://example.test".into(),
            "token".into(),
            Vec::new(),
        );
        (session, status)
    }

    #[test]
    fn cancel_pending_quit_clears_quit_at_and_shutdown_timeout() {
        // Regression test for a code-review finding: cmd_load_new and
        // cmd_replace_queue (via the shared cancel_pending_quit helper)
        // must reset shutdown_report_timeout, not just quit_at, when a
        // LoadNew/ReplaceQueue command cancels an in-flight quit. Otherwise
        // App::teardown -> Player::stop_for_shutdown sets
        // shutdown_report_timeout = Some(quit_timeout) before sending the
        // stop signal; if that quit then gets cancelled by an
        // already-queued LoadNew/ReplaceQueue, shutdown_report_timeout
        // would stay Some for the rest of the session, silently degrading
        // every later track transition to the tight shutdown budget/no-retry
        // path instead of the ordinary one. cmd_load_new/cmd_replace_queue
        // themselves aren't unit-tested directly here since they require a
        // real Mpv handle; this exercises the exact reset logic they share.
        let (mut session, _status) = make_queue_session_for_pos_tests(0);
        session.quit_at = Some(Instant::now());
        *session.shutdown_report_timeout.lock().unwrap() = Some(Duration::from_secs(5));

        session.cancel_pending_quit();

        assert!(session.quit_at.is_none());
        assert!(session.shutdown_report_timeout.lock().unwrap().is_none());
        // progress_join_budget/report_stopped_for_current_context both key off
        // shutdown_report_timeout being None to behave as ordinary mid-playback
        // calls again — asserting the None state above is the load-bearing
        // check; both helpers are exercised directly by other tests.
        assert_eq!(session.progress_join_budget(), Duration::from_secs(30));
    }

    #[test]
    fn playlist_pos_does_not_clobber_pending_initial_queue_jump() {
        let (mut session, status) = make_queue_session_for_pos_tests(2);

        session.on_playlist_pos_changed(0);

        assert_eq!(session.current_idx, 2);
        assert_eq!(status.lock().unwrap().current_idx, 2);
    }

    #[test]
    fn playlist_pos_does_not_clobber_pending_replace_queue_load() {
        let (mut session, status) = make_queue_session_for_pos_tests(1);
        session.pending_initial_jump = false;
        session.pending_load = 1;

        session.on_playlist_pos_changed(0);

        assert_eq!(session.current_idx, 1);
        assert_eq!(status.lock().unwrap().current_idx, 1);
    }

    #[test]
    fn playlist_pos_does_not_clobber_in_flight_jump_to() {
        let (mut session, status) = make_queue_session_for_pos_tests(0);
        session.pending_initial_jump = false;
        session.forced_slot_id = session.slot_id_at(1);

        session.on_playlist_pos_changed(1);

        assert_eq!(session.current_idx, 0);
        assert_eq!(status.lock().unwrap().current_idx, 0);
        assert_eq!(session.forced_slot_id, session.slot_id_at(1));
    }

    #[test]
    fn playlist_pos_updates_idle_queue_with_valid_mpv_position() {
        let (mut session, status) = make_queue_session_for_pos_tests(0);
        session.pending_initial_jump = false;

        session.on_playlist_pos_changed(2);

        assert_eq!(session.current_idx, 2);
        assert_eq!(status.lock().unwrap().current_idx, 2);
    }

    #[test]
    fn append_items_to_queue_extends_queue_without_moving_current_idx() {
        let (mut session, status) = make_queue_session_for_pos_tests(1);
        let appended = make_media_item("ep4");

        session.append_items_to_queue(vec![appended.clone()]);

        assert_eq!(session.queue_len(), 4);
        assert_eq!(session.current_idx, 1);
        let status = status.lock().unwrap();
        assert_eq!(status.current_idx, 1);
        assert_eq!(status.queue_len, 4);
        assert_eq!(
            session
                .queue
                .slots()
                .last()
                .map(|slot| slot.item.id.as_str()),
            Some(appended.id.as_str())
        );
    }

    #[test]
    fn load_new_serde_roundtrip() {
        let cmd = PlayerCommand::LoadNew {
            url: "http://emby.local/Videos/ep1/stream".into(),
            start_pos: 0.0,
            item: Box::new(make_media_item("ep1")),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: PlayerCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, PlayerCommand::LoadNew { .. }));
    }

    #[test]
    fn shutdown_stop_sets_timeout_without_changing_plain_stop() {
        let (event_tx, _event_rx) = mpsc::channel();
        let player = Player::new(
            String::new(),
            String::new(),
            false,
            false,
            false,
            false,
            false,
            SubtitlePrefs::default(),
            event_tx,
            None,
        );

        let (plain_tx, plain_rx) = mpsc::channel();
        *player.stop_tx.lock().unwrap() = Some(plain_tx);
        player.stop();
        assert!(plain_rx.recv_timeout(Duration::from_millis(50)).is_ok());
        assert!(player.shutdown_report_timeout.lock().unwrap().is_none());

        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        *player.stop_tx.lock().unwrap() = Some(shutdown_tx);
        player.stop_for_shutdown(Duration::from_secs(7));
        assert!(shutdown_rx.recv_timeout(Duration::from_millis(50)).is_ok());
        assert_eq!(
            *player.shutdown_report_timeout.lock().unwrap(),
            Some(Duration::from_secs(7))
        );
    }

    #[test]
    fn end_file_quit_uses_shutdown_aware_stop_report_context() {
        assert_eq!(
            end_file_stop_report_context(mpv_end_file_reason::Quit),
            StopReportContext::ShutdownAware
        );
        assert_eq!(
            end_file_stop_report_context(mpv_end_file_reason::Eof),
            StopReportContext::Ordinary
        );
        assert_eq!(
            end_file_stop_report_context(mpv_end_file_reason::Error),
            StopReportContext::Ordinary
        );
    }

    #[test]
    fn progress_guard_stop_and_join_bounded_when_thread_hangs() {
        let (stop_tx, _stop_rx) = mpsc::channel();
        let handle = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(5));
        });
        let mut guard = ProgressGuard {
            stop_tx,
            handle: Some(handle),
        };

        let started = std::time::Instant::now();
        guard.stop_and_join(Duration::from_millis(150));
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_secs(1),
            "stop_and_join should return near its 150ms budget, took {elapsed:?}"
        );
        assert!(
            guard.handle.is_none(),
            "handle should be taken regardless of outcome"
        );
    }

    #[test]
    fn progress_guard_stop_and_join_fast_when_thread_finishes_quickly() {
        let (stop_tx, _stop_rx) = mpsc::channel();
        let handle = std::thread::spawn(|| {});
        let mut guard = ProgressGuard {
            stop_tx,
            handle: Some(handle),
        };

        let started = std::time::Instant::now();
        guard.stop_and_join(Duration::from_secs(30));
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_secs(1),
            "a thread that finishes immediately should not add latency, took {elapsed:?}"
        );
    }

    // ── queue_completed_pos / is_near_end ─────────────────────────────────

    const RUNTIME: i64 = 600 * TICKS_PER_SECOND; // 10-minute episode

    #[test]
    fn mid_episode_quit_preserves_position() {
        // User quits at ~88% (528 s into a 600 s episode). Not natural, not near-end,
        // next-up overlay may have appeared but next_up_jump was never set because the
        // user pressed q rather than clicking the overlay. Position must be preserved.
        let pos = 528 * TICKS_PER_SECOND;
        assert!(!is_near_end(false, false, pos, RUNTIME)); // 88% < 95%
        assert_eq!(queue_completed_pos(false, false, false, pos), pos);
    }

    #[test]
    fn next_up_fired_preserves_position() {
        // Bug fix: was_next_up alone used to force completed_pos = 0. After the fix,
        // only natural EOF or >=95% position zeroes it. next_up_jump is now irrelevant
        // to completed_pos — queue_completed_pos doesn't receive it at all.
        let pos = 540 * TICKS_PER_SECOND; // 90% — past 60s-before-end threshold
        assert!(!is_near_end(false, false, pos, RUNTIME)); // still below 95%
        assert_eq!(queue_completed_pos(false, false, false, pos), pos);
    }

    #[test]
    fn natural_end_resets_position() {
        let pos = RUNTIME - TICKS_PER_SECOND; // 1 s before end
        assert_eq!(queue_completed_pos(false, true, false, pos), 0);
    }

    #[test]
    fn near_end_boundary_resets_position() {
        // Exactly 95% (19/20) is near-end; 94% is not.
        let at_95 = RUNTIME * 19 / 20;
        let below = at_95 - 1;
        assert!(is_near_end(false, false, at_95, RUNTIME));
        assert!(!is_near_end(false, false, below, RUNTIME));
        assert_eq!(queue_completed_pos(false, false, true, at_95), 0);
        assert_eq!(queue_completed_pos(false, false, false, below), below);
    }

    #[test]
    fn audio_track_always_resets_position() {
        let pos = 300 * TICKS_PER_SECOND; // 50%
        assert!(!is_near_end(true, false, pos, RUNTIME));
        assert_eq!(queue_completed_pos(true, false, false, pos), 0);
    }

    #[test]
    fn near_end_requires_runtime_known() {
        // If runtime_ticks is 0 (unknown), near-end must never trigger.
        assert!(!is_near_end(false, false, 1_000_000_000, 0));
    }

    #[test]
    fn standalone_quit_timeout_marks_near_end_without_consuming() {
        let pos = RUNTIME * 19 / 20;
        assert_eq!(
            quit_timeout_stop_flags(PlaybackOrigin::Standalone, false, pos, RUNTIME, false),
            (true, false)
        );
        assert_eq!(
            quit_timeout_stop_flags(PlaybackOrigin::Standalone, true, pos, RUNTIME, false),
            (false, false)
        );
        assert_eq!(
            quit_timeout_stop_flags(PlaybackOrigin::Queue, false, pos, RUNTIME, true),
            (true, true)
        );
    }

    #[test]
    fn standalone_fresh_start_does_not_set_pending_resume_secs() {
        // Mirrors cmd_load_new's mutation sequence for a fresh one-slot standalone
        // load of a resumable video: origin becomes Standalone, the queue is
        // replaced with the single new item, then load_active_item_state() runs.
        // mpv's `start` property (set separately by cmd_load_new, not exercised
        // here since it requires a live mpv) already seeks to the resume position,
        // so pending_resume_secs must stay None to avoid a redundant absolute
        // seek in on_playback_restart that would also suppress the first
        // progress report for ~500ms.
        let (mut session, _status) = make_queue_session_for_pos_tests(0);

        let mut item = make_media_item("resumable");
        item.playback_position_ticks = item.runtime_ticks / 2; // 50% watched
        assert!(item.should_resume(), "test item must actually be resumable");

        session.origin = PlaybackOrigin::Standalone;
        session.queue = PlaybackQueue::from_items(vec![item], Some(0));
        session.current_idx = 0;

        session.load_active_item_state();

        assert_eq!(
            session.pending_resume_secs, None,
            "standalone fresh-start must rely on mpv's `start` property, not a redundant seek"
        );
    }

    #[test]
    fn queue_slot_activation_still_sets_pending_resume_secs() {
        // Sibling case to the standalone fix above: mid-session slot activation
        // (Queue origin) has no mpv `start`-property shortcut, so
        // load_active_item_state() must still arm pending_resume_secs for a
        // resumable item.
        let (mut session, _status) = make_queue_session_for_pos_tests(0);

        let mut item = make_media_item("resumable");
        item.playback_position_ticks = item.runtime_ticks / 2; // 50% watched
        let resume_secs = item.resume_seconds();
        assert!(item.should_resume(), "test item must actually be resumable");

        session.origin = PlaybackOrigin::Queue;
        session.queue = PlaybackQueue::from_items(vec![item], Some(0));
        session.current_idx = 0;

        session.load_active_item_state();

        assert_eq!(session.pending_resume_secs, Some(resume_secs));
    }

    #[test]
    fn subtitle_stream_index_maps_to_mpv_subtitle_id() {
        let status = PlayerStatus {
            active: true,
            sub_tracks: vec![(1, "English".to_string(), false)],
            sub_track_stream_indexes: vec![(1, 2)],
            video_height: 1080,
            ..Default::default()
        };

        assert_eq!(status.subtitle_stream_index_to_mpv_id(2), Some(1));
        assert_eq!(status.subtitle_stream_index_to_mpv_id(-1), Some(0));
        assert_eq!(status.subtitle_stream_index_to_mpv_id(1), None);
    }

    // ── PlayerStatus::next_idx / previous_idx / toggle_to_reach ──────────────
    // (issue #80: single source of truth for next/previous/toggle-play bounds
    // and paused-state logic, replacing four near-identical copies.)

    fn make_status(
        current_idx: usize,
        queue_len: usize,
        active: bool,
        paused: bool,
    ) -> PlayerStatus {
        PlayerStatus {
            paused,
            current_idx,
            queue_len,
            active,
            ..Default::default()
        }
    }

    #[test]
    fn next_idx_advances_when_room() {
        let status = make_status(1, 3, true, false);
        assert_eq!(status.next_idx(), Some(2));
    }

    #[test]
    fn next_idx_none_at_end_of_queue() {
        // Regression test for the mpris.rs bug: next() had no upper-bound check
        // and would jump past the end of the queue.
        let status = make_status(2, 3, true, false);
        assert_eq!(status.next_idx(), None);
    }

    #[test]
    fn next_idx_none_when_inactive() {
        let status = make_status(0, 3, false, false);
        assert_eq!(status.next_idx(), None);
    }

    #[test]
    fn current_item_metadata_stores_art_item_id_never_a_token_url() {
        // Regression coverage for #158: `set_current_item_metadata` no longer
        // takes a server URL or token at all (it can't reconstruct an Emby
        // image URL that would embed `token` as a query-string api_key and
        // leak it onto the session D-Bus via mpris:artUrl). mbv-core has no
        // access to the on-disk image cache, so it only records the raw item
        // id; `src/mpris.rs::resolve_art_url` turns that into a file:// URI
        // (or omits mpris:artUrl) using the cache.
        let mut item = make_media_item("track-1");
        item.artist = "Artist".to_string();
        item.album = "Album".to_string();

        let mut status = PlayerStatus::default();
        status.set_current_item_metadata(&item);

        assert_eq!(status.title, item.display_name());
        assert_eq!(status.artist, "Artist");
        assert_eq!(status.album, "Album");
        assert_eq!(status.art_item_id, "track-1");
        // Episode (the default make_media_item item_type) isn't grouped by
        // album, so no album-cache key is recorded.
        assert_eq!(status.art_album_id, "");
    }

    #[test]
    fn current_item_metadata_uses_album_id_for_grouped_audio_tracks() {
        let mut item = make_media_item("track-1");
        item.item_type = "Audio".to_string();
        item.album_id = "album-9".to_string();

        let mut status = PlayerStatus::default();
        status.set_current_item_metadata(&item);

        assert_eq!(status.art_item_id, "track-1");
        assert_eq!(status.art_album_id, "album-9");
    }

    #[test]
    fn clear_current_item_metadata_clears_art_fields() {
        let mut item = make_media_item("track-1");
        item.item_type = "Audio".to_string();
        item.album_id = "album-9".to_string();

        let mut status = PlayerStatus::default();
        status.set_current_item_metadata(&item);
        status.clear_current_item_metadata();

        assert_eq!(status.art_item_id, "");
        assert_eq!(status.art_album_id, "");
    }

    #[test]
    fn previous_idx_none_at_start() {
        let status = make_status(0, 3, true, false);
        assert_eq!(status.previous_idx(), None);
    }

    #[test]
    fn previous_idx_steps_back() {
        let status = make_status(2, 3, true, false);
        assert_eq!(status.previous_idx(), Some(1));
    }

    #[test]
    fn previous_idx_none_when_inactive() {
        let status = make_status(2, 3, false, false);
        assert_eq!(status.previous_idx(), None);
    }

    #[test]
    fn toggle_to_reach_noop_when_already_in_state() {
        let status = make_status(0, 1, true, true);
        assert!(status.toggle_to_reach(true).is_none());
    }

    #[test]
    fn toggle_to_reach_emits_toggle_when_state_differs() {
        let status = make_status(0, 1, true, false);
        assert!(matches!(
            status.toggle_to_reach(true),
            Some(PlayerCommand::TogglePause)
        ));
    }

    #[test]
    fn set_initial_queue_seeds_status_without_starting_playback() {
        let (tx, _rx) = mpsc::channel();
        let player = Player::new(
            String::new(),
            String::new(),
            false,
            false,
            false,
            false,
            false,
            SubtitlePrefs::default(),
            tx,
            None,
        );
        let mut items = vec![make_media_item("ep1"), make_media_item("ep2")];
        items[1].playback_position_ticks = 123;
        items[1].runtime_ticks = 456;

        player.set_initial_queue(&items, 1);

        let status = player.status.lock().unwrap().clone();
        assert_eq!(status.current_idx, 1);
        assert_eq!(status.queue_len, 2);
        assert_eq!(status.position_ticks, 123);
        assert_eq!(status.runtime_ticks, 456);
        assert!(!status.active);
        assert_eq!(status.title, items[1].display_name());
    }

    // ── lang_code_to_name (sync with parse_audio_info in api.rs) ─────────────
    // Mirror of parse_audio_info_lang_table_matches_player_lang_code_to_name in
    // api.rs::tests. Both tables must be updated together when adding a language.
    #[test]
    fn lang_code_to_name_matches_api_table() {
        let cases: &[(&str, &str)] = &[
            ("en", "English"),
            ("eng", "English"),
            ("fr", "French"),
            ("fre", "French"),
            ("fra", "French"),
            ("de", "German"),
            ("ger", "German"),
            ("deu", "German"),
            ("es", "Spanish"),
            ("spa", "Spanish"),
            ("it", "Italian"),
            ("ita", "Italian"),
            ("pt", "Portuguese"),
            ("por", "Portuguese"),
            ("ja", "Japanese"),
            ("jpn", "Japanese"),
            ("ko", "Korean"),
            ("kor", "Korean"),
            ("zh", "Chinese"),
            ("chi", "Chinese"),
            ("zho", "Chinese"),
            ("ru", "Russian"),
            ("rus", "Russian"),
            ("ar", "Arabic"),
            ("ara", "Arabic"),
            ("nl", "Dutch"),
            ("nld", "Dutch"),
            ("dut", "Dutch"),
            ("sv", "Swedish"),
            ("swe", "Swedish"),
            ("no", "Norwegian"),
            ("nor", "Norwegian"),
            ("da", "Danish"),
            ("dan", "Danish"),
            ("fi", "Finnish"),
            ("fin", "Finnish"),
            ("pl", "Polish"),
            ("pol", "Polish"),
            ("cs", "Czech"),
            ("cze", "Czech"),
            ("ces", "Czech"),
            ("tr", "Turkish"),
            ("tur", "Turkish"),
        ];
        for (code, expected) in cases {
            assert_eq!(
                lang_code_to_name(code),
                *expected,
                "lang_code_to_name({:?})",
                code
            );
        }
    }

    #[test]
    fn disconnect_remote_is_a_no_op_for_a_local_player() {
        let status = Arc::new(Mutex::new(PlayerStatus::default()));
        let proxy = PlayerProxy::stub(status);
        assert!(!proxy.is_remote());

        proxy.disconnect_remote(); // must not panic
    }

    #[test]
    fn disconnect_remote_disconnects_a_remote_player() {
        let (remote, _event_rx) = crate::remote_player::RemotePlayer::stub(Vec::new(), 0);
        let proxy = PlayerProxy {
            always_play_next: false,
            status: remote.status.clone(),
            subtitle_prefs: remote.subtitle_prefs.clone(),
            inner: PlayerProxyInner::Remote(remote),
        };
        assert!(proxy.is_remote());

        proxy.disconnect_remote(); // must not panic; a stub has no real
                                   // socket, so this only exercises the
                                   // dispatch, not the shutdown itself
                                   // (that's covered by Task 2's tests).
    }
}
