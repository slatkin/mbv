use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use libmpv2::{Format, Mpv, events::{Event, PropertyData}, mpv_end_file_reason};
use crate::api::{EmbyClient, MediaItem, TICKS_PER_SECOND};
use crate::applog::{AppLog, Level};

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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayerStatus {
    pub position_ticks: i64,
    pub runtime_ticks: i64,
    pub paused: bool,
    pub volume: i64,
    pub volume_max: i64,
    pub current_idx: usize,
    pub active: bool,
    pub title: String,
    pub audio_tracks: Vec<(i64, String)>, // (mpv id, label)
    pub sub_tracks: Vec<(i64, String)>,
    pub audio_id: i64, // 0 = none/unknown
    pub sub_id: i64,   // 0 = off
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum PlayerEvent {
    Stopped { idx: usize, position_ticks: i64 },
    TrackChanged(usize),
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
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum PlayerCommand {
    TogglePause,
    JumpTo(usize),
    SetVolume(i64),
    Seek(f64),
    SeekAbsolute(f64),
    SetAudio(i64),
    SetSub(i64), // 0 = off
    LoadNew { url: String, start_pos: f64, item: Box<MediaItem> },
    NextUpShow { item_id: String, title: String },
}

pub fn lang_to_flag(s: &str) -> &'static str {
    let l = s.to_lowercase();
    // Matches both ISO lang codes (en, eng) and label prefixes (english ...)
    if l.starts_with("en") { "🇬🇧" }
    else if l.starts_with("fr") { "🇫🇷" }
    else if l.starts_with("de") || l.starts_with("ger") || l.starts_with("deu") { "🇩🇪" }
    else if l.starts_with("es") || l.starts_with("spa") || l.starts_with("spanish") { "🇪🇸" }
    else if l.starts_with("it") || l.starts_with("ita") || l.starts_with("italian") { "🇮🇹" }
    else if l.starts_with("pt") || l.starts_with("por") || l.starts_with("portuguese") { "🇵🇹" }
    else if l.starts_with("ja") || l.starts_with("jpn") || l.starts_with("japanese") { "🇯🇵" }
    else if l.starts_with("ko") || l.starts_with("kor") || l.starts_with("korean") { "🇰🇷" }
    else if l.starts_with("zh") || l.starts_with("chi") || l.starts_with("zho") || l.starts_with("chinese") { "🇨🇳" }
    else if l.starts_with("ru") || l.starts_with("rus") || l.starts_with("russian") { "🇷🇺" }
    else if l.starts_with("ar") || l.starts_with("ara") || l.starts_with("arabic") { "🇸🇦" }
    else if l.starts_with("nl") || l.starts_with("nld") || l.starts_with("dut") || l.starts_with("dutch") { "🇳🇱" }
    else if l.starts_with("sv") || l.starts_with("swe") || l.starts_with("swedish") { "🇸🇪" }
    else if l.starts_with("no") || l.starts_with("nor") || l.starts_with("norwegian") { "🇳🇴" }
    else if l.starts_with("da") || l.starts_with("dan") || l.starts_with("danish") { "🇩🇰" }
    else if l.starts_with("fi") || l.starts_with("fin") || l.starts_with("finnish") { "🇫🇮" }
    else if l.starts_with("pl") || l.starts_with("pol") || l.starts_with("polish") { "🇵🇱" }
    else if l.starts_with("cs") || l.starts_with("cze") || l.starts_with("ces") || l.starts_with("czech") { "🇨🇿" }
    else if l.starts_with("tr") || l.starts_with("tur") || l.starts_with("turkish") { "🇹🇷" }
    else if l.starts_with("uk") || l.starts_with("ukr") || l.starts_with("ukrainian") { "🇺🇦" }
    else if l.starts_with("hi") || l.starts_with("hin") || l.starts_with("hindi") { "🇮🇳" }
    else if l.starts_with("th") || l.starts_with("tha") || l.starts_with("thai") { "🇹🇭" }
    else { "" }
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

pub fn is_english(label: &str) -> bool {
    let l = label.to_lowercase();
    l == "en" || l == "eng" || l.starts_with("en-") || l.starts_with("english")
}

fn auto_select_tracks(mpv: &Mpv, status: &Arc<Mutex<PlayerStatus>>, subs_off: bool) {
    refresh_tracks(mpv, status);

    let (audio_tracks, audio_id) = {
        let s = status.lock().unwrap();
        (s.audio_tracks.clone(), s.audio_id)
    };

    // Switch to English audio if the current track isn't English
    let current_is_english = audio_tracks.iter()
        .find(|(id, _)| *id == audio_id)
        .is_some_and(|(_, l)| is_english(l));
    if !current_is_english {
        if let Some((id, _)) = audio_tracks.iter().find(|(_, l)| is_english(l)) {
            let _ = mpv.set_property("aid", *id);
            status.lock().unwrap().audio_id = *id;
        }
    }

    let sub_tracks: Vec<(i64, String)> = status.lock().unwrap().sub_tracks.clone();
    if subs_off {
        let _ = mpv.set_property("sid", "no".to_string());
        status.lock().unwrap().sub_id = 0;
    } else if let Some(&(first_id, _)) = sub_tracks.first() {
        let _ = mpv.set_property("sid", first_id);
        status.lock().unwrap().sub_id = first_id;
    }

    refresh_tracks(mpv, status);
}

fn refresh_tracks(mpv: &Mpv, status: &Arc<Mutex<PlayerStatus>>) {
    let count: i64 = match mpv.get_property("track-list/count") {
        Ok(n) => n,
        Err(_) => return,
    };
    let mut audio: Vec<(i64, String)> = Vec::new();
    let mut subs:  Vec<(i64, String)> = Vec::new();
    let mut audio_id: i64 = 0;
    let mut sub_id:   i64 = 0;

    for i in 0..count {
        let ttype: String = mpv.get_property(&format!("track-list/{i}/type")).unwrap_or_default();
        let id:    i64    = mpv.get_property(&format!("track-list/{i}/id")).unwrap_or(i + 1);
        let lang:  String = mpv.get_property(&format!("track-list/{i}/lang")).unwrap_or_default();
        let title: String = mpv.get_property(&format!("track-list/{i}/title")).unwrap_or_default();
        let codec: String = mpv.get_property(&format!("track-list/{i}/codec")).unwrap_or_default();
        let sel:   bool   = mpv.get_property(&format!("track-list/{i}/selected")).unwrap_or(false);

        match ttype.as_str() {
            "audio" => {
                if sel { audio_id = id; }
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
                let label = if !title.is_empty() { title }
                    else if !lang.is_empty() { lang.to_uppercase() }
                    else { format!("#{}", i + 1) };
                subs.push((id, label));
            }
            _ => {}
        }
    }

    let mut s = status.lock().unwrap();
    s.audio_tracks = audio;
    s.sub_tracks   = subs;
    s.audio_id     = audio_id;
    s.sub_id       = sub_id;
}

pub struct Player {
    server_url: String,
    token: String,
    show_audio_window: bool,
    use_mpv_config: bool,
    no_scripts: bool,
    #[allow(dead_code)]
    pub always_play_next: bool,
    pub always_skip_intro: bool,
    pub subs_off: Arc<AtomicBool>,
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
        subs_off: bool,
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
            subs_off: Arc::new(AtomicBool::new(subs_off)),
            event_tx,
            stop_tx: Arc::new(Mutex::new(None)),
            cmd_tx: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(PlayerStatus {
                position_ticks: 0,
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
                sub_id: 0,
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

    pub fn send_command(&self, cmd: PlayerCommand) {
        if let Some(tx) = self.cmd_tx.lock().unwrap().as_ref() {
            let _ = tx.send(cmd);
        }
    }

    pub fn play(&self, item: &MediaItem, client: Arc<EmbyClient>, log: AppLog, initial_volume: u8) {
        // If a video is already playing, load the new file into the existing mpv window
        if self.status.lock().unwrap().active {
            let url = format!(
                "{}/Videos/{}/stream?static=true&api_key={}",
                self.server_url, item.id, self.token
            );
            let start_pos = item.resume_seconds();
            {
                let mut st = self.status.lock().unwrap();
                st.position_ticks = item.playback_position_ticks;
                st.runtime_ticks = item.runtime_ticks;
                st.paused = false;
                st.current_idx = 0;
                st.title = item.display_name();
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

        let url = format!(
            "{}/Videos/{}/stream?static=true&api_key={}",
            self.server_url, item.id, self.token
        );
        let start_pos = item.resume_seconds();
        let item = item.clone();
        let item_pos = item.playback_position_ticks;
        let title = item.display_name();
        let headless = !self.show_audio_window && (item.media_type == "Audio" || item.item_type == "Audio");
        let use_mpv_config = self.use_mpv_config;
        let no_scripts = self.no_scripts;
        let always_skip_intro = self.always_skip_intro;
        let initial_volume = initial_volume;
        let event_tx = self.event_tx.clone();
        let status = self.status.clone();
        let ws_tx = self.ws_tx.clone();
        let subs_off = self.subs_off.clone();

        {
            let mut st = status.lock().unwrap();
            st.position_ticks = item_pos;
            st.runtime_ticks = item.runtime_ticks;
            st.paused = false;
            st.current_idx = 0;
            st.active = true;
            st.title = title.clone();
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *self.stop_tx.lock().unwrap() = Some(stop_tx);

        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        *self.cmd_tx.lock().unwrap() = Some(cmd_tx);

        let handle = thread::spawn(move || {
            let (session_id_str, msid_str) = {
                let (sid, msid) = client.get_playback_info(&item.id, &log);
                client.report_start(&item, &msid, &sid, &log);
                (sid, msid)
            };

            let current_item_id = Arc::new(Mutex::new(item.id.clone()));
            let current_msid    = Arc::new(Mutex::new(msid_str));
            let current_sid     = Arc::new(Mutex::new(session_id_str));

            let ipc_path = crate::config::mpv_ipc_path();
            let ipc_existed = std::path::Path::new(&ipc_path).exists();
            if ipc_existed {
                let _ = std::fs::remove_file(&ipc_path);
                log.push(Level::Info, "player", format!("init: removed stale ipc socket {}", ipc_path));
            }
            log.push(Level::Info, "player", format!("init: ipc={} (existed={})", ipc_path, ipc_existed));

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
                    log.push(Level::Error, "player", msg);
                    return;
                }
            };

            unsafe {
                libmpv2_sys::mpv_request_log_messages(mpv.ctx.as_ptr(), b"warn\0".as_ptr() as _);
            }

            // Set after init so user's mpv.conf cannot override these.
            if headless {
                let _ = mpv.set_property("vo", "null");
                let _ = mpv.set_property("force-window", "no");
            }

            {
                let mut st = status.lock().unwrap();
                let raw_max   = mpv.get_property::<i64>("volume-max").unwrap_or(130);
                st.volume_max = raw_max * raw_max / 100;
                let v = (initial_volume as i64).clamp(0, st.volume_max);
                let raw = (10.0 * (v as f64).sqrt()).round() as i64;
                let _ = mpv.set_property("volume", raw as f64);
                st.volume = v;
            }

            if start_pos > 0.0 {
                let _ = mpv.set_property("start", format!("{:.0}", start_pos));
            }

            let title_opt = mpv_title_opt(&title);
            log.push(Level::Info, "player", format!("loadfile url={url} opts={title_opt:?}"));
            if let Err(e) = mpv.command("loadfile", &[url.as_str(), "replace", "-1", title_opt.as_str()]) {
                log.push(Level::Warn, "player", format!("loadfile error: {} | url={url} opts={title_opt:?}", mpv_err_str(&e)));
                return;
            }

            let _ = mpv.observe_property("time-pos", Format::Double, 0);
            let _ = mpv.observe_property("pause", Format::Flag, 1);
            let _ = mpv.observe_property("volume", Format::Double, 2);
            let _ = mpv.observe_property("sid", Format::Int64, 3);
            if use_mpv_config {
                let _ = mpv.command("keybind", &["MOUSE_MOVE", "script-message mouse-moved"]);
            }

            let client_progress = client.clone();
            let cid_p   = current_item_id.clone();
            let cmsid_p = current_msid.clone();
            let csid_p  = current_sid.clone();
            let status_p = status.clone();
            let ws_tx_p = ws_tx.clone();
            let log_p = log.clone();
            let (progress_stop_tx, progress_stop_rx) = mpsc::channel::<()>();
            let mut progress_handle = Some(thread::spawn(move || {
                let mut ticks: u32 = 0;
                loop {
                    match progress_stop_rx.recv_timeout(Duration::from_secs(10)) {
                        Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            ticks += 1;
                            let (pos, paused) = {
                                let s = status_p.lock().unwrap();
                                (s.position_ticks, s.paused)
                            };
                            let id   = cid_p.lock().unwrap().clone();
                            let msid = cmsid_p.lock().unwrap().clone();
                            let sid  = csid_p.lock().unwrap().clone();
                            if ticks.is_multiple_of(3) {
                                if let Some(ref tx) = ws_tx_p {
                                    client_progress.report_progress_ws(&id, &msid, pos, paused, &sid, "TimeUpdate", tx, &log_p);
                                } else {
                                    client_progress.report_progress_http(&id, &msid, pos, paused, &sid, "TimeUpdate", &log_p);
                                }
                            } else {
                                client_progress.report_ping(&sid, &log_p);
                            }
                        }
                    }
                }
            }));

            let mut quit_at: Option<Instant> = None;
            let mut stop_reported = false;
            let mut mark_played_id: Option<String> = None; // set when natural end; retry in Shutdown if needed
            let mut pending_load = false;
            let mut pending_resume_secs: Option<f64> = None;
            let mut last_seek_at: Option<Instant> = None;
            let mut tracks_initialized = false;
            let mut current_osd_title = item.display_name();
            let mut last_mouse_osd: Option<Instant> = None;
            // mpv fires time-pos=0 when closing a file, before EndFile; track the last
            // non-zero position so report_stopped sends the real position, not 0.
            let mut last_valid_pos: i64 = item_pos;
            let mut series_id_for_next_up = if item.item_type == "Episode" { item.series_id.clone() } else { String::new() };
            let mut season_for_next_up   = item.parent_index_number;
            let mut episode_for_next_up  = item.index_number;
            let mut next_up_fired = false;
            let mut next_up_armed_logged = false;
            let mut intro_start_ticks: i64 = 0;
            let mut intro_end_ticks: i64   = 0;
            let mut intro_show_fired = false;
            let mut intro_hide_fired = false;
            if client.chapter_api_available {
                if let Some((s, e)) = client.get_intro_times(&item.id, &log) {
                    intro_start_ticks = s;
                    intro_end_ticks   = e;
                }
            }

            loop {
                // Process commands before checking the stop signal so that a LoadNew
                // arriving in the same iteration (e.g. WS Stop then Play) can cancel
                // the quit instead of fighting with it.
                let mut cancel_stop = false;
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        PlayerCommand::NextUpShow { item_id, title } => {
                            log.push(Level::Warn, "player", format!("next-up: sending script-message mbv-next-up id={item_id} title={title}"));
                            let r = mpv.command("script-message", &["mbv-next-up", &item_id, &title]);
                            log.push(Level::Warn, "player", format!("next-up: script-message result={r:?}"));
                        }
                        PlayerCommand::TogglePause => {
                            let p = status.lock().unwrap().paused;
                            let _ = mpv.set_property("pause", !p);
                        }
                        PlayerCommand::SetVolume(v) => {
                            let vol_max = status.lock().unwrap().volume_max;
                            let v = v.clamp(0, vol_max);
                            // v is perceptual (processed); convert to raw for mpv: raw = 10*sqrt(v)
                            let raw = (10.0 * (v as f64).sqrt()).round() as i64;
                            let _ = mpv.set_property("volume", raw as f64);
                            status.lock().unwrap().volume = v;
                            let _ = mpv.command("show-text", &[&format!("Volume: {v}%"), "1500"]);
                        }
                        PlayerCommand::JumpTo(_) => {}
                        PlayerCommand::Seek(secs) => {
                            let _ = mpv.command("seek", &[&secs.to_string(), "relative"]);
                            last_seek_at = Some(Instant::now());
                        }
                        PlayerCommand::SeekAbsolute(secs) => {
                            let _ = mpv.command("seek", &[&secs.to_string(), "absolute"]);
                            last_seek_at = Some(Instant::now());
                        }
                        PlayerCommand::SetAudio(id) => {
                            if id > 0 { let _ = mpv.set_property("aid", id); }
                            else { let _ = mpv.set_property("aid", "no".to_string()); }
                            status.lock().unwrap().audio_id = id;
                            refresh_tracks(&mpv, &status);
                        }
                        PlayerCommand::SetSub(id) => {
                            if id == 0 {
                                let _ = mpv.set_property("sid", "no".to_string());
                            } else {
                                let _ = mpv.set_property("sid", id);
                            }
                            status.lock().unwrap().sub_id = id;
                            refresh_tracks(&mpv, &status);
                        }
                        PlayerCommand::LoadNew { url, start_pos, item } => {
                            // Cancel any pending quit so the new file loads in the same window.
                            // This handles WS Stop+Play arriving in the same iteration.
                            cancel_stop = true;
                            quit_at = None;

                            // Report stopped for the current item before replacing
                            let old_id   = current_item_id.lock().unwrap().clone();
                            let old_msid = current_msid.lock().unwrap().clone();
                            let old_sid  = current_sid.lock().unwrap().clone();
                            client.report_stopped(&old_id, &old_msid, last_valid_pos, &old_sid, &log);

                            // Obtain session info for the new item and report start
                            let (new_sid, new_msid) = {
                                let (sid, msid) = client.get_playback_info(&item.id, &log);
                                client.report_start(&item, &msid, &sid, &log);
                                (sid, msid)
                            };
                            *current_item_id.lock().unwrap() = item.id.clone();
                            *current_msid.lock().unwrap()    = new_msid;
                            *current_sid.lock().unwrap()     = new_sid;

                            last_valid_pos = item.playback_position_ticks;
                            current_osd_title = item.display_name();
                            {
                                let mut st = status.lock().unwrap();
                                st.runtime_ticks      = item.runtime_ticks;
                                st.position_ticks     = item.playback_position_ticks;
                                st.title              = current_osd_title.clone();
                            }
                            tracks_initialized = false;
                            stop_reported = false;
                            pending_load = true;
                            next_up_fired = false;
                            next_up_armed_logged = false;
                            if item.item_type == "Episode" {
                                series_id_for_next_up = item.series_id.clone();
                                season_for_next_up    = item.parent_index_number;
                                episode_for_next_up   = item.index_number;
                            } else {
                                series_id_for_next_up = String::new();
                            }

                            if start_pos > 0.0 {
                                let _ = mpv.set_property("start", format!("{:.0}", start_pos));
                            } else {
                                let _ = mpv.set_property("start", "0");
                            }
                            let title_opt = mpv_title_opt(&item.display_name());
                            log.push(Level::Info, "player", format!("loadfile url={url} opts={title_opt:?}"));
                            if let Err(e) = mpv.command("loadfile", &[url.as_str(), "replace", "-1", title_opt.as_str()]) {
                                log.push(Level::Warn, "player", format!("loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e)));
                            }
                            let _ = mpv.command("script-message", &["mbv-next-up-dismiss"]);
                            let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
                            intro_show_fired = false;
                            intro_hide_fired = false;
                            intro_start_ticks = 0;
                            intro_end_ticks   = 0;
                            if client.chapter_api_available {
                                if let Some((s, e)) = client.get_intro_times(&item.id, &log) {
                                    intro_start_ticks = s;
                                    intro_end_ticks   = e;
                                }
                            }
                        }
                    }
                }

                if !cancel_stop && quit_at.is_none() && stop_rx.try_recv().is_ok() {
                    let _ = progress_stop_tx.send(());
                    if let Some(h) = progress_handle.take() { let _ = h.join(); }
                    if !stop_reported {
                        let id   = current_item_id.lock().unwrap().clone();
                        let msid = current_msid.lock().unwrap().clone();
                        let sid  = current_sid.lock().unwrap().clone();
                        client.report_stopped(&id, &msid, last_valid_pos, &sid, &log);
                        stop_reported = true;
                    }
                    let _ = mpv.command("quit", &[]);
                    quit_at = Some(Instant::now());
                }

                if quit_at.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) {
                    status.lock().unwrap().active = false;
                    let _ = event_tx.send(PlayerEvent::Stopped { idx: 0, position_ticks: last_valid_pos });
                    return;
                }

                match mpv.wait_event(0.5) {
                    Some(Ok(Event::PropertyChange { name: "volume", change: PropertyData::Double(vol), .. })) => {
                        status.lock().unwrap().volume = (vol * vol / 100.0) as i64;
                    }
                    Some(Ok(Event::PropertyChange { change: PropertyData::Double(pos_secs), .. })) => {
                        let ticks = (pos_secs * TICKS_PER_SECOND as f64) as i64;
                        status.lock().unwrap().position_ticks = ticks;
                        if pos_secs > 0.0 { last_valid_pos = ticks; }
                        // Fire next-up prompt 60 s before the episode ends (once per playback).
                        const NEXT_UP_TICKS: i64 = 60 * TICKS_PER_SECOND;
                        if !next_up_fired {
                            if series_id_for_next_up.is_empty() {
                                if !next_up_armed_logged && ticks > 0 && ticks < TICKS_PER_SECOND * 5 {
                                    next_up_armed_logged = true;
                                    log.push(Level::Warn, "player", "next-up disabled: no series_id (Episode item without SeriesId in fetch)");
                                }
                            } else {
                                let runtime = status.lock().unwrap().runtime_ticks;
                                if runtime > NEXT_UP_TICKS && ticks > runtime - NEXT_UP_TICKS {
                                    next_up_fired = true;
                                    log.push(Level::Warn, "player", format!("next-up: threshold reached series={}", series_id_for_next_up));
                                    let _ = event_tx.send(PlayerEvent::NextUpThreshold {
                                        series_id: series_id_for_next_up.clone(),
                                        season: season_for_next_up,
                                        episode: episode_for_next_up,
                                    });
                                } else if !next_up_armed_logged && ticks > 0 && ticks < TICKS_PER_SECOND * 5 {
                                    next_up_armed_logged = true;
                                    log.push(Level::Info, "player", format!("next-up: armed series={} runtime={}s", series_id_for_next_up, runtime / TICKS_PER_SECOND));
                                }
                            }
                        }
                        if intro_end_ticks > intro_start_ticks {
                            if !intro_show_fired && ticks >= intro_start_ticks {
                                intro_show_fired = true;
                                intro_hide_fired = true;
                                let end_secs = intro_end_ticks as f64 / TICKS_PER_SECOND as f64;
                                if always_skip_intro {
                                    let _ = mpv.set_property("time-pos", end_secs);
                                } else {
                                    let _ = event_tx.send(PlayerEvent::IntroStarted { intro_end_ticks });
                                    let _ = mpv.command("script-message", &["mbv-skip-intro", &end_secs.to_string()]);
                                }
                            }
                            if !intro_hide_fired && ticks >= intro_end_ticks {
                                intro_hide_fired = true;
                                let _ = event_tx.send(PlayerEvent::IntroEnded);
                                let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
                            }
                        }
                    }
                    Some(Ok(Event::PropertyChange { name: "pause", change: PropertyData::Flag(paused), .. })) => {
                        status.lock().unwrap().paused = paused;
                        if quit_at.is_none() {
                            let pos = status.lock().unwrap().position_ticks;
                            let event_name = if paused { "Pause" } else { "Unpause" };
                            let id   = current_item_id.lock().unwrap().clone();
                            let msid = current_msid.lock().unwrap().clone();
                            let sid  = current_sid.lock().unwrap().clone();
                            if let Some(ref tx) = ws_tx {
                                client.report_progress_ws(&id, &msid, pos, paused, &sid, event_name, tx, &log);
                            } else {
                                client.report_progress_http(&id, &msid, pos, paused, &sid, event_name, &log);
                            }
                        }
                    }
                    Some(Ok(Event::PropertyChange { name: "sid", change: PropertyData::Int64(id), .. })) => {
                        status.lock().unwrap().sub_id = id;
                    }
                    Some(Ok(Event::PlaybackRestart)) => {
                        let event_name: &str;
                        if !tracks_initialized {
                            auto_select_tracks(&mpv, &status, subs_off.load(Ordering::Relaxed));
                            tracks_initialized = true;
                            let _ = mpv.set_property("start", "0");
                            if let Some(secs) = pending_resume_secs.take() {
                                if secs > 0.0 {
                                    let _ = mpv.command("seek", &[&format!("{secs:.0}"), "absolute"]);
                                    last_seek_at = Some(Instant::now());
                                }
                            }
                            if use_mpv_config { let _ = mpv.command("show-text", &[&current_osd_title, "3000"]); }
                            event_name = "TimeUpdate";
                        } else {
                            // Any restart after init means a seek happened (via TUI or mpv OSC).
                            // Re-arm so a seek into/out-of the threshold is handled correctly.
                            next_up_fired = false;
                            next_up_armed_logged = false;
                            if last_seek_at.take().is_some() {
                                if use_mpv_config { let _ = mpv.command("show-text", &[&current_osd_title, "2000"]); }
                            }
                            event_name = "Seek";
                        }
                        let seek_settled = last_seek_at.is_none_or(|t| t.elapsed() > Duration::from_millis(500));
                        if quit_at.is_none() && seek_settled {
                            last_seek_at = None;
                            let (pos, paused) = {
                                let s = status.lock().unwrap();
                                (s.position_ticks, s.paused)
                            };
                            let id   = current_item_id.lock().unwrap().clone();
                            let msid = current_msid.lock().unwrap().clone();
                            let sid  = current_sid.lock().unwrap().clone();
                            if let Some(ref tx) = ws_tx {
                                client.report_progress_ws(&id, &msid, pos, paused, &sid, event_name, tx, &log);
                            } else {
                                client.report_progress_http(&id, &msid, pos, paused, &sid, event_name, &log);
                            }
                        }
                    }
                    Some(Ok(Event::EndFile(reason))) => {
                        if quit_at.is_some() { continue; }
                        // EndFile triggered by a loadfile replace — already reported stopped above
                        if pending_load { pending_load = false; continue; }

                        if reason == mpv_end_file_reason::Error {
                            log.push(Level::Warn, "player", "EndFile: playback error (file may be unreadable or format unsupported)");
                        }
                        let id   = current_item_id.lock().unwrap().clone();
                        let msid = current_msid.lock().unwrap().clone();
                        let sid  = current_sid.lock().unwrap().clone();
                        let natural_end = reason == mpv_end_file_reason::Eof
                            && status.lock().unwrap().runtime_ticks > 0;
                        let _ = progress_stop_tx.send(());
                        if let Some(h) = progress_handle.take() { let _ = h.join(); }
                        client.report_stopped(&id, &msid, last_valid_pos, &sid, &log);
                        stop_reported = true;
                        if natural_end {
                            match client.mark_played(&id) {
                                Ok(()) => {
                                    log.push(Level::Info, "player", format!("mark_played ok id={id}"));
                                }
                                Err(e) => {
                                    log.push(Level::Warn, "player", format!("mark_played failed id={id}: {e}; will retry"));
                                    mark_played_id = Some(id.clone());
                                }
                            }
                            let _ = event_tx.send(PlayerEvent::Stopped { idx: 0, position_ticks: last_valid_pos });
                        }
                    }
                    Some(Ok(Event::LogMessage { prefix, level, text, .. })) => {
                        let t = text.trim_end();
                        if !t.is_empty() {
                            log.push(Level::Warn, "mpv", format!("[{}/{}] {}", prefix, level, t));
                        }
                    }
                    Some(Ok(Event::ClientMessage(args))) if args.first().copied() == Some("mbv-next-up-play") => {
                        log.push(Level::Info, "player", "next-up: mbv-next-up-play received from Lua");
                        let _ = event_tx.send(PlayerEvent::NextUpPlay);
                    }
                    Some(Ok(Event::ClientMessage(args))) if args.first().copied() == Some("mbv-skip-intro-play") => {
                        let _ = event_tx.send(PlayerEvent::SkipIntroPlay);
                    }
                    Some(Ok(Event::ClientMessage(args))) if use_mpv_config && args.first().copied() == Some("mouse-moved") => {
                        let show = last_mouse_osd.is_none_or(|t: Instant| t.elapsed() > Duration::from_secs(3));
                        if show {
                            let _ = mpv.command("show-text", &[&current_osd_title, "2000"]);
                            last_mouse_osd = Some(Instant::now());
                        }
                    }
                    Some(Ok(Event::Shutdown)) => {
                        if !stop_reported {
                            let _ = progress_stop_tx.send(());
                            drop(progress_handle.take()); // detach; don't block on HTTP call
                            let id   = current_item_id.lock().unwrap().clone();
                            let msid = current_msid.lock().unwrap().clone();
                            let sid  = current_sid.lock().unwrap().clone();
                            client.report_stopped(&id, &msid, last_valid_pos, &sid, &log);
                        }
                        // Retry mark_played in a detached thread so Shutdown never blocks.
                        if let Some(mid) = mark_played_id.take() {
                            let c2 = client.clone();
                            let l2 = log.clone();
                            std::thread::spawn(move || {
                                if let Err(e) = c2.mark_played(&mid) {
                                    l2.push(Level::Warn, "player", format!("mark_played retry failed id={mid}: {e}"));
                                } else {
                                    l2.push(Level::Info, "player", format!("mark_played retry ok id={mid}"));
                                }
                            });
                        }
                        status.lock().unwrap().active = false;
                        let _ = event_tx.send(PlayerEvent::Stopped { idx: 0, position_ticks: last_valid_pos });
                        return;
                    }
                    Some(Err(e)) => {
                        log.push(Level::Warn, "player", format!("event error: {}", mpv_err_str(&e)));
                    }
                    _ => {}
                }
            }
        });
        *self.thread_handle.lock().unwrap() = Some(handle);
    }

    pub fn play_playlist(&self, items: Vec<MediaItem>, start_idx: usize, client: Arc<EmbyClient>, log: AppLog, initial_volume: u8) {
        self.stop();
        self.join();
        if items.is_empty() { return; }
        let start_idx = start_idx.min(items.len() - 1);

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        *self.stop_tx.lock().unwrap() = Some(stop_tx);

        let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
        *self.cmd_tx.lock().unwrap() = Some(cmd_tx);

        let event_tx = self.event_tx.clone();
        let server_url = self.server_url.clone();
        let token = self.token.clone();
        let n = items.len();
        let status = self.status.clone();
        let ws_tx = self.ws_tx.clone();
        let subs_off = self.subs_off.clone();
        let headless = !self.show_audio_window
            && items.iter().all(|i| i.media_type == "Audio" || i.item_type == "Audio");
        let use_mpv_config = self.use_mpv_config;
        let no_scripts = self.no_scripts;
        let always_skip_intro = self.always_skip_intro;
        let initial_volume = initial_volume;

        {
            let mut st = status.lock().unwrap();
            st.position_ticks = 0;
            st.runtime_ticks = items[start_idx].runtime_ticks;
            st.paused = false;
            st.current_idx = start_idx;
            st.active = true;
            st.title = items[start_idx].display_name();
        }

        let handle = thread::spawn(move || {
            let (session_id_str, first_msid) = {
                let (sid, msid) = client.get_playback_info(&items[start_idx].id, &log);
                client.report_start(&items[start_idx], &msid, &sid, &log);
                (sid, msid)
            };
            let session_id = Arc::new(Mutex::new(session_id_str));

            let ipc_path = crate::config::mpv_ipc_path();
            let ipc_existed = std::path::Path::new(&ipc_path).exists();
            if ipc_existed {
                let _ = std::fs::remove_file(&ipc_path);
                log.push(Level::Info, "player", format!("init: removed stale ipc socket {}", ipc_path));
            }
            log.push(Level::Info, "player", format!("init: ipc={} (existed={})", ipc_path, ipc_existed));

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
                    log.push(Level::Error, "player", msg);
                    return;
                }
            };

            unsafe {
                libmpv2_sys::mpv_request_log_messages(mpv.ctx.as_ptr(), b"warn\0".as_ptr() as _);
            }

            // Set after init so user's mpv.conf cannot override these.
            if headless {
                let _ = mpv.set_property("vo", "null");
                let _ = mpv.set_property("force-window", "no");
            }

            {
                let mut st = status.lock().unwrap();
                let raw_max   = mpv.get_property::<i64>("volume-max").unwrap_or(130);
                st.volume_max = raw_max * raw_max / 100;
                let v = (initial_volume as i64).clamp(0, st.volume_max);
                let raw = (10.0 * (v as f64).sqrt()).round() as i64;
                let _ = mpv.set_property("volume", raw as f64);
                st.volume = v;
            }

            // Load items starting from start_idx so mpv plays the right item
            // immediately with no playlist-pos jump required.
            for (i, item) in items[start_idx..].iter().enumerate() {
                let url = format!("{}/Videos/{}/stream?static=true&api_key={}", server_url, item.id, token);
                let mode = if i == 0 { "replace" } else { "append-play" };
                let title_opt = if i == 0 && item.playback_position_ticks > 0 {
                    format!("{},start={:.0}", mpv_title_opt(&item.display_name()), item.resume_seconds())
                } else {
                    mpv_title_opt(&item.display_name())
                };
                if let Err(e) = mpv.command("loadfile", &[url.as_str(), mode, "-1", title_opt.as_str()]) {
                    log.push(Level::Warn, "player", format!("loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e)));
                    if i == 0 {
                        // First file failed: nothing queued, exit cleanly.
                        status.lock().unwrap().active = false;
                        return;
                    }
                    // Subsequent file failed: skip it, keep playing what loaded.
                }
            }

            let _ = mpv.observe_property("time-pos", Format::Double, 0);
            let _ = mpv.observe_property("pause", Format::Flag, 1);
            let _ = mpv.observe_property("volume", Format::Double, 2);
            let _ = mpv.observe_property("sid", Format::Int64, 3);
            if use_mpv_config {
                let _ = mpv.command("keybind", &["MOUSE_MOVE", "script-message mouse-moved"]);
            }

            let current_item_id = Arc::new(Mutex::new(items[start_idx].id.clone()));
            let current_msid = Arc::new(Mutex::new(first_msid));

            let client_progress = client.clone();
            let cid_p = current_item_id.clone();
            let cmsid_p = current_msid.clone();
            let csid_p = session_id.clone();
            let status_p = status.clone();
            let ws_tx_p = ws_tx.clone();
            let log_p = log.clone();
            let (progress_stop_tx, progress_stop_rx) = mpsc::channel::<()>();
            let mut progress_handle = Some(thread::spawn(move || {
                let mut ticks: u32 = 0;
                loop {
                    match progress_stop_rx.recv_timeout(Duration::from_secs(10)) {
                        Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            ticks += 1;
                            let (pos, paused) = {
                                let s = status_p.lock().unwrap();
                                (s.position_ticks, s.paused)
                            };
                            let id = cid_p.lock().unwrap().clone();
                            let msid = cmsid_p.lock().unwrap().clone();
                            let sid = csid_p.lock().unwrap().clone();
                            if ticks.is_multiple_of(3) {
                                if let Some(ref tx) = ws_tx_p {
                                    client_progress.report_progress_ws(&id, &msid, pos, paused, &sid, "TimeUpdate", tx, &log_p);
                                } else {
                                    client_progress.report_progress_http(&id, &msid, pos, paused, &sid, "TimeUpdate", &log_p);
                                }
                            } else {
                                client_progress.report_ping(&sid, &log_p);
                            }
                        }
                    }
                }
            }));

            let mut current_idx = start_idx;
            let mut mpv_offset = start_idx; // mpv playlist pos k == items[mpv_offset + k]
            let mut forced_idx: Option<usize> = None;
            let mut quit_at: Option<Instant> = None;
            let mut last_seek_at: Option<Instant> = None;
            let mut last_valid_pos: i64 = 0;
            let mut tracks_initialized = false;
            let mut playlist_cancelled = false;
            let mut pending_load = false;
            let mut stop_reported = false;
            let mut current_osd_title = items[start_idx].display_name();
            let mut last_mouse_osd: Option<Instant> = None;
            let mut pending_resume_secs: Option<f64> = None;
            let mut playlist_next_up_fired = false;
            let mut playlist_next_up_armed = false;
            let mut intro_start_ticks: i64 = 0;
            let mut intro_end_ticks: i64   = 0;
            let mut intro_show_fired = false;
            let mut intro_hide_fired = false;
            if client.chapter_api_available {
                if let Some((s, e)) = client.get_intro_times(&items[start_idx].id, &log) {
                    intro_start_ticks = s;
                    intro_end_ticks   = e;
                }
            }

            loop {
                let mut cancel_stop = false;
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        PlayerCommand::NextUpShow { item_id, title } => {
                            log.push(Level::Warn, "player", format!("next-up: sending script-message mbv-next-up id={item_id} title={title}"));
                            let r = mpv.command("script-message", &["mbv-next-up", &item_id, &title]);
                            log.push(Level::Warn, "player", format!("next-up: script-message result={r:?}"));
                        }
                        PlayerCommand::TogglePause => {
                            let p = status.lock().unwrap().paused;
                            let _ = mpv.set_property("pause", !p);
                        }
                        PlayerCommand::JumpTo(idx) => {
                            if idx < n {
                                if idx >= mpv_offset {
                                    forced_idx = Some(idx);
                                    {
                                        let mut s = status.lock().unwrap();
                                        s.current_idx    = idx;
                                        s.position_ticks = 0;
                                        s.runtime_ticks  = items[idx].runtime_ticks;
                                        s.title          = items[idx].display_name();
                                    }
                                    let _ = mpv.set_property("playlist-pos", (idx - mpv_offset) as i64);
                                } else {
                                    // Backward jump: reload mpv playlist from idx
                                    let old_id   = current_item_id.lock().unwrap().clone();
                                    let old_msid = current_msid.lock().unwrap().clone();
                                    let old_sid  = session_id.lock().unwrap().clone();
                                    client.report_stopped(&old_id, &old_msid, last_valid_pos, &old_sid, &log);
                                    let (new_sid, new_msid) = {
                                        let (sid, msid) = client.get_playback_info(&items[idx].id, &log);
                                        client.report_start(&items[idx], &msid, &sid, &log);
                                        (sid, msid)
                                    };
                                    *current_item_id.lock().unwrap() = items[idx].id.clone();
                                    *current_msid.lock().unwrap()    = new_msid;
                                    *session_id.lock().unwrap()      = new_sid;
                                    {
                                        let mut s = status.lock().unwrap();
                                        s.position_ticks = 0;
                                        s.runtime_ticks  = items[idx].runtime_ticks;
                                        s.current_idx    = idx;
                                        s.title          = items[idx].display_name();
                                    }
                                    current_idx   = idx;
                                    mpv_offset    = idx;
                                    current_osd_title = items[idx].display_name();
                                    last_valid_pos    = 0;
                                    tracks_initialized = false;
                                    pending_load  = true;
                                    let resume = if items[idx].playback_position_ticks > 0 {
                                        format!("{:.0}", items[idx].resume_seconds())
                                    } else {
                                        "0".to_string()
                                    };
                                    let _ = mpv.set_property("start", resume);
                                    for (i, item) in items[idx..].iter().enumerate() {
                                        let url = format!("{}/Videos/{}/stream?static=true&api_key={}", server_url, item.id, token);
                                        let mode = if i == 0 { "replace" } else { "append-play" };
                                        let title_opt = mpv_title_opt(&item.display_name());
                                        if let Err(e) = mpv.command("loadfile", &[url.as_str(), mode, "-1", title_opt.as_str()]) {
                                            log.push(Level::Warn, "player", format!("loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e)));
                                        }
                                    }
                                    let _ = event_tx.send(PlayerEvent::TrackChanged(idx));
                                }
                            }
                        }
                        PlayerCommand::SetVolume(v) => {
                            let vol_max = status.lock().unwrap().volume_max;
                            let v = v.clamp(0, vol_max);
                            // v is perceptual (processed); convert to raw for mpv: raw = 10*sqrt(v)
                            let raw = (10.0 * (v as f64).sqrt()).round() as i64;
                            let _ = mpv.set_property("volume", raw as f64);
                            status.lock().unwrap().volume = v;
                            let _ = mpv.command("show-text", &[&format!("Volume: {v}%"), "1500"]);
                        }
                        PlayerCommand::Seek(secs) => {
                            let _ = mpv.command("seek", &[&secs.to_string(), "relative"]);
                            last_seek_at = Some(Instant::now());
                        }
                        PlayerCommand::SeekAbsolute(secs) => {
                            let _ = mpv.command("seek", &[&secs.to_string(), "absolute"]);
                            last_seek_at = Some(Instant::now());
                        }
                        PlayerCommand::SetAudio(id) => {
                            if id > 0 { let _ = mpv.set_property("aid", id); }
                            else { let _ = mpv.set_property("aid", "no".to_string()); }
                            status.lock().unwrap().audio_id = id;
                            refresh_tracks(&mpv, &status);
                        }
                        PlayerCommand::SetSub(id) => {
                            if id == 0 {
                                let _ = mpv.set_property("sid", "no".to_string());
                            } else {
                                let _ = mpv.set_property("sid", id);
                            }
                            status.lock().unwrap().sub_id = id;
                            refresh_tracks(&mpv, &status);
                        }
                        PlayerCommand::LoadNew { url, start_pos, item } => {
                            cancel_stop = true;
                            quit_at = None;
                            playlist_cancelled = true;

                            let old_id   = current_item_id.lock().unwrap().clone();
                            let old_msid = current_msid.lock().unwrap().clone();
                            let old_sid  = session_id.lock().unwrap().clone();
                            client.report_stopped(&old_id, &old_msid, last_valid_pos, &old_sid, &log);

                            let (new_sid, new_msid) = {
                                let (sid, msid) = client.get_playback_info(&item.id, &log);
                                client.report_start(&item, &msid, &sid, &log);
                                (sid, msid)
                            };
                            *current_item_id.lock().unwrap() = item.id.clone();
                            *current_msid.lock().unwrap()    = new_msid;
                            *session_id.lock().unwrap()      = new_sid;

                            last_valid_pos = item.playback_position_ticks;
                            tracks_initialized = false;
                            stop_reported = false;
                            pending_load = true;

                            let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
                            intro_show_fired = false;
                            intro_hide_fired = false;
                            intro_start_ticks = 0;
                            intro_end_ticks   = 0;
                            if client.chapter_api_available {
                                if let Some((s, e)) = client.get_intro_times(&item.id, &log) {
                                    intro_start_ticks = s;
                                    intro_end_ticks   = e;
                                }
                            }

                            if start_pos > 0.0 {
                                let _ = mpv.set_property("start", format!("{:.0}", start_pos));
                            } else {
                                let _ = mpv.set_property("start", "0");
                            }
                            let title_opt = mpv_title_opt(&item.display_name());
                            log.push(Level::Info, "player", format!("loadfile url={url} opts={title_opt:?}"));
                            if let Err(e) = mpv.command("loadfile", &[url.as_str(), "replace", "-1", title_opt.as_str()]) {
                                log.push(Level::Warn, "player", format!("loadfile error: {} | opts={title_opt:?}", mpv_err_str(&e)));
                            }
                        }
                    }
                }

                if !cancel_stop && quit_at.is_none() && stop_rx.try_recv().is_ok() {
                    let _ = progress_stop_tx.send(());
                    if let Some(h) = progress_handle.take() { let _ = h.join(); }
                    if !stop_reported {
                        let id   = current_item_id.lock().unwrap().clone();
                        let msid = current_msid.lock().unwrap().clone();
                        let sid  = session_id.lock().unwrap().clone();
                        client.report_stopped(&id, &msid, last_valid_pos, &sid, &log);
                        stop_reported = true;
                    }
                    let _ = mpv.command("quit", &[]);
                    quit_at = Some(Instant::now());
                }

                if quit_at.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) {
                    status.lock().unwrap().active = false;
                    let stopped_idx = current_idx;
                    let stopped_pos = last_valid_pos;
                    let _ = event_tx.send(PlayerEvent::Stopped { idx: stopped_idx, position_ticks: stopped_pos });
                    return;
                }

                match mpv.wait_event(0.5) {
                    Some(Ok(Event::PropertyChange { name: "volume", change: PropertyData::Double(vol), .. })) => {
                        status.lock().unwrap().volume = (vol * vol / 100.0) as i64;
                    }
                    Some(Ok(Event::PropertyChange { change: PropertyData::Double(pos_secs), .. })) => {
                        let ticks = (pos_secs * TICKS_PER_SECOND as f64) as i64;
                        status.lock().unwrap().position_ticks = ticks;
                        if pos_secs > 0.0 { last_valid_pos = ticks; }
                        // Playlist next-up: match Emby Web's timing from videoosd.js.
                        // 90 s before end regardless of runtime length.
                        // Minimum episode: 10 min. Minimum remaining when shown: 20 s.
                        const MIN_RUNTIME_TICKS: i64 = 600 * TICKS_PER_SECOND;
                        const MIN_REMAIN_TICKS:  i64 = 20 * TICKS_PER_SECOND;
                        if current_idx + 1 < items.len() {
                            let runtime = status.lock().unwrap().runtime_ticks;
                            if runtime > 0 {
                                let show_secs: i64 = 60;
                                let show_at = runtime - show_secs * TICKS_PER_SECOND;
                                let remaining = runtime - ticks;
                                if playlist_next_up_fired && ticks < show_at {
                                    playlist_next_up_fired = false;
                                    playlist_next_up_armed = false;
                                }
                                if !playlist_next_up_fired && runtime >= MIN_RUNTIME_TICKS {
                                    if remaining >= MIN_REMAIN_TICKS && ticks >= show_at {
                                        playlist_next_up_fired = true;
                                        let _ = event_tx.send(PlayerEvent::PlaylistNextUp { next_idx: current_idx + 1 });
                                    } else if !playlist_next_up_armed && ticks > 0 && ticks < TICKS_PER_SECOND * 5 {
                                        playlist_next_up_armed = true;
                                        log.push(Level::Info, "player", format!("playlist next-up armed idx={}", current_idx + 1));
                                    }
                                }
                            }
                        }
                        if intro_end_ticks > intro_start_ticks {
                            if !intro_show_fired && ticks >= intro_start_ticks {
                                intro_show_fired = true;
                                intro_hide_fired = true;
                                let end_secs = intro_end_ticks as f64 / TICKS_PER_SECOND as f64;
                                if always_skip_intro {
                                    let _ = mpv.set_property("time-pos", end_secs);
                                } else {
                                    let _ = event_tx.send(PlayerEvent::IntroStarted { intro_end_ticks });
                                    let _ = mpv.command("script-message", &["mbv-skip-intro", &end_secs.to_string()]);
                                }
                            }
                            if !intro_hide_fired && ticks >= intro_end_ticks {
                                intro_hide_fired = true;
                                let _ = event_tx.send(PlayerEvent::IntroEnded);
                                let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
                            }
                        }
                    }
                    Some(Ok(Event::PropertyChange { name: "pause", change: PropertyData::Flag(paused), .. })) => {
                        let id   = current_item_id.lock().unwrap().clone();
                        let msid = current_msid.lock().unwrap().clone();
                        status.lock().unwrap().paused = paused;
                        if quit_at.is_none() {
                            let pos = status.lock().unwrap().position_ticks;
                            let event_name = if paused { "Pause" } else { "Unpause" };
                            let sid = session_id.lock().unwrap().clone();
                            if let Some(ref tx) = ws_tx {
                                client.report_progress_ws(&id, &msid, pos, paused, &sid, event_name, tx, &log);
                            } else {
                                client.report_progress_http(&id, &msid, pos, paused, &sid, event_name, &log);
                            }
                        }
                    }
                    Some(Ok(Event::PropertyChange { name: "sid", change: PropertyData::Int64(id), .. })) => {
                        status.lock().unwrap().sub_id = id;
                    }
                    Some(Ok(Event::PlaybackRestart)) => {
                        if !tracks_initialized {
                            auto_select_tracks(&mpv, &status, subs_off.load(Ordering::Relaxed));
                            tracks_initialized = true;
                            if let Some(secs) = pending_resume_secs.take() {
                                let _ = mpv.command("seek", &[&format!("{secs:.0}"), "absolute"]);
                                last_seek_at = Some(Instant::now());
                            }
                            if use_mpv_config { let _ = mpv.command("show-text", &[&current_osd_title, "3000"]); }
                        } else if last_seek_at.take().is_some() {
                            if use_mpv_config { let _ = mpv.command("show-text", &[&current_osd_title, "2000"]); }
                        }
                        let seek_settled = last_seek_at.is_none_or(|t| t.elapsed() > Duration::from_millis(500));
                        if quit_at.is_none() && seek_settled {
                            last_seek_at = None;
                            let (pos, paused) = {
                                let s = status.lock().unwrap();
                                (s.position_ticks, s.paused)
                            };
                            let id   = current_item_id.lock().unwrap().clone();
                            let msid = current_msid.lock().unwrap().clone();
                            let sid  = session_id.lock().unwrap().clone();
                            if let Some(ref tx) = ws_tx {
                                client.report_progress_ws(&id, &msid, pos, paused, &sid, "TimeUpdate", tx, &log);
                            } else {
                                client.report_progress_http(&id, &msid, pos, paused, &sid, "TimeUpdate", &log);
                            }
                        }
                    }
                    Some(Ok(Event::LogMessage { prefix, level, text, .. })) => {
                        let t = text.trim_end();
                        if !t.is_empty() {
                            log.push(Level::Warn, "mpv", format!("[{}/{}] {}", prefix, level, t));
                        }
                    }
                    Some(Ok(Event::EndFile(reason))) => {
                        if quit_at.is_some() { continue; }
                        if pending_load { pending_load = false; continue; }
                        if reason == mpv_end_file_reason::Error {
                            log.push(Level::Warn, "player", "EndFile: playback error (file may be unreadable or format unsupported)");
                        }
                        let id   = current_item_id.lock().unwrap().clone();
                        let msid = current_msid.lock().unwrap().clone();
                        let sid  = session_id.lock().unwrap().clone();

                        if playlist_cancelled {
                            let natural_end = reason == mpv_end_file_reason::Eof
                                && status.lock().unwrap().runtime_ticks > 0;
                            let _ = progress_stop_tx.send(());
                            if let Some(h) = progress_handle.take() { let _ = h.join(); }
                            client.report_stopped(&id, &msid, last_valid_pos, &sid, &log);
                            stop_reported = true;
                            if natural_end {
                                if let Err(e) = client.mark_played(&id) {
                                    log.push(Level::Warn, "player", format!("mark_played failed id={id}: {e}"));
                                }
                            }
                            continue; // wait for Shutdown to fire PlayerEvent::Stopped
                        }

                        let completed_idx = current_idx;
                        let natural = reason == mpv_end_file_reason::Eof
                            && items[completed_idx].runtime_ticks > 0;
                        let completed_pos = if natural { 0 } else { last_valid_pos };

                        let next_idx = if let Some(jump_idx) = forced_idx.take() {
                            jump_idx
                        } else {
                            current_idx + 1
                        };

                        if next_idx >= n {
                            let _ = progress_stop_tx.send(());
                            if let Some(h) = progress_handle.take() { let _ = h.join(); }
                            status.lock().unwrap().active = false;
                            client.report_stopped(&id, &msid, completed_pos, &sid, &log);
                            if natural {
                                if let Err(e) = client.mark_played(&items[completed_idx].id) {
                                    log.push(Level::Warn, "player", format!("mark_played failed id={}: {e}", items[completed_idx].id));
                                }
                            }
                            let _ = event_tx.send(PlayerEvent::Stopped { idx: completed_idx, position_ticks: completed_pos });
                            return;
                        }

                        // Update UI to the next track immediately, before slow network calls
                        current_idx = next_idx;
                        last_valid_pos = 0;
                        tracks_initialized = false;
                        {
                            let mut s = status.lock().unwrap();
                            s.position_ticks = 0;
                            s.runtime_ticks  = items[current_idx].runtime_ticks;
                            s.current_idx    = current_idx;
                            s.title          = items[current_idx].display_name();
                        }

                        client.report_stopped(&id, &msid, completed_pos, &sid, &log);
                        if natural {
                            if let Err(e) = client.mark_played(&items[completed_idx].id) {
                                log.push(Level::Warn, "player", format!("mark_played failed id={}: {e}", items[completed_idx].id));
                            }
                        }

                        // Reset start position so the next playlist item isn't affected
                        let _ = mpv.set_property("start", "0");
                        playlist_next_up_fired = false;
                        playlist_next_up_armed = false;
                        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
                        intro_show_fired = false;
                        intro_hide_fired = false;
                        intro_start_ticks = 0;
                        intro_end_ticks   = 0;

                        let (new_sid, new_msid) = {
                            let (sid, msid) = client.get_playback_info(&items[current_idx].id, &log);
                            client.report_start(&items[current_idx], &msid, &sid, &log);
                            (sid, msid)
                        };
                        if client.chapter_api_available {
                            if let Some((s, e)) = client.get_intro_times(&items[current_idx].id, &log) {
                                intro_start_ticks = s;
                                intro_end_ticks   = e;
                            }
                        }
                        *current_item_id.lock().unwrap() = items[current_idx].id.clone();
                        *current_msid.lock().unwrap()    = new_msid;
                        *session_id.lock().unwrap()      = new_sid;
                        current_osd_title = items[current_idx].display_name();
                        let next = &items[current_idx];
                        if next.playback_position_ticks > 0 {
                            pending_resume_secs = Some(next.resume_seconds());
                        }
                        let _ = event_tx.send(PlayerEvent::TrackChanged(current_idx));
                    }
                    Some(Ok(Event::ClientMessage(args))) if args.first().copied() == Some("mbv-next-up-play") => {
                        log.push(Level::Info, "player", "next-up: mbv-next-up-play received from Lua");
                        let _ = event_tx.send(PlayerEvent::NextUpPlay);
                    }
                    Some(Ok(Event::ClientMessage(args))) if args.first().copied() == Some("mbv-skip-intro-play") => {
                        let _ = event_tx.send(PlayerEvent::SkipIntroPlay);
                    }
                    Some(Ok(Event::ClientMessage(args))) if use_mpv_config && args.first().copied() == Some("mouse-moved") => {
                        let show = last_mouse_osd.is_none_or(|t: Instant| t.elapsed() > Duration::from_secs(3));
                        if show {
                            let _ = mpv.command("show-text", &[&current_osd_title, "2000"]);
                            last_mouse_osd = Some(Instant::now());
                        }
                    }
                    Some(Ok(Event::Shutdown)) => {
                        if !stop_reported {
                            drop(progress_handle.take()); // detach; don't block on HTTP call
                            let id   = current_item_id.lock().unwrap().clone();
                            let msid = current_msid.lock().unwrap().clone();
                            let sid  = session_id.lock().unwrap().clone();
                            client.report_stopped(&id, &msid, last_valid_pos, &sid, &log);
                        }
                        status.lock().unwrap().active = false;
                        let _ = event_tx.send(PlayerEvent::Stopped { idx: current_idx, position_ticks: last_valid_pos });
                        return;
                    }
                    Some(Err(e)) => {
                        log.push(Level::Warn, "player", format!("event error: {}", mpv_err_str(&e)));
                    }
                    _ => {}
                }
            }
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
    pub subs_off: Arc<AtomicBool>,
    inner: PlayerProxyInner,
}

impl PlayerProxy {
    #[cfg(test)]
    pub fn stub(status: Arc<Mutex<PlayerStatus>>) -> Self {
        let (tx, _rx) = std::sync::mpsc::channel();
        let player = Player::new(String::new(), String::new(), false, false, false, false, false, true, tx, None);
        let subs_off = player.subs_off.clone();
        PlayerProxy { always_play_next: false, status, subs_off, inner: PlayerProxyInner::Local(player) }
    }

    pub fn local(player: Player, always_play_next: bool) -> Self {
        let status = player.status.clone();
        let subs_off = player.subs_off.clone();
        PlayerProxy { always_play_next, status, subs_off, inner: PlayerProxyInner::Local(player) }
    }

    pub fn remote(remote: crate::remote_player::RemotePlayer, always_play_next: bool) -> Self {
        let status = remote.status.clone();
        PlayerProxy { always_play_next, status, subs_off: Arc::new(AtomicBool::new(true)), inner: PlayerProxyInner::Remote(remote) }
    }

    pub fn play(&self, item: &MediaItem, client: Arc<EmbyClient>, log: AppLog, initial_volume: u8) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.play(item, client, log, initial_volume),
            PlayerProxyInner::Remote(r) => r.play(item, client, log, initial_volume),
        }
    }

    pub fn play_playlist(
        &self,
        items: Vec<MediaItem>,
        start_idx: usize,
        client: Arc<EmbyClient>,
        log: AppLog,
        initial_volume: u8,
    ) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.play_playlist(items, start_idx, client, log, initial_volume),
            PlayerProxyInner::Remote(r) => r.play_playlist(items, start_idx, client, log, initial_volume),
        }
    }

    pub fn stop(&self) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.stop(),
            PlayerProxyInner::Remote(r) => r.stop(),
        }
    }

    pub fn join(&self) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.join(),
            PlayerProxyInner::Remote(r) => r.join(),
        }
    }

    pub fn send_command(&self, cmd: PlayerCommand) {
        match &self.inner {
            PlayerProxyInner::Local(p) => p.send_command(cmd),
            PlayerProxyInner::Remote(r) => r.send_command(cmd),
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.inner, PlayerProxyInner::Remote(_))
    }

    pub fn is_remote_disconnected(&self) -> bool {
        match &self.inner {
            PlayerProxyInner::Local(_) => false,
            PlayerProxyInner::Remote(r) => r.is_disconnected(),
        }
    }
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

    // ── is_english ───────────────────────────────────────────────────────────

    #[test]
    fn english_language_codes() {
        assert!(is_english("en"));
        assert!(is_english("eng"));
        assert!(is_english("en-US"));
        assert!(is_english("en-GB"));
    }

    #[test]
    fn english_case_insensitive() {
        assert!(is_english("EN"));
        assert!(is_english("ENG"));
        assert!(is_english("English"));
        assert!(is_english("ENGLISH"));
        assert!(is_english("English (Stereo)"));
        assert!(is_english("English 5.1"));
    }

    #[test]
    fn non_english_rejected() {
        assert!(!is_english("fr"));
        assert!(!is_english("fra"));
        assert!(!is_english("French"));
        assert!(!is_english("deu"));
        assert!(!is_english("German"));
        assert!(!is_english("jpn"));
        assert!(!is_english("Japanese"));
        assert!(!is_english("#1"));
        assert!(!is_english(""));
    }

    // ── PlayerCommand serde (IPC protocol integrity) ─────────────────────────

    fn make_media_item(id: &str) -> crate::api::MediaItem {
        crate::api::MediaItem {
            id: id.into(), name: "Test Episode".into(), item_type: "Episode".into(),
            is_folder: false, media_type: "Video".into(), collection_type: String::new(),
            runtime_ticks: 3600 * crate::api::TICKS_PER_SECOND,
            played: false, playback_position_ticks: 0,
            series_id: "series1".into(), series_name: "Show".into(), album_id: String::new(),
            index_number: 2, parent_index_number: 1,
            unplayed_item_count: 0,
            path: String::new(), artist: String::new(), sort_name: String::new(),
            production_year: 0, end_year: 0, overview: String::new(),
            premiere_date: String::new(), total_count: 0, container: String::new(),
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
}
