use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use libmpv2::{EndFileReason, Format, Mpv, events::{Event, PropertyData}, mpv_end_file_reason};
use crate::api::{EmbyClient, MediaItem, TICKS_PER_SECOND};

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
    let val = if item.item_type == "Episode" && item.parent_index_number > 0 && item.index_number > 0 {
        format!("Season {}  Episode {}", item.parent_index_number, item.index_number)
    } else {
        String::new()
    };
    let _ = mpv.set_property("user-data/mbv/ep-tag", val.as_str());
}

#[derive(Clone, Default)]
pub struct SubtitlePrefs {
    pub mode: String,           // "Default"|"Always"|"Smart"|"OnlyForced"|"None"|"HearingImpaired"
    pub subtitle_lang: String,  // full language name, e.g. "English"
    pub audio_lang: String,     // full language name, e.g. "English"
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
    pub active: bool,
    pub title: String,
    pub audio_tracks: Vec<(i64, String)>, // (mpv id, label)
    pub sub_tracks: Vec<(i64, String, bool)>,  // (mpv id, label, forced)
    pub audio_id: i64,   // 0 = none/unknown
    pub audio_lang: String, // raw lang code of selected audio track, e.g. "en", "ru"
    pub sub_id: i64,    // 0 = off
    pub muted: bool,
    pub video_height: i64,  // 0 = no video / audio-only
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum PlayerEvent {
    Stopped { idx: usize, position_ticks: i64, played: bool, error: Option<String> },
    TrackChanged(usize),
    TrackCompleted { idx: usize, position_ticks: i64, played: bool, consume: bool },
    NextUpThreshold { series_id: String, season: i64, episode: i64 },
    NextUpPlay,
    PlaylistNextUp { next_idx: usize },
    /// Emitted by RemotePlayer when CtrlState arrives so App can sync player_tab.
    QueueUpdated { items: Vec<crate::api::MediaItem>, cursor: usize },
    /// Chapter API: playback entered the intro window.
    IntroStarted { intro_end_ticks: i64 },
    /// Chapter API: playback passed IntroEnd (or track changed).
    IntroEnded,
    /// Chapter API: user clicked the "Skip Intro" button in MPV.
    SkipIntroPlay,
    /// mpv exited on its own (user pressed q inside mpv, or mpv crashed).
    MpvQuit,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum PlayerCommand {
    TogglePause,
    JumpTo(usize),
    PlaylistRemove(usize),
    SetVolume(i64),
    Seek(f64),
    SeekAbsolute(f64),
    SetAudio(i64),
    SetSub(i64), // 0 = off
    SetSubtitlePrefs { mode: String, subtitle_lang: String, audio_lang: String },
    SetMute(bool),
    LoadNew { url: String, start_pos: f64, item: Box<MediaItem> },
    NextUpShow { item_id: String, show_title: String, ep_title: String, artist: String },
    NextUpDismiss,
    SkipIntroDismiss,
    ReplacePlaylist { items: Vec<MediaItem>, start_idx: usize },
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
    matches!(codec, "hdmv_pgs_subtitle" | "pgssub" | "dvd_subtitle" | "dvdsub" | "dvb_subtitle" | "xsub")
}

/// Returns true if `label` begins with or contains the full language name `lang_pref`
/// (case-insensitive). Used to match audio/subtitle track labels against a preferred language.
fn label_matches_lang(label: &str, lang_pref: &str) -> bool {
    if lang_pref.is_empty() { return false; }
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
        let current_matches = audio_tracks.iter()
            .find(|(id, _)| *id == audio_id)
            .is_some_and(|(_, l)| label_matches_lang(l, &prefs.audio_lang));
        if !current_matches {
            if let Some((id, _)) = audio_tracks.iter().find(|(_, l)| label_matches_lang(l, &prefs.audio_lang)) {
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
        "OnlyForced" => {
            sub_tracks.iter()
                .find(|(_, l, forced)| *forced && label_matches_lang(l, &prefs.subtitle_lang))
                .or_else(|| sub_tracks.iter().find(|(_, _, forced)| *forced))
                .map(|(id, _, _)| *id)
        }
        "Always" => {
            sub_tracks.iter()
                .find(|(_, l, _)| label_matches_lang(l, &prefs.subtitle_lang))
                .or_else(|| sub_tracks.first())
                .map(|(id, _, _)| *id)
        }
        "Smart" => {
            if !sub_pref.is_empty() && audio_lang_name == sub_pref {
                None
            } else {
                sub_tracks.iter()
                    .find(|(_, l, _)| label_matches_lang(l, &prefs.subtitle_lang))
                    .or_else(|| sub_tracks.first())
                    .map(|(id, _, _)| *id)
            }
        }
        "HearingImpaired" => {
            sub_tracks.iter()
                .find(|(_, l, _)| { let ll = l.to_lowercase(); ll.contains("sdh") || ll.contains(" cc") || ll.contains("(cc)") })
                .or_else(|| sub_tracks.iter().find(|(_, l, _)| label_matches_lang(l, &prefs.subtitle_lang)))
                .or_else(|| sub_tracks.first())
                .map(|(id, _, _)| *id)
        }
        _ => {
            // Unknown mode: treat like Default, don't interfere
            refresh_tracks(mpv, status);
            return;
        }
    };

    match sid {
        None    => { let _ = mpv.set_property("sid", "no".to_string()); status.lock().unwrap().sub_id = 0; }
        Some(id) => { let _ = mpv.set_property("sid", id); status.lock().unwrap().sub_id = id; }
    }

    refresh_tracks(mpv, status);
}

fn refresh_tracks(mpv: &Mpv, status: &Arc<Mutex<PlayerStatus>>) {
    let count: i64 = match mpv.get_property("track-list/count") {
        Ok(n) => n,
        Err(_) => return,
    };
    let mut audio: Vec<(i64, String)> = Vec::new();
    let mut subs:  Vec<(i64, String, bool)> = Vec::new();
    let mut audio_id:   i64    = 0;
    let mut audio_lang: String = String::new();
    let mut sub_id:     i64    = 0;

    for i in 0..count {
        let ttype: String = mpv.get_property(&format!("track-list/{i}/type")).unwrap_or_default();
        let id:    i64    = mpv.get_property(&format!("track-list/{i}/id")).unwrap_or(i + 1);
        let lang:  String = mpv.get_property(&format!("track-list/{i}/lang")).unwrap_or_default();
        let title: String = mpv.get_property(&format!("track-list/{i}/title")).unwrap_or_default();
        let codec: String = mpv.get_property(&format!("track-list/{i}/codec")).unwrap_or_default();
        let sel:   bool   = mpv.get_property(&format!("track-list/{i}/selected")).unwrap_or(false);

        match ttype.as_str() {
            "audio" => {
                if sel { audio_id = id; audio_lang = lang.clone(); }
                // Build label from lang+codec+channels to avoid scene-branded titles
                let ch: i64 = mpv.get_property(&format!("track-list/{i}/demux-channel-count")).unwrap_or(0);
                let name = lang_code_to_name(&lang);
                let label = if !name.is_empty() {
                    let mut parts = vec![name.to_string(), codec.to_uppercase()];
                    let ch_str = fmt_channels(ch);
                    if !ch_str.is_empty() { parts.push(ch_str.to_string()); }
                    parts.join(" ")
                } else if !title.is_empty() { title }
                else if !lang.is_empty() { lang.to_uppercase() }
                else { format!("#{}", i + 1) };
                audio.push((id, label));
            }
            "sub" if !is_image_sub(&codec) => {
                if sel { sub_id = id; }
                let forced: bool = mpv.get_property(&format!("track-list/{i}/forced")).unwrap_or(false);
                let name = lang_code_to_name(&lang);
                let base_label = if !title.is_empty() { title.clone() }
                    else if !name.is_empty() { name.to_string() }
                    else if !lang.is_empty() { lang.to_uppercase() }
                    else { format!("#{}", i + 1) };
                let label = if forced { format!("{base_label} (Forced)") } else { base_label };
                subs.push((id, label, forced));
            }
            _ => {}
        }
    }

    let mut s = status.lock().unwrap();
    s.audio_tracks = audio;
    s.sub_tracks   = subs;
    s.audio_id     = audio_id;
    s.audio_lang   = audio_lang;
    s.sub_id       = sub_id;
}

// ── Session infrastructure ────────────────────────────────────────────────────

struct ProgressGuard {
    stop_tx: mpsc::Sender<()>,
    handle:  Option<thread::JoinHandle<()>>,
}

impl ProgressGuard {
    fn stop_and_join(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(h) = self.handle.take() { let _ = h.join(); }
    }
}

struct MpvSessionConfig {
    headless:          bool,
    use_mpv_config:    bool,
    no_scripts:        bool,
    always_skip_intro: bool,
}

// Shared between the event loop thread and the progress reporter thread.
// All mutable fields are Arc-wrapped so transitions are visible to both.
#[derive(Clone)]
struct SessionReporter {
    client:   Arc<EmbyClient>,
    ws_tx:    Option<mpsc::Sender<String>>,
    // (item_id, msid, sid) in a single lock so progress and event-loop threads never
    // observe a torn triple during item transitions.
    ids:      Arc<Mutex<(String, String, String)>>,
    // Shared with progress thread so it knows whether to send progress or just ping.
    is_audio: Arc<AtomicBool>,
    status:   Arc<Mutex<PlayerStatus>>,
}

impl SessionReporter {
    fn new(
        client:   Arc<EmbyClient>,
        ws_tx:    Option<mpsc::Sender<String>>,
        item_id:  String,
        msid:     String,
        sid:      String,
        is_audio: bool,
        status:   Arc<Mutex<PlayerStatus>>,
    ) -> Self {
        SessionReporter {
            client,
            ws_tx,
            ids:      Arc::new(Mutex::new((item_id, msid, sid))),
            is_audio: Arc::new(AtomicBool::new(is_audio)),
            status,
        }
    }

    // Selects ws or http automatically; reads pos/paused from status.
    // Recovers from poisoned mutexes so the progress thread never panics
    // while holding a lock.
    fn report_progress(&self, event_name: &str) {
        let (id, msid, sid) = self.ids.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let (pos, runtime, paused) = {
            let s = self.status.lock().unwrap_or_else(|e| e.into_inner());
            (s.position_ticks, s.runtime_ticks, s.paused)
        };
        if let Some(ref tx) = self.ws_tx {
            self.client.report_progress_ws(&id, &msid, pos, runtime, paused, &sid, event_name, tx);
        } else {
            self.client.report_progress_http(&id, &msid, pos, paused, &sid, event_name);
        }
    }

    // Zeroes position for audio items so Emby doesn't resume audio from mid-track.
    fn report_stopped(&self, last_valid_pos: i64) -> bool {
        let (id, msid, sid) = self.ids.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let is_audio = self.is_audio.load(Ordering::Relaxed);
        let pos = if is_audio { 0 } else { last_valid_pos };
        let runtime_ticks = self.status.lock().unwrap_or_else(|e| e.into_inner()).runtime_ticks;
        log::info!(target: "player", "report_stopped: item={id} is_audio={is_audio} last_valid_pos={}s sending pos={}s",
            last_valid_pos / TICKS_PER_SECOND, pos / TICKS_PER_SECOND);
        self.client.report_stopped(&id, &msid, pos, &sid, runtime_ticks)
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
        let (new_sid, new_msid, ext_sub_urls) = self.client.get_playback_info(&item.id);
        // Update ids before report_start so the progress reporter (which reads
        // ids on a 10-second timer) always sees the new item.
        {
            let mut ids = self.ids.lock().unwrap_or_else(|e| e.into_inner());
            ids.0 = item.id.clone();
            ids.1 = new_msid.clone();
            ids.2 = new_sid.clone();
        }
        self.is_audio.store(item.is_audio(), Ordering::Relaxed);
        let ok = self.client.report_start(item, &new_msid, &new_sid);
        (ext_sub_urls, ok)
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

fn init_mpv(config: &MpvSessionConfig) -> Result<Mpv, String> {
    let ipc_path = crate::config::mpv_ipc_path();
    let ipc_existed = std::path::Path::new(&ipc_path).exists();
    if ipc_existed {
        let _ = std::fs::remove_file(&ipc_path);
        log::info!(target: "player", "init: removed stale ipc socket {}", ipc_path);
    }
    log::info!(target: "player", "init: ipc={} (existed={})", ipc_path, ipc_existed);

    let no_scripts     = config.no_scripts;
    let use_mpv_config = config.use_mpv_config;
    let mut init_err: Option<String> = None;
    let mpv = match Mpv::with_initializer(|init| {
        macro_rules! opt {
            ($k:expr, $v:expr) => {{
                let r = init.set_option($k, $v);
                if let Err(ref e) = r { init_err = Some(format!("[player] set_option('{}') failed: {}", $k, mpv_err_str(e))); }
                r?;
            }};
        }
        opt!("config", "yes");
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
            let msg = init_err.unwrap_or_else(|| format!("[player] mpv init error: {}", mpv_err_str(&e)));
            return Err(msg);
        }
    };

    unsafe {
        libmpv2_sys::mpv_request_log_messages(mpv.ctx.as_ptr(), c"warn".as_ptr() as _);
    }

    // Set after init so user's mpv.conf cannot override these.
    if config.headless {
        let _ = mpv.set_property("vo", "null");
        let _ = mpv.set_property("force-window", "no");
    }

    Ok(mpv)
}

fn init_volume(mpv: &Mpv, status: &Arc<Mutex<PlayerStatus>>, initial_volume: u8) {
    let mut st    = status.lock().unwrap();
    let raw_max   = mpv.get_property::<i64>("volume-max").unwrap_or(130);
    st.volume_max = raw_max * raw_max / 100;
    let v   = (initial_volume as i64).clamp(0, st.volume_max);
    let raw = (10.0 * (v as f64).sqrt()).round() as i64;
    let _   = mpv.set_property("volume", raw as f64);
    st.volume = v;
}

fn observe_properties(mpv: &Mpv, use_mpv_config: bool) {
    let _ = mpv.observe_property("time-pos",      Format::Double, 0);
    let _ = mpv.observe_property("pause",         Format::Flag,   1);
    let _ = mpv.observe_property("volume",        Format::Double, 2);
    let _ = mpv.observe_property("sid",           Format::String, 3);
    let _ = mpv.observe_property("mute",          Format::Flag,   4);
    let _ = mpv.observe_property("aid",           Format::String, 5);
    let _ = mpv.observe_property("video-params/h",Format::Int64,  6);
    if use_mpv_config {
        let _ = mpv.command("keybind", &["MOUSE_MOVE", "script-message mouse-moved"]);
    }
}

fn spawn_progress_reporter(reporter: SessionReporter) -> ProgressGuard {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let handle = thread::spawn(move || {
        loop {
            match stop_rx.recv_timeout(Duration::from_secs(10)) {
                Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if reporter.is_audio.load(Ordering::Relaxed) {
                        reporter.report_ping();
                    } else {
                        reporter.report_progress("TimeUpdate");
                        reporter.report_ping();
                    }
                }
            }
        }
    });
    ProgressGuard { stop_tx, handle: Some(handle) }
}

fn load_intro_times(client: &EmbyClient, item_id: &str) -> (i64, i64) {
    if client.chapter_api_available {
        client.get_intro_times(item_id).unwrap_or((0, 0))
    } else {
        (0, 0)
    }
}

fn handle_intro(
    ticks:       i64,
    start:       i64,
    end:         i64,
    show_fired:  &mut bool,
    hide_fired:  &mut bool,
    always_skip: bool,
    mpv:         &Mpv,
    event_tx:    &mpsc::Sender<PlayerEvent>,
) {
    if end <= start { return; }
    if !*show_fired && ticks >= start {
        *show_fired = true;
        if ticks < end {
            let end_secs = end as f64 / TICKS_PER_SECOND as f64;
            if always_skip {
                let _ = mpv.set_property("time-pos", end_secs);
            } else {
                let _ = event_tx.send(PlayerEvent::IntroStarted { intro_end_ticks: end });
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

// ── SingleSession ─────────────────────────────────────────────────────────────

struct SingleSession {
    config:   MpvSessionConfig,
    reporter: SessionReporter,
    event_tx: mpsc::Sender<PlayerEvent>,
    status:   Arc<Mutex<PlayerStatus>>,
    subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    ext_sub_urls: Vec<String>,
    // loop state
    quit_at:             Option<Instant>,
    stop_reported:       bool,
    stopped_event_sent:  bool,
    mark_played_id:      Option<String>,
    pending_load:        bool,
    pending_resume_secs: Option<f64>,
    last_seek_at:        Option<Instant>,
    tracks_initialized:  bool,
    osd_title:           String,
    last_mouse_osd:      Option<Instant>,
    last_valid_pos:      i64,
    // next-up
    series_id:     String,
    season:        i64,
    episode:       i64,
    next_up_fired: bool,
    next_up_armed: bool,
    // intro
    intro_start: i64,
    intro_end:   i64,
    intro_show:  bool,
    intro_hide:  bool,
}

impl SingleSession {
    fn new(
        item:     &MediaItem,
        reporter: SessionReporter,
        config:   MpvSessionConfig,
        status:   Arc<Mutex<PlayerStatus>>,
        event_tx: mpsc::Sender<PlayerEvent>,
        subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
        ext_sub_urls: Vec<String>,
    ) -> Self {
        let is_audio       = item.is_audio();
        let last_valid_pos = if is_audio { 0 } else { item.playback_position_ticks };
        let (intro_start, intro_end) = load_intro_times(&reporter.client, &item.id);
        let past = intro_end > 0 && last_valid_pos >= intro_end;
        SingleSession {
            osd_title: item.display_name(),
            series_id: if item.item_type == "Episode" { item.series_id.clone() } else { String::new() },
            season:    item.parent_index_number,
            episode:   item.index_number,
            last_valid_pos,
            intro_start,
            intro_end,
            intro_show: past,
            intro_hide: past,
            config,
            reporter,
            event_tx,
            status,
            subtitle_prefs,
            ext_sub_urls,
            quit_at:             None,
            stop_reported:       false,
            stopped_event_sent:  false,
            mark_played_id:      None,
            pending_load:        false,
            pending_resume_secs: None,
            last_seek_at:        None,
            tracks_initialized:  false,
            last_mouse_osd:      None,
            next_up_fired:       false,
            next_up_armed:       false,
        }
    }

    fn set_intro(&mut self, start: i64, end: i64, pos: i64) {
        self.intro_start = start;
        self.intro_end   = end;
        let past = end > 0 && pos >= end;
        self.intro_show  = past;
        self.intro_hide  = past;
    }

    // Returns true if a pending quit should be cancelled (LoadNew arrived).
    fn handle_command(&mut self, cmd: PlayerCommand, mpv: &Mpv, progress: &mut ProgressGuard) -> bool {
        let mut cancel_stop = false;
        match cmd {
            PlayerCommand::NextUpShow { item_id, show_title, ep_title, artist } => {
                log::warn!(target: "player", "next-up: sending script-message mbv-next-up id={item_id} show={show_title} ep={ep_title}");
                let r = mpv.command("script-message", &["mbv-next-up", &item_id, &show_title, &ep_title, &artist]);
                log::warn!(target: "player", "next-up: script-message result={r:?}");
            }
            PlayerCommand::TogglePause => {
                let p = self.status.lock().unwrap().paused;
                let _ = mpv.set_property("pause", !p);
            }
            PlayerCommand::SetVolume(v) => {
                let vol_max = self.status.lock().unwrap().volume_max;
                let v = v.clamp(0, vol_max);
                let raw = (10.0 * (v as f64).sqrt()).round() as i64;
                let _ = mpv.set_property("volume", raw as f64);
                self.status.lock().unwrap().volume = v;
                let _ = mpv.command("show-text", &[&format!("Volume: {v}%"), "1500"]);
            }
            PlayerCommand::NextUpDismiss => {
                let _ = mpv.command("script-message", &["mbv-next-up-dismiss"]);
            }
            PlayerCommand::SkipIntroDismiss => {
                let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
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
                if id > 0 { let _ = mpv.set_property("aid", id); }
                else       { let _ = mpv.set_property("aid", "no".to_string()); }
                self.status.lock().unwrap().audio_id = id;
                refresh_tracks(mpv, &self.status);
            }
            PlayerCommand::SetSub(id) => {
                if id == 0 { let _ = mpv.set_property("sid", "no".to_string()); }
                else        { let _ = mpv.set_property("sid", id); }
                refresh_tracks(mpv, &self.status);
                self.status.lock().unwrap().sub_id = id;
            }
            PlayerCommand::SetSubtitlePrefs { mode, subtitle_lang, audio_lang } => {
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
            PlayerCommand::LoadNew { url, start_pos, item } => {
                // Cancel any pending quit so the new file loads in the same window.
                cancel_stop = true;
                self.quit_at = None;
                // Stop progress reporter during transition to prevent stale reports.
                progress.stop_and_join();
                self.ext_sub_urls = self.reporter.transition_to(&item, self.last_valid_pos);
                *progress = spawn_progress_reporter(self.reporter.clone());

                self.last_valid_pos = item.playback_position_ticks;
                self.osd_title      = item.display_name();
                {
                    let mut st = self.status.lock().unwrap();
                    st.runtime_ticks  = item.runtime_ticks;
                    st.position_ticks = item.playback_position_ticks;
                    st.title          = self.osd_title.clone();
                }
                self.tracks_initialized = false;
                self.stop_reported  = false;
                self.pending_load   = true;
                self.next_up_fired  = false;
                self.next_up_armed  = false;
                if item.item_type == "Episode" {
                    self.series_id = item.series_id.clone();
                    self.season    = item.parent_index_number;
                    self.episode   = item.index_number;
                } else {
                    self.series_id = String::new();
                }
                let (s, e) = load_intro_times(&self.reporter.client, &item.id);
                self.set_intro(s, e, item.playback_position_ticks);

                if start_pos > 0.0 {
                    let _ = mpv.set_property("start", format!("{:.0}", start_pos));
                } else {
                    let _ = mpv.set_property("start", "0");
                }
                let title_opt = mpv_title_opt(&item.display_name());
                log::info!(target: "player", "loadfile url={url} opts={title_opt:?}");
                if let Err(e) = mpv.command("loadfile", &[url.as_str(), "replace", "-1", title_opt.as_str()]) {
                    log::warn!(target: "player", "loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e));
                }
                send_ep_info(mpv, &item);
                let _ = mpv.command("script-message", &["mbv-next-up-dismiss"]);
                let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
            }
            // Not applicable in single-item mode
            PlayerCommand::JumpTo(_) | PlayerCommand::PlaylistRemove(_) | PlayerCommand::ReplacePlaylist { .. } => {}
        }
        cancel_stop
    }

    fn on_time_pos(&mut self, pos_secs: f64, mpv: &Mpv) {
        let ticks = (pos_secs * TICKS_PER_SECOND as f64) as i64;
        {
            let mut st = self.status.lock().unwrap();
            st.position_ticks = ticks;
            if pos_secs > 0.0 {
                if self.last_valid_pos == 0 {
                    log::info!(target: "player", "last_valid_pos first non-zero: {}s", ticks / TICKS_PER_SECOND);
                }
                self.last_valid_pos = ticks;
                st.last_valid_pos = ticks;
            }
        }

        const NEXT_UP_TICKS: i64 = 60 * TICKS_PER_SECOND;
        if !self.next_up_fired {
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
                        season:    self.season,
                        episode:   self.episode,
                    });
                } else if !self.next_up_armed && ticks > 0 && ticks < TICKS_PER_SECOND * 5 {
                    self.next_up_armed = true;
                    log::info!(target: "player", "next-up: armed series={} runtime={}s", self.series_id, runtime / TICKS_PER_SECOND);
                }
            }
        }

        handle_intro(
            ticks,
            self.intro_start, self.intro_end,
            &mut self.intro_show, &mut self.intro_hide,
            self.config.always_skip_intro,
            mpv, &self.event_tx,
        );
    }

    fn on_playback_restart(&mut self, mpv: &Mpv) {
        let event_name: &str;
        if !self.tracks_initialized {
            let prefs = self.subtitle_prefs.lock().unwrap().clone();
            for url in &self.ext_sub_urls {
                if let Err(e) = mpv.command("sub-add", &[url.as_str()]) {
                    log::warn!(target: "player", "sub-add failed: {url}: {e:?}");
                }
            }
            auto_select_tracks(mpv, &self.status, &prefs);
            self.tracks_initialized = true;
            let val = if self.season > 0 && self.episode > 0 {
                format!("Season {}  Episode {}", self.season, self.episode)
            } else {
                String::new()
            };
            let _ = mpv.set_property("user-data/mbv/ep-tag", val.as_str());
            let _ = mpv.set_property("start", "0");
            if let Some(secs) = self.pending_resume_secs.take() {
                if secs > 0.0 {
                    let _ = mpv.command("seek", &[&format!("{secs:.0}"), "absolute"]);
                    self.last_seek_at = Some(Instant::now());
                }
            }
            if self.config.use_mpv_config {
                let _ = mpv.command("show-text", &[&self.osd_title, "3000"]);
            }
            event_name = "TimeUpdate";
        } else {
            // Any restart after init means a seek happened (via TUI or mpv OSC).
            // Re-arm so a seek into/out-of the threshold is handled correctly.
            self.next_up_fired = false;
            self.next_up_armed = false;
            if self.last_seek_at.take().is_some() && self.config.use_mpv_config {
                let _ = mpv.command("show-text", &[&self.osd_title, "2000"]);
            }
            event_name = "Seek";
        }
        let seek_settled = self.last_seek_at.is_none_or(|t| t.elapsed() > Duration::from_millis(500));
        if self.quit_at.is_none() && seek_settled {
            self.last_seek_at = None;
            self.reporter.report_progress(event_name);
        }
    }

    // Returns true if the event loop should `continue` (skip the rest of this iteration).
    fn on_end_file(&mut self, reason: EndFileReason, progress: &mut ProgressGuard) -> bool {
        if self.quit_at.is_some() { return true; }
        if self.pending_load { self.pending_load = false; return true; }

        if reason == mpv_end_file_reason::Error {
            log::warn!(target: "player", "EndFile: playback error (file may be unreadable or format unsupported)");
        }

        let id = self.reporter.ids.lock().unwrap().0.clone();
        let natural_end = reason == mpv_end_file_reason::Eof
            && self.status.lock().unwrap().runtime_ticks > 0;

        progress.stop_and_join();
        self.reporter.report_stopped(self.last_valid_pos);
        self.stop_reported = true;

        if natural_end {
            if !self.reporter.is_audio.load(Ordering::Relaxed) {
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
                played: !self.reporter.is_audio.load(Ordering::Relaxed),
                error: None,
            });
            self.stopped_event_sent = true;
        }
        false
    }

    fn on_shutdown(&mut self, progress: &mut ProgressGuard) {
        if !self.stop_reported {
            progress.stop_and_join();
            self.reporter.report_stopped(self.last_valid_pos);
            self.stop_reported = true;
        }
        let client = self.reporter.client.clone();
        // Retry mark_played in a detached thread so Shutdown never blocks.
        if let Some(mid) = self.mark_played_id.take() {
            retry_mark_played(client.clone(), mid);
        }
        let runtime  = self.status.lock().unwrap().runtime_ticks;
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
                error: None,
            });
        }
        // mpv exited on its own (not via our stop command) — tell the app to quit.
        if self.quit_at.is_none() {
            let _ = self.event_tx.send(PlayerEvent::MpvQuit);
        }
    }

    fn run(mut self, mpv: Mpv, stop_rx: mpsc::Receiver<()>, cmd_rx: mpsc::Receiver<PlayerCommand>, mut progress: ProgressGuard) {
        let event_tx_panic = self.event_tx.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        loop {
            // Process commands before checking the stop signal so that a LoadNew
            // arriving in the same iteration (e.g. WS Stop then Play) can cancel
            // the quit instead of fighting with it.
            let mut cancel_stop = false;
            while let Ok(cmd) = cmd_rx.try_recv() {
                cancel_stop |= self.handle_command(cmd, &mpv, &mut progress);
            }

            if !cancel_stop && self.quit_at.is_none() && stop_rx.try_recv().is_ok() {
                let _ = mpv.command("quit", &[]);
                self.quit_at = Some(Instant::now());
            }

            if self.quit_at.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) {
                if !self.stop_reported {
                    progress.stop_and_join();
                    self.reporter.report_stopped(self.last_valid_pos);
                }
                let runtime  = self.status.lock().unwrap().runtime_ticks;
                let is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
                let near_end = !is_audio && runtime > 0 && self.last_valid_pos * 20 / runtime >= 19;
                self.status.lock().unwrap().active = false;
                let _ = self.event_tx.send(PlayerEvent::Stopped {
                    idx: 0,
                    position_ticks: self.last_valid_pos,
                    played: near_end,
                    error: None,
                });
                return;
            }

            match mpv.wait_event(0.5) {
                Some(Ok(Event::PropertyChange { name: "volume", change: PropertyData::Double(vol), .. })) => {
                    self.status.lock().unwrap().volume = (vol * vol / 100.0) as i64;
                }
                Some(Ok(Event::PropertyChange { change: PropertyData::Double(pos_secs), .. })) => {
                    self.on_time_pos(pos_secs, &mpv);
                }
                Some(Ok(Event::PropertyChange { name: "pause", change: PropertyData::Flag(paused), .. })) => {
                    self.status.lock().unwrap().paused = paused;
                    if self.quit_at.is_none() {
                        let event_name = if paused { "Pause" } else { "Unpause" };
                        self.reporter.report_progress(event_name);
                    }
                }
                Some(Ok(Event::PropertyChange { name: "sid", change: PropertyData::Str(s), .. })) => {
                    self.status.lock().unwrap().sub_id = s.parse::<i64>().unwrap_or(0);
                }
                Some(Ok(Event::PropertyChange { name: "aid", change: PropertyData::Str(_), .. })) => {
                    refresh_tracks(&mpv, &self.status);
                }
                Some(Ok(Event::PropertyChange { name: "mute", change: PropertyData::Flag(m), .. })) => {
                    self.status.lock().unwrap().muted = m;
                }
                Some(Ok(Event::PropertyChange { name: "video-params/h", change: PropertyData::Int64(h), .. })) => {
                    self.status.lock().unwrap().video_height = h;
                }
                Some(Ok(Event::PlaybackRestart)) => {
                    self.on_playback_restart(&mpv);
                }
                Some(Ok(Event::EndFile(reason))) => {
                    if self.on_end_file(reason, &mut progress) { continue; }
                }
                Some(Ok(Event::LogMessage { prefix, level, text, .. })) => {
                    let t = text.trim_end();
                    if !t.is_empty() {
                        log::warn!(target: "mpv", "[{}/{}] {}", prefix, level, t);
                    }
                }
                Some(Ok(Event::ClientMessage(args))) if args.first().copied() == Some("mbv-next-up-play") => {
                    log::info!(target: "player", "next-up: mbv-next-up-play received from Lua");
                    let _ = self.event_tx.send(PlayerEvent::NextUpPlay);
                }
                Some(Ok(Event::ClientMessage(args))) if args.first().copied() == Some("mbv-skip-intro-play") => {
                    let _ = self.event_tx.send(PlayerEvent::SkipIntroPlay);
                }
                Some(Ok(Event::ClientMessage(args))) if self.config.use_mpv_config && args.first().copied() == Some("mouse-moved") => {
                    let show = self.last_mouse_osd.is_none_or(|t: Instant| t.elapsed() > Duration::from_secs(3));
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
            let msg = panic.downcast_ref::<&str>().map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            log::error!(target: "player", "SingleSession panicked: {msg}");
            let _ = event_tx_panic.send(PlayerEvent::Stopped {
                idx: 0,
                position_ticks: 0,
                played: false,
                error: Some(msg),
            });
        }
    }
}

// ── PlaylistSession ───────────────────────────────────────────────────────────

struct PlaylistSession {
    config:     MpvSessionConfig,
    reporter:   SessionReporter,
    event_tx:   mpsc::Sender<PlayerEvent>,
    status:     Arc<Mutex<PlayerStatus>>,
    subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    server_url: String,
    token:      String,
    items:      Vec<MediaItem>,
    n:          usize,
    ext_sub_urls: Vec<String>,
    // loop state
    current_idx:            usize,
    forced_idx:             Option<usize>,
    quit_at:                Option<Instant>,
    last_seek_at:           Option<Instant>,
    last_valid_pos:         i64,
    tracks_initialized:     bool,
    playlist_cancelled:     bool,
    pending_load:           u8,
    pending_initial_jump:   bool,
    stop_reported:          bool,
    osd_title:              String,
    last_mouse_osd:         Option<Instant>,
    pending_resume_secs:    Option<f64>,
    playlist_next_up_fired: bool,
    playlist_next_up_armed: bool,
    next_up_jump:           bool,
    stopped_near_end:       bool,
    // intro
    intro_start: i64,
    intro_end:   i64,
    intro_show:  bool,
    intro_hide:  bool,
}

impl PlaylistSession {
    fn new(
        items:        Vec<MediaItem>,
        start_idx:    usize,
        reporter:     SessionReporter,
        config:       MpvSessionConfig,
        status:       Arc<Mutex<PlayerStatus>>,
        event_tx:     mpsc::Sender<PlayerEvent>,
        subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
        server_url:   String,
        token:        String,
        ext_sub_urls: Vec<String>,
    ) -> Self {
        let n          = items.len();
        let initial_pos = items[start_idx].playback_position_ticks;
        let (intro_start, intro_end) = load_intro_times(&reporter.client, &items[start_idx].id);
        let past = intro_end > 0 && initial_pos >= intro_end;
        let pending_resume_secs = if !items[start_idx].is_audio() && items[start_idx].should_resume() {
            Some(items[start_idx].resume_seconds())
        } else {
            None
        };
        log::info!(target: "player", "playlist init idx={start_idx} item_pos={}s pending_resume={pending_resume_secs:?}s",
            initial_pos / crate::api::TICKS_PER_SECOND);
        let osd_title = items[start_idx].display_name();
        PlaylistSession {
            config,
            reporter,
            event_tx,
            status,
            subtitle_prefs,
            server_url,
            token,
            n,
            ext_sub_urls,
            current_idx:            start_idx,
            forced_idx:             None,
            quit_at:                None,
            last_seek_at:           None,
            last_valid_pos:         initial_pos,
            tracks_initialized:     false,
            playlist_cancelled:     false,
            pending_load:           0,
            pending_initial_jump:   start_idx > 0,
            stop_reported:          false,
            last_mouse_osd:         None,
            playlist_next_up_fired: false,
            playlist_next_up_armed: false,
            next_up_jump:           false,
            stopped_near_end:       false,
            intro_start,
            intro_end,
            intro_show: past,
            intro_hide: past,
            osd_title,
            pending_resume_secs,
            items,
        }
    }

    fn set_intro(&mut self, start: i64, end: i64, pos: i64) {
        self.intro_start = start;
        self.intro_end   = end;
        let past = end > 0 && pos >= end;
        self.intro_show  = past;
        self.intro_hide  = past;
    }

    fn handle_command(&mut self, cmd: PlayerCommand, mpv: &Mpv, progress: &mut ProgressGuard) -> bool {
        let mut cancel_stop = false;
        match cmd {
            PlayerCommand::NextUpShow { item_id, show_title, ep_title, artist } => {
                log::warn!(target: "player", "next-up: sending script-message mbv-next-up id={item_id} show={show_title} ep={ep_title}");
                let r = mpv.command("script-message", &["mbv-next-up", &item_id, &show_title, &ep_title, &artist]);
                log::warn!(target: "player", "next-up: script-message result={r:?}");
            }
            PlayerCommand::TogglePause => {
                let p = self.status.lock().unwrap().paused;
                let _ = mpv.set_property("pause", !p);
            }
            PlayerCommand::JumpTo(idx) => {
                if idx < self.n {
                    // mpv playlist indices match items indices directly.
                    self.forced_idx = Some(idx);
                    {
                        let mut s = self.status.lock().unwrap();
                        s.current_idx    = idx;
                        s.position_ticks = 0;
                        s.runtime_ticks  = self.items[idx].runtime_ticks;
                        s.title          = self.items[idx].display_name();
                    }
                    let _ = mpv.set_property("playlist-pos", idx as i64);
                }
            }
            PlayerCommand::PlaylistRemove(idx) => {
                if idx < self.n {
                    let _ = mpv.command("playlist-remove", &[&idx.to_string()]);
                    self.items.remove(idx);
                    self.n -= 1;
                    if idx < self.current_idx {
                        self.current_idx -= 1;
                        self.status.lock().unwrap().current_idx = self.current_idx;
                    }
                    if let Some(fi) = self.forced_idx {
                        self.forced_idx = if idx == fi { None }
                                          else if idx < fi { Some(fi - 1) }
                                          else { Some(fi) };
                    }
                    if idx == self.current_idx {
                        // Currently playing track removed — clear reporter item_id to prevent
                        // stale progress reports until on_end_file transitions to the next track.
                        let mut ids = self.reporter.ids.lock().unwrap();
                        ids.0.clear();
                    }
                }
            }
            PlayerCommand::NextUpDismiss => {
                let _ = mpv.command("script-message", &["mbv-next-up-dismiss"]);
            }
            PlayerCommand::SkipIntroDismiss => {
                let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
            }
            PlayerCommand::ReplacePlaylist { items: new_items, start_idx } => {
                cancel_stop = true;
                if new_items.is_empty() { return cancel_stop; }
                // report_stopped for current item; is_audio zeroing handled inside.
                self.reporter.report_stopped(self.last_valid_pos);
                self.stop_reported = true;

                let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
                // Remove all old playlist entries except the current one so that
                // the subsequent loadfile "replace" starts from a clean slate.
                // Without this, old entries remain and playlist-pos = start_idx
                // lands on a stale file instead of new_items[start_idx].
                let _ = mpv.command("playlist-clear", &[]);

                let start_idx = start_idx.min(new_items.len() - 1);
                for (i, item) in new_items.iter().enumerate() {
                    let ep = if item.is_audio() { "Audio" } else { "Videos" };
                    let url = format!("{}/{}/{}/stream?static=true&api_key={}", self.server_url, ep, item.id, self.token);
                    let mode = if i == 0 { "replace" } else { "append-play" };
                    let title_opt = mpv_title_opt(&item.display_name());
                    if let Err(e) = mpv.command("loadfile", &[url.as_str(), mode, "-1", title_opt.as_str()]) {
                        log::warn!(target: "player", "ReplacePlaylist loadfile error: {}", mpv_err_str(&e));
                    }
                }
                let _ = mpv.set_property("start", "0");
                send_ep_info(mpv, &new_items[start_idx]);
                // loadfile "replace" displaces the current file (EndFile #1).
                // If start_idx > 0 we also set playlist-pos which displaces item[0] (EndFile #2).
                // Use = not += so a stale pending_load from a prior operation never stacks.
                // Clear pending_initial_jump too since any in-flight initial jump is superseded.
                self.pending_initial_jump = false;
                self.pending_load = if start_idx > 0 { 2 } else { 1 };
                if start_idx > 0 {
                    let _ = mpv.set_property("playlist-pos", start_idx as i64);
                }

                self.last_valid_pos         = new_items[start_idx].playback_position_ticks;
                self.tracks_initialized     = false;
                // stop_reported stays true until pending_load drains to 0 in on_end_file,
                // preventing a duplicate report_stopped for the displaced file's EndFile(Quit).
                self.playlist_cancelled     = false;
                self.forced_idx             = None;
                self.playlist_next_up_fired = false;
                self.playlist_next_up_armed = false;
                self.next_up_jump           = false;
                self.osd_title              = new_items[start_idx].display_name();
                self.pending_resume_secs    = if !new_items[start_idx].is_audio() && new_items[start_idx].should_resume() {
                    Some(new_items[start_idx].resume_seconds())
                } else {
                    None
                };
                log::info!(target: "player", "playlist queue-replace idx={start_idx} pending_resume={:?}s", self.pending_resume_secs);
                let (s, e) = load_intro_times(&self.reporter.client, &new_items[start_idx].id);
                self.set_intro(s, e, new_items[start_idx].playback_position_ticks);

                // Stop progress reporter during transition to prevent stale reports,
                // then restart for the new item.
                progress.stop_and_join();
                let (_urls, ok) = self.reporter.start_item(&new_items[start_idx]);
                if !ok {
                    log::warn!(target: "player", "start_item failed for playlist replace item={}", new_items[start_idx].id);
                }
                *progress = spawn_progress_reporter(self.reporter.clone());
                self.n          = new_items.len();
                self.current_idx = start_idx;
                self.items      = new_items;
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
                if id > 0 { let _ = mpv.set_property("aid", id); }
                else       { let _ = mpv.set_property("aid", "no".to_string()); }
                self.status.lock().unwrap().audio_id = id;
                refresh_tracks(mpv, &self.status);
            }
            PlayerCommand::SetSub(id) => {
                if id == 0 { let _ = mpv.set_property("sid", "no".to_string()); }
                else        { let _ = mpv.set_property("sid", id); }
                refresh_tracks(mpv, &self.status);
                self.status.lock().unwrap().sub_id = id;
            }
            PlayerCommand::SetSubtitlePrefs { mode, subtitle_lang, audio_lang } => {
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
            PlayerCommand::LoadNew { url, start_pos, item } => {
                cancel_stop = true;
                self.quit_at          = None;
                self.playlist_cancelled = true;

                // Stop progress reporter during transition to prevent stale reports.
                progress.stop_and_join();
                self.ext_sub_urls = self.reporter.transition_to(&item, self.last_valid_pos);
                *progress = spawn_progress_reporter(self.reporter.clone());

                self.last_valid_pos    = item.playback_position_ticks;
                self.tracks_initialized = false;
                self.stop_reported     = false;
                self.pending_load      = 1;

                let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
                let (s, e) = load_intro_times(&self.reporter.client, &item.id);
                self.set_intro(s, e, item.playback_position_ticks);

                if start_pos > 0.0 {
                    let _ = mpv.set_property("start", format!("{:.0}", start_pos));
                } else {
                    let _ = mpv.set_property("start", "0");
                }
                let title_opt = mpv_title_opt(&item.display_name());
                log::info!(target: "player", "loadfile url={url} opts={title_opt:?}");
                if let Err(e) = mpv.command("loadfile", &[url.as_str(), "replace", "-1", title_opt.as_str()]) {
                    log::warn!(target: "player", "loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e));
                }
                send_ep_info(mpv, &item);
            }
        }
        cancel_stop
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

        // Playlist next-up: match Emby Web's timing from videoosd.js.
        // 60 s before end. Minimum episode: 10 min. Minimum remaining when shown: 20 s.
        const MIN_RUNTIME_TICKS: i64 = 600 * TICKS_PER_SECOND;
        const MIN_REMAIN_TICKS:  i64 = 20  * TICKS_PER_SECOND;
        if self.current_idx + 1 < self.items.len() {
            let runtime = self.status.lock().unwrap().runtime_ticks;
            if runtime > 0 {
                let show_at   = runtime - 60 * TICKS_PER_SECOND;
                let remaining = runtime - ticks;
                if self.playlist_next_up_fired && ticks < show_at {
                    self.playlist_next_up_fired = false;
                    self.playlist_next_up_armed = false;
                }
                if !self.playlist_next_up_fired && runtime >= MIN_RUNTIME_TICKS {
                    if remaining >= MIN_REMAIN_TICKS && ticks >= show_at {
                        self.playlist_next_up_fired = true;
                        let _ = self.event_tx.send(PlayerEvent::PlaylistNextUp { next_idx: self.current_idx + 1 });
                    } else if !self.playlist_next_up_armed && ticks > 0 && ticks < TICKS_PER_SECOND * 5 {
                        self.playlist_next_up_armed = true;
                        log::info!(target: "player", "playlist next-up armed idx={}", self.current_idx + 1);
                    }
                }
            }
        }

        handle_intro(
            ticks,
            self.intro_start, self.intro_end,
            &mut self.intro_show, &mut self.intro_hide,
            self.config.always_skip_intro,
            mpv, &self.event_tx,
        );
    }

    fn on_playback_restart(&mut self, mpv: &Mpv) {
        if self.pending_initial_jump {
            // mpv ignored playlist-pos before the event loop started; now that
            // playback is live (first PlaybackRestart), the jump is honored.
            self.pending_initial_jump = false;
            self.pending_load += 1;
            let _ = mpv.set_property("playlist-pos", self.current_idx as i64);
            // Skip normal handling; wait for the next PlaybackRestart (for start_idx item).
            return;
        }
        if !self.tracks_initialized {
            let prefs = self.subtitle_prefs.lock().unwrap().clone();
            for url in &self.ext_sub_urls {
                if let Err(e) = mpv.command("sub-add", &[url.as_str()]) {
                    log::warn!(target: "player", "sub-add failed: {url}: {e:?}");
                }
            }
            auto_select_tracks(mpv, &self.status, &prefs);
            self.tracks_initialized = true;
            send_ep_info(mpv, &self.items[self.current_idx]);
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
        } else if self.last_seek_at.take().is_some() && self.config.use_mpv_config {
            let _ = mpv.command("show-text", &[&self.osd_title, "2000"]);
        }
        let seek_settled = self.last_seek_at.is_none_or(|t| t.elapsed() > Duration::from_millis(500));
        if self.quit_at.is_none() && seek_settled {
            self.last_seek_at = None;
            if !self.reporter.is_audio.load(Ordering::Relaxed) {
                self.reporter.report_progress("TimeUpdate");
            }
        }
    }

    // Returns true if the event loop should `continue`.
    fn on_end_file(&mut self, reason: EndFileReason, mpv: &Mpv, progress: &mut ProgressGuard) -> bool {
        if self.quit_at.is_some() { return true; }
        if self.pending_load > 0 {
            self.pending_load -= 1;
            // Once all pending EndFiles from a ReplacePlaylist are drained, the new item's
            // lifecycle begins — reset stop_reported so on_end_file/on_shutdown can report it.
            if self.pending_load == 0 { self.stop_reported = false; }
            return true;
        }

        if reason == mpv_end_file_reason::Error {
            log::warn!(target: "player", "EndFile: playback error (file may be unreadable or format unsupported)");
        }

        let completed_is_audio = self.reporter.is_audio.load(Ordering::Relaxed);
        let runtime            = self.status.lock().unwrap().runtime_ticks;

        if self.playlist_cancelled || reason == mpv_end_file_reason::Quit {
            let natural_end = reason == mpv_end_file_reason::Eof && runtime > 0;
            let near_end    = !natural_end && !completed_is_audio
                && runtime > 0
                && self.last_valid_pos * 20 / runtime >= 19;
            log::warn!(target: "player", "quit path: last_valid_pos={} runtime={} pending_resume={} stop_reported={}",
                self.last_valid_pos, runtime, self.pending_resume_secs.is_some(), self.stop_reported);
            if !self.stop_reported {
                progress.stop_and_join();
                self.reporter.report_stopped(self.last_valid_pos);
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

        let completed_idx = self.current_idx;
        log::warn!(target: "player", "advance path: reason={reason:?} last_valid_pos={} runtime={} pending_resume={}",
            self.last_valid_pos, self.status.lock().unwrap().runtime_ticks, self.pending_resume_secs.is_some());
        // H11: bounds-check completed_idx — PlaylistRemove can shrink the list
        // while the current track is finishing.
        if completed_idx >= self.items.len() {
            log::warn!(target: "player", "on_end_file: completed_idx={completed_idx} out of bounds (len={}), stopping",
                self.items.len());
            progress.stop_and_join();
            self.status.lock().unwrap().active = false;
            self.reporter.report_stopped(self.last_valid_pos);
            let _ = self.event_tx.send(PlayerEvent::Stopped {
                idx: completed_idx.min(self.items.len().saturating_sub(1)),
                position_ticks: self.last_valid_pos,
                played: false,
                error: None,
            });
            return false;
        }
        let natural       = reason == mpv_end_file_reason::Eof
            && self.items[completed_idx].runtime_ticks > 0;
        let near_end = is_near_end(completed_is_audio, natural, self.last_valid_pos, self.items[completed_idx].runtime_ticks);
        let was_next_up   = std::mem::replace(&mut self.next_up_jump, false);
        let played_out    = (natural || near_end || was_next_up) && !completed_is_audio;
        let consume_track = (natural || near_end || was_next_up) && !completed_is_audio;
        let completed_pos = playlist_completed_pos(completed_is_audio, natural, near_end, self.last_valid_pos);

        let next_idx = self.forced_idx.take().unwrap_or(self.current_idx + 1);

        if next_idx >= self.n {
            progress.stop_and_join();
            self.status.lock().unwrap().active = false;
            self.reporter.report_stopped(completed_pos);
            if played_out {
                let id = self.items[completed_idx].id.clone();
                if let Err(e) = self.reporter.client.mark_played(&id) {
                    log::warn!(target: "player", "mark_played failed id={id}: {e}; scheduling retry");
                    retry_mark_played(self.reporter.client.clone(), id);
                }
            }
            let _ = self.event_tx.send(PlayerEvent::Stopped {
                idx: completed_idx,
                position_ticks: completed_pos,
                played: played_out,
                error: None,
            });
            return false; // signals run() to return
        }

        // Update UI to the next track immediately, before slow network calls.
        self.current_idx       = next_idx;
        self.last_valid_pos    = self.items[self.current_idx].playback_position_ticks;
        self.tracks_initialized = false;
        {
            let mut s = self.status.lock().unwrap();
            s.position_ticks = 0;
            s.runtime_ticks  = self.items[self.current_idx].runtime_ticks;
            s.current_idx    = self.current_idx;
            s.title          = self.items[self.current_idx].display_name();
        }

        self.reporter.report_stopped(completed_pos);
        if played_out {
            let id = self.items[completed_idx].id.clone();
            if let Err(e) = self.reporter.client.mark_played(&id) {
                log::warn!(target: "player", "mark_played failed id={id}: {e}; scheduling retry");
                retry_mark_played(self.reporter.client.clone(), id);
            }
        }

        let _ = mpv.set_property("start", "0");
        self.playlist_next_up_fired = false;
        self.playlist_next_up_armed = false;
        send_ep_info(mpv, &self.items[self.current_idx]);
        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);

        // Stop progress reporter during transition to prevent stale reports.
        progress.stop_and_join();
        let (_urls, ok) = self.reporter.start_item(&self.items[self.current_idx]);
        if !ok {
            log::warn!(target: "player", "start_item failed for playlist track-transition item={}", self.items[self.current_idx].id);
        }
        *progress = spawn_progress_reporter(self.reporter.clone());

        let (s, e) = load_intro_times(&self.reporter.client, &self.items[self.current_idx].id);
        self.set_intro(s, e, self.items[self.current_idx].playback_position_ticks);

        self.osd_title = self.items[self.current_idx].display_name();
        if !self.items[self.current_idx].is_audio() && self.items[self.current_idx].should_resume() {
            self.pending_resume_secs = Some(self.items[self.current_idx].resume_seconds());
        }
        log::info!(target: "player", "playlist track-transition idx={} pending_resume={:?}s", self.current_idx, self.pending_resume_secs);

        let _ = self.event_tx.send(PlayerEvent::TrackCompleted {
            idx: completed_idx,
            position_ticks: completed_pos,
            played: played_out,
            consume: consume_track,
        });
        let _ = self.event_tx.send(PlayerEvent::TrackChanged(self.current_idx));
        false
    }

    fn on_shutdown(&mut self, progress: &mut ProgressGuard) {
        log::warn!(target: "player", "shutdown: last_valid_pos={} stop_reported={} pending_resume={}",
            self.last_valid_pos, self.stop_reported, self.pending_resume_secs.is_some());
        if !self.stop_reported {
            progress.stop_and_join();
            self.reporter.report_stopped(self.last_valid_pos);
            self.stop_reported = true;
        }
        self.status.lock().unwrap().active = false;
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            idx: self.current_idx,
            position_ticks: self.last_valid_pos,
            played: self.stopped_near_end,
            error: None,
        });
        // mpv exited on its own (not via our stop command) — tell the app to quit.
        if self.quit_at.is_none() {
            let _ = self.event_tx.send(PlayerEvent::MpvQuit);
        }
    }

    fn run(mut self, mpv: Mpv, stop_rx: mpsc::Receiver<()>, cmd_rx: mpsc::Receiver<PlayerCommand>, mut progress: ProgressGuard) {
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

            if self.quit_at.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) {
                if !self.stop_reported {
                    progress.stop_and_join();
                    self.reporter.report_stopped(self.last_valid_pos);
                }
                self.status.lock().unwrap().active = false;
                let _ = self.event_tx.send(PlayerEvent::Stopped {
                    idx: self.current_idx,
                    position_ticks: self.last_valid_pos,
                    played: self.stopped_near_end,
                    error: None,
                });
                return;
            }

            match mpv.wait_event(0.5) {
                Some(Ok(Event::PropertyChange { name: "volume", change: PropertyData::Double(vol), .. })) => {
                    self.status.lock().unwrap().volume = (vol * vol / 100.0) as i64;
                }
                Some(Ok(Event::PropertyChange { change: PropertyData::Double(pos_secs), .. })) => {
                    self.on_time_pos(pos_secs, &mpv);
                }
                Some(Ok(Event::PropertyChange { name: "pause", change: PropertyData::Flag(paused), .. })) => {
                    self.status.lock().unwrap().paused = paused;
                    if self.quit_at.is_none() {
                        let event_name = if paused { "Pause" } else { "Unpause" };
                        self.reporter.report_progress(event_name);
                    }
                }
                Some(Ok(Event::PropertyChange { name: "sid", change: PropertyData::Str(s), .. })) => {
                    self.status.lock().unwrap().sub_id = s.parse::<i64>().unwrap_or(0);
                }
                Some(Ok(Event::PropertyChange { name: "aid", change: PropertyData::Str(_), .. })) => {
                    refresh_tracks(&mpv, &self.status);
                }
                Some(Ok(Event::PropertyChange { name: "mute", change: PropertyData::Flag(m), .. })) => {
                    self.status.lock().unwrap().muted = m;
                }
                Some(Ok(Event::PropertyChange { name: "video-params/h", change: PropertyData::Int64(h), .. })) => {
                    self.status.lock().unwrap().video_height = h;
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
                    if should_continue { continue; }
                }
                Some(Ok(Event::LogMessage { prefix, level, text, .. })) => {
                    let t = text.trim_end();
                    if !t.is_empty() {
                        log::warn!(target: "mpv", "[{}/{}] {}", prefix, level, t);
                    }
                }
                Some(Ok(Event::ClientMessage(args))) if args.first().copied() == Some("mbv-next-up-play") => {
                    log::info!(target: "player", "next-up: mbv-next-up-play received from Lua");
                    self.next_up_jump = true;
                    let _ = self.event_tx.send(PlayerEvent::NextUpPlay);
                }
                Some(Ok(Event::ClientMessage(args))) if args.first().copied() == Some("mbv-skip-intro-play") => {
                    let _ = self.event_tx.send(PlayerEvent::SkipIntroPlay);
                }
                Some(Ok(Event::ClientMessage(args))) if self.config.use_mpv_config && args.first().copied() == Some("mouse-moved") => {
                    let show = self.last_mouse_osd.is_none_or(|t: Instant| t.elapsed() > Duration::from_secs(3));
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
            let msg = panic.downcast_ref::<&str>().map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            log::error!(target: "player", "PlaylistSession panicked: {msg}");
            let _ = event_tx_panic.send(PlayerEvent::Stopped {
                idx: current_idx_panic,
                position_ticks: 0,
                played: false,
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
}

impl QuitHandle {
    pub fn stop(&self) {
        if let Some(tx) = self.stop_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
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
    is_playlist_mode: Arc<AtomicBool>,
    current_is_headless: Arc<AtomicBool>,
    pub event_tx: mpsc::Sender<PlayerEvent>,
    stop_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    pub cmd_tx: Arc<Mutex<Option<mpsc::Sender<PlayerCommand>>>>,
    pub status: Arc<Mutex<PlayerStatus>>,
    thread_handle: Mutex<Option<thread::JoinHandle<()>>>,
    ws_tx: Option<mpsc::Sender<String>>,
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
        ws_tx: Option<mpsc::Sender<String>>,
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
            is_playlist_mode: Arc::new(AtomicBool::new(false)),
            current_is_headless: Arc::new(AtomicBool::new(false)),
            event_tx,
            stop_tx: Arc::new(Mutex::new(None)),
            cmd_tx: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(PlayerStatus {
                position_ticks: 0,
                last_valid_pos: 0,
                runtime_ticks: 0,
                paused: false,
                volume: 100,
                volume_max: 130,
                current_idx: 0,
                active: false,
                title: String::new(),
                audio_tracks: Vec::new(),
                sub_tracks: Vec::new(),
                audio_id: 0,
                audio_lang: String::new(),
                sub_id: 0,
                muted: false,
                video_height: 0,
            })),
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

    pub fn play(&self, item: &MediaItem, client: Arc<EmbyClient>, initial_volume: u8) {
        // If a video is already playing, load the new file into the existing mpv window.
        if self.status.lock().unwrap().active {
            let ep = if item.is_audio() { "Audio" } else { "Videos" };
            let url = format!(
                "{}/{}/{}/stream?static=true&api_key={}",
                self.server_url, ep, item.id, self.token
            );
            let start_pos = if item.should_resume() { item.resume_seconds() } else { 0.0 };
            {
                let mut st = self.status.lock().unwrap();
                st.position_ticks = item.playback_position_ticks;
                st.runtime_ticks  = item.runtime_ticks;
                st.paused         = false;
                st.current_idx    = 0;
                st.title          = item.display_name();
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

        let item      = item.clone();
        let is_audio  = item.is_audio();
        let headless  = !self.show_audio_window && is_audio;
        let item_pos  = if is_audio { 0 } else { item.playback_position_ticks };
        let start_pos = if is_audio || !item.should_resume() { 0.0 } else { item.resume_seconds() };
        let ep        = if is_audio { "Audio" } else { "Videos" };
        let url       = format!("{}/{}/{}/stream?static=true&api_key={}", self.server_url, ep, item.id, self.token);
        let title     = item.display_name();

        let config = MpvSessionConfig {
            headless,
            use_mpv_config:    self.use_mpv_config,
            no_scripts:        self.no_scripts,
            always_skip_intro: self.always_skip_intro,
        };
        let status           = self.status.clone();
        let event_tx         = self.event_tx.clone();
        let ws_tx            = self.ws_tx.clone();
        let subtitle_prefs   = self.subtitle_prefs.clone();
        let is_playlist_mode = self.is_playlist_mode.clone();
        self.current_is_headless.store(headless, Ordering::Relaxed);

        {
            let mut st = status.lock().unwrap();
            st.position_ticks = item_pos;
            st.runtime_ticks  = item.runtime_ticks;
            st.paused         = false;
            st.current_idx    = 0;
            st.active         = true;
            st.title          = title.clone();
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *self.stop_tx.lock().unwrap() = Some(stop_tx);
        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        *self.cmd_tx.lock().unwrap() = Some(cmd_tx);

        let handle = thread::spawn(move || {
            is_playlist_mode.store(false, Ordering::Relaxed);

            let mpv = match init_mpv(&config) {
                Ok(m)  => m,
                Err(e) => { log::error!(target: "player", "{}", e); return; }
            };
            init_volume(&mpv, &status, initial_volume);

            if start_pos > 0.0 {
                let _ = mpv.set_property("start", format!("{:.0}", start_pos));
            }
            let title_opt = mpv_title_opt(&title);
            log::info!(target: "player", "loadfile url={url} opts={title_opt:?}");
            if let Err(e) = mpv.command("loadfile", &[url.as_str(), "replace", "-1", title_opt.as_str()]) {
                log::warn!(target: "player", "loadfile error: {} | url={url} opts={title_opt:?}", mpv_err_str(&e));
                return;
            }
            send_ep_info(&mpv, &item);
            observe_properties(&mpv, config.use_mpv_config);

            let (sid, msid, ext_sub_urls) = client.get_playback_info(&item.id);
            client.report_start(&item, &msid, &sid);
            let reporter = SessionReporter::new(
                client, ws_tx, item.id.clone(), msid, sid, is_audio, status.clone(),
            );
            let progress = spawn_progress_reporter(reporter.clone());
            let session  = SingleSession::new(&item, reporter, config, status, event_tx, subtitle_prefs, ext_sub_urls);
            session.run(mpv, stop_rx, cmd_rx, progress);
        });
        *self.thread_handle.lock().unwrap() = Some(handle);
    }

    pub fn play_playlist(&self, items: Vec<MediaItem>, start_idx: usize, client: Arc<EmbyClient>, initial_volume: u8) {
        if items.is_empty() { return; }

        let new_is_headless = !self.show_audio_window
            && items.iter().all(|i| i.media_type == "Audio" || i.item_type == "Audio");

        // If playlist loop already running, replace in place (no window close),
        // unless mpv was spawned headless but new items need a window.
        if self.status.lock().unwrap().active
            && self.is_playlist_mode.load(Ordering::Relaxed)
            && (!self.current_is_headless.load(Ordering::Relaxed) || new_is_headless)
        {
            let start_idx = start_idx.min(items.len() - 1);
            {
                let mut st = self.status.lock().unwrap();
                st.position_ticks = items[start_idx].playback_position_ticks;
                st.runtime_ticks  = items[start_idx].runtime_ticks;
                st.paused         = false;
                st.current_idx    = start_idx;
                st.title          = items[start_idx].display_name();
            }
            self.send_command(PlayerCommand::ReplacePlaylist { items, start_idx });
            return;
        }

        self.stop();
        self.join();

        let start_idx = start_idx.min(items.len() - 1);
        let headless  = new_is_headless;

        let config = MpvSessionConfig {
            headless,
            use_mpv_config:    self.use_mpv_config,
            no_scripts:        self.no_scripts,
            always_skip_intro: self.always_skip_intro,
        };
        let status           = self.status.clone();
        let event_tx         = self.event_tx.clone();
        let ws_tx            = self.ws_tx.clone();
        let subtitle_prefs   = self.subtitle_prefs.clone();
        let is_playlist_mode = self.is_playlist_mode.clone();
        let server_url       = self.server_url.clone();
        let token            = self.token.clone();
        self.current_is_headless.store(headless, Ordering::Relaxed);

        {
            let mut st = status.lock().unwrap();
            st.position_ticks = 0;
            st.runtime_ticks  = items[start_idx].runtime_ticks;
            st.paused         = false;
            st.current_idx    = start_idx;
            st.active         = true;
            st.title          = items[start_idx].display_name();
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *self.stop_tx.lock().unwrap() = Some(stop_tx);
        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        *self.cmd_tx.lock().unwrap() = Some(cmd_tx);

        let handle = thread::spawn(move || {
            is_playlist_mode.store(true, Ordering::Relaxed);

            let mpv = match init_mpv(&config) {
                Ok(m)  => m,
                Err(e) => { log::error!(target: "player", "{}", e); return; }
            };
            init_volume(&mpv, &status, initial_volume);

            // Load the full playlist into mpv so every index matches items[i] directly.
            for (i, item) in items.iter().enumerate() {
                let ep = if item.is_audio() { "Audio" } else { "Videos" };
                let url = format!("{}/{}/{}/stream?static=true&api_key={}", server_url, ep, item.id, token);
                let mode = if i == 0 { "replace" } else { "append-play" };
                let title_opt = mpv_title_opt(&item.display_name());
                if let Err(e) = mpv.command("loadfile", &[url.as_str(), mode, "-1", title_opt.as_str()]) {
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

            let (sid, msid, ext_sub_urls) = client.get_playback_info(&items[start_idx].id);
            client.report_start(&items[start_idx], &msid, &sid);
            let reporter = SessionReporter::new(
                client, ws_tx,
                items[start_idx].id.clone(), msid, sid,
                items[start_idx].is_audio(),
                status.clone(),
            );
            let progress = spawn_progress_reporter(reporter.clone());
            let session  = PlaylistSession::new(
                items, start_idx, reporter, config, status, event_tx, subtitle_prefs, server_url, token, ext_sub_urls,
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
    #[cfg(test)]
    pub fn stub(status: Arc<Mutex<PlayerStatus>>) -> Self {
        let (tx, _rx) = std::sync::mpsc::channel();
        let player = Player::new(String::new(), String::new(), false, false, false, false, false, SubtitlePrefs::default(), tx, None);
        let subtitle_prefs = player.subtitle_prefs.clone();
        PlayerProxy { always_play_next: false, status, subtitle_prefs, inner: PlayerProxyInner::Local(player) }
    }

    pub fn local(player: Player, always_play_next: bool) -> Self {
        let status   = player.status.clone();
        let subtitle_prefs = player.subtitle_prefs.clone();
        PlayerProxy { always_play_next, status, subtitle_prefs, inner: PlayerProxyInner::Local(player) }
    }

    pub fn remote(remote: crate::remote_player::RemotePlayer, always_play_next: bool) -> Self {
        let status   = remote.status.clone();
        let subtitle_prefs = remote.subtitle_prefs.clone();
        PlayerProxy { always_play_next, status, subtitle_prefs, inner: PlayerProxyInner::Remote(remote) }
    }

    pub fn play(&self, item: &MediaItem, client: Arc<EmbyClient>, initial_volume: u8) {
        match &self.inner {
            PlayerProxyInner::Local(p)  => p.play(item, client, initial_volume),
            PlayerProxyInner::Remote(r) => r.play(item, client, initial_volume),
        }
    }

    pub fn play_playlist(
        &self,
        items: Vec<MediaItem>,
        start_idx: usize,
        client: Arc<EmbyClient>,
        initial_volume: u8,
    ) {
        match &self.inner {
            PlayerProxyInner::Local(p)  => p.play_playlist(items, start_idx, client, initial_volume),
            PlayerProxyInner::Remote(r) => r.play_playlist(items, start_idx, client, initial_volume),
        }
    }

    pub fn stop(&self) {
        match &self.inner {
            PlayerProxyInner::Local(p)  => p.stop(),
            PlayerProxyInner::Remote(r) => r.stop(),
        }
    }

    pub fn join(&self) {
        match &self.inner {
            PlayerProxyInner::Local(p)  => p.join(),
            PlayerProxyInner::Remote(r) => r.join(),
        }
    }

    pub fn join_or_timeout(&self, timeout: std::time::Duration) {
        match &self.inner {
            PlayerProxyInner::Local(p)  => p.join_or_timeout(timeout),
            PlayerProxyInner::Remote(_) => {}
        }
    }

    pub fn send_command(&self, cmd: PlayerCommand) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(p)  => p.send_command(cmd),
            PlayerProxyInner::Remote(r) => r.send_command(cmd),
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.inner, PlayerProxyInner::Remote(_))
    }

    pub fn is_remote_disconnected(&self) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_)  => false,
            PlayerProxyInner::Remote(r) => r.is_disconnected(),
        }
    }

    /// Returns a clonable stop handle for use from other threads (e.g. the
    /// quit watchdog). None in remote mode — the daemon owns the player.
    pub fn quit_handle(&self) -> Option<QuitHandle> {
        match &self.inner {
            PlayerProxyInner::Local(p)  => Some(QuitHandle { stop_tx: p.stop_tx.clone() }),
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
pub(crate) fn is_near_end(is_audio: bool, natural: bool, last_valid_pos: i64, runtime_ticks: i64) -> bool {
    !natural && !is_audio && runtime_ticks > 0
        && last_valid_pos * 20 / runtime_ticks >= 19
}

// Position to report to Emby when a playlist track ends.
// Zero means "treat as fully played / reset resume point".
// was_next_up alone does NOT zero the position — the user may have dismissed
// or ignored the overlay, and we must preserve where they actually were.
pub(crate) fn playlist_completed_pos(is_audio: bool, natural: bool, near_end: bool, last_valid_pos: i64) -> i64 {
    if is_audio || natural || near_end { 0 } else { last_valid_pos }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── mpv_title_opt ────────────────────────────────────────────────────────

    #[test]
    fn title_opt_plain() {
        assert_eq!(mpv_title_opt("Inception"), "force-media-title=%9%Inception");
    }

    #[test]
    fn title_opt_comma() {
        assert_eq!(mpv_title_opt("Cardiff, Claire (2)"), "force-media-title=%19%Cardiff, Claire (2)");
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
            id: id.into(), name: "Test Episode".into(), item_type: "Episode".into(),
            is_folder: false, media_type: "Video".into(), collection_type: String::new(),
            runtime_ticks: 3600 * crate::api::TICKS_PER_SECOND,
            played: false, playback_position_ticks: 0,
            series_id: "series1".into(), series_name: "Show".into(), album_id: String::new(),
            album: String::new(), index_number: 2, parent_index_number: 1,
            unplayed_item_count: 0,
            path: String::new(), artist: String::new(), sort_name: String::new(),
            production_year: 0, end_year: 0, overview: String::new(),
            premiere_date: String::new(), date_added: String::new(), total_count: 0, container: String::new(),
            director: String::new(), video_info: String::new(), audio_info: String::new(),
            genre: String::new(),
            playlist_item_id: String::new(),
        }
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

    // ── playlist_completed_pos / is_near_end ─────────────────────────────────

    const RUNTIME: i64 = 600 * TICKS_PER_SECOND; // 10-minute episode

    #[test]
    fn mid_episode_quit_preserves_position() {
        // User quits at ~88% (528 s into a 600 s episode). Not natural, not near-end,
        // next-up overlay may have appeared but next_up_jump was never set because the
        // user pressed q rather than clicking the overlay. Position must be preserved.
        let pos = 528 * TICKS_PER_SECOND;
        assert!(!is_near_end(false, false, pos, RUNTIME)); // 88% < 95%
        assert_eq!(playlist_completed_pos(false, false, false, pos), pos);
    }

    #[test]
    fn next_up_fired_preserves_position() {
        // Bug fix: was_next_up alone used to force completed_pos = 0. After the fix,
        // only natural EOF or >=95% position zeroes it. next_up_jump is now irrelevant
        // to completed_pos — playlist_completed_pos doesn't receive it at all.
        let pos = 540 * TICKS_PER_SECOND; // 90% — past 60s-before-end threshold
        assert!(!is_near_end(false, false, pos, RUNTIME)); // still below 95%
        assert_eq!(playlist_completed_pos(false, false, false, pos), pos);
    }

    #[test]
    fn natural_end_resets_position() {
        let pos = RUNTIME - TICKS_PER_SECOND; // 1 s before end
        assert_eq!(playlist_completed_pos(false, true, false, pos), 0);
    }

    #[test]
    fn near_end_boundary_resets_position() {
        // Exactly 95% (19/20) is near-end; 94% is not.
        let at_95   = RUNTIME * 19 / 20;
        let below   = at_95 - 1;
        assert!(is_near_end(false, false, at_95, RUNTIME));
        assert!(!is_near_end(false, false, below, RUNTIME));
        assert_eq!(playlist_completed_pos(false, false, true,  at_95), 0);
        assert_eq!(playlist_completed_pos(false, false, false, below), below);
    }

    #[test]
    fn audio_track_always_resets_position() {
        let pos = 300 * TICKS_PER_SECOND; // 50%
        assert!(!is_near_end(true, false, pos, RUNTIME));
        assert_eq!(playlist_completed_pos(true, false, false, pos), 0);
    }

    #[test]
    fn near_end_requires_runtime_known() {
        // If runtime_ticks is 0 (unknown), near-end must never trigger.
        assert!(!is_near_end(false, false, 1_000_000_000, 0));
    }
}
