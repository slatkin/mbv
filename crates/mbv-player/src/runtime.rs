use super::{
    Format, IntroState, Mpv, OsStr, Path, PathBuf, PlayerError, PlayerEvent, PlayerStatus,
    SessionReporter, fs, mpv_err_str,
};
use mbv_core::applog as app_logging;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

pub(super) struct ProgressGuard {
    pub(super) stop_tx: mpsc::Sender<()>,
    pub(super) handle: Option<thread::JoinHandle<()>>,
}

impl ProgressGuard {
    pub(super) fn stop_and_join(&mut self, budget: Duration) {
        let _ = self.stop_tx.send(());
        if let Some(h) = self.handle.take() {
            let start = std::time::Instant::now();
            let result = mbv_net::bounded::run_with_hard_bound(
                move || {
                    let _ = h.join();
                    Ok::<(), String>(())
                },
                budget,
            );
            let elapsed = start.elapsed();
            match result {
                Ok(()) => {
                    tracing::info!(name: "player.progress_join.completed", target: "player", elapsed_ms = elapsed.as_millis(), budget_ms = budget.as_millis(), "progress reporter joined");
                }
                Err(e) => {
                    tracing::warn!(name: "player.progress_join.failed", target: "player", error = %e, elapsed_ms = elapsed.as_millis(), budget_ms = budget.as_millis(), "progress reporter join failed");
                }
            }
        }
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "mpv run options are independently configurable properties (design analysis, issue #804)"
)]
pub(super) struct MpvRunConfig {
    pub(super) headless: bool,
    pub(super) use_mpv_config: bool,
    pub(super) video_cache_forward_mb: u32,
    pub(super) video_cache_back_mb: u32,
    pub(super) no_scripts: bool,
    pub(super) always_skip_intro: bool,
    pub(super) audio_pipe_path: Option<String>,
    pub(super) audio_pipe_samplerate: u32,
    pub(super) audio_pipe_bitdepth: u8,
    /// Mutually exclusive with `audio_pipe_path`: set only when pipe output
    /// is not selected for this run (see `resolve_run_output`).
    pub(super) audio_device: Option<String>,
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
    if let Some(path) = user_conf
        && let Ok(text) = fs::read_to_string(path)
    {
        for line in text.lines() {
            if !is_mpv_ipc_config_line(line) {
                sanitized.push_str(line);
                sanitized.push('\n');
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

fn reset_private_mpv_config_dir(private_dir: &Path) -> Result<(), PlayerError> {
    match fs::symlink_metadata(private_dir) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {
            fs::remove_dir_all(private_dir)
                .map_err(|e| PlayerError::remove_directory(private_dir.display().to_string(), e))?;
        }
        Ok(_) => {
            fs::remove_file(private_dir)
                .map_err(|e| PlayerError::remove_path(private_dir.display().to_string(), e))?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(PlayerError::inspect_directory(
                private_dir.display().to_string(),
                e,
            ));
        }
    }
    fs::create_dir_all(private_dir)
        .map_err(|e| PlayerError::create_directory(private_dir.display().to_string(), e))
}

fn prepare_mpv_config_dir(use_mpv_config: bool, ipc_path: &str) -> Result<PathBuf, PlayerError> {
    let private_dir = mbv_config::mpv_config_dir();
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
                        tracing::warn!(name: "player.config.link_failed", target: "player", { file.path = %src.display(), error = %e }, "failed to link mpv config entry");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(name: "player.config.read_failed", target: "player", { file.path = %user_dir.display(), error = %e }, "cannot read user config directory");
            }
        }
    }

    let user_conf = user_dir
        .as_ref()
        .map(|dir| dir.join("mpv.conf"))
        .filter(|path| path.exists());
    let conf = sanitized_mpv_conf(user_conf.as_deref(), ipc_path);
    fs::write(private_dir.join("mpv.conf"), conf)
        .map_err(|e| PlayerError::write_config(private_dir.display().to_string(), e))?;

    Ok(private_dir)
}

// Ensures `path` exists as a FIFO this user may use exclusively: creates it
// owner-only (mkfifo 0600) when missing, and refuses to use an existing FIFO
// owned by another uid or granting group or other access (issue #918 — mpv
// then writes raw PCM of this session only into its own private pipe).
// Refuses to touch a path that exists but isn't a FIFO, without repairing it.
pub(crate) fn ensure_pipe(path: &str) -> Result<(), PlayerError> {
    use std::os::unix::fs::FileTypeExt;
    match std::fs::metadata(path) {
        Ok(meta) if meta.file_type().is_fifo() => {
            // SAFETY: `libc::getuid` has no precondition beyond FFI itself.
            check_private_fifo(path, &meta, unsafe { libc::getuid() })
        }
        Ok(_) => Err(PlayerError::pipe_not_fifo(path)),
        Err(_) => {
            let cpath = std::ffi::CString::new(path)?;
            // SAFETY: `cpath` is NUL-terminated and remains alive for the call.
            let rc = unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) };
            if rc != 0 {
                Err(PlayerError::make_pipe(
                    path,
                    std::io::Error::last_os_error(),
                ))
            } else {
                Ok(())
            }
        }
    }
}

/// Refuses an existing FIFO this user must not attach mpv to: one owned by
/// another `uid`, or one granting group or other access. `uid` is a
/// parameter so a test can cover the foreign-owner case without a second
/// account (mirrors `check_private_dir` in mbv-config).
pub(crate) fn check_private_fifo(
    path: &str,
    meta: &std::fs::Metadata,
    uid: u32,
) -> Result<(), PlayerError> {
    use std::os::unix::fs::MetadataExt;
    let owned_by = meta.uid();
    if owned_by != uid {
        return Err(PlayerError::unsafe_fifo(
            path,
            format!("owned by uid {owned_by}, expected {uid}"),
        ));
    }
    if meta.mode() & 0o077 != 0 {
        return Err(PlayerError::unsafe_fifo(
            path,
            "grants group or other access".to_string(),
        ));
    }
    Ok(())
}

/// Resolve the chosen mpv overlay script and warn about an ignored leftover
/// installer copy. `None` means no script is handed to mpv (the chosen one
/// is missing), so the fonts are not resolved either.
fn resolve_overlay_scripts() -> Option<PathBuf> {
    let source = mbv_config::osc_script_source();
    let script = source.chosen;
    if !script.exists() {
        tracing::warn!(
            name: "player.overlay_script.missing",
            target: "player",
            { file.path = %script.display() },
            "resolved mpv overlay script does not exist; mpv will run with no overlay scripts"
        );
        return None;
    }
    tracing::info!(name: "player.overlay_script.resolved", target: "player", { file.path = %script.display() }, "resolved mpv overlay script");
    if let Some(legacy) = source.unused_legacy {
        tracing::warn!(
            name: "player.overlay_script.legacy_ignored",
            target: "player",
            { legacy_path = %legacy.display(), file.path = %script.display() },
            "ignoring leftover installer script copy"
        );
    }
    Some(script)
}

/// Resolve the chosen mpv overlay fonts directory and warn about an ignored
/// leftover installer copy.
fn resolve_overlay_fonts() -> PathBuf {
    let source = mbv_config::osc_fonts_source();
    let fonts = source.chosen;
    tracing::info!(name: "player.overlay_fonts.resolved", target: "player", { file.path = %fonts.display() }, "resolved mpv overlay fonts");
    if let Some(legacy) = source.unused_legacy {
        tracing::warn!(
            name: "player.overlay_fonts.legacy_ignored",
            target: "player",
            { legacy_path = %legacy.display(), file.path = %fonts.display() },
            "ignoring leftover installer font directory"
        );
    }
    fonts
}

/// Post-init demuxer cache budgets (and, when mbv owns the mpv config, the
/// hwdec policy). Set after init so a user's mpv.conf cannot override them.
fn configure_caches(mpv: &Mpv, config: &MpvRunConfig) {
    if config.headless {
        configure_headless_caches(mpv);
        return;
    }
    configure_video_caches(mpv, config);
    if !config.use_mpv_config
        && let Err(e) = mpv.set_property("hwdec", "auto-safe")
    {
        tracing::warn!(name: "player.hardware_decode.configure_failed", target: "player", error = %e, "failed to configure hardware decode policy");
    }
}

/// Headless budgets: no video window, so the audio-sized demuxer cache and an
/// audio-only display policy replace the video-sized budgets.
fn configure_headless_caches(mpv: &Mpv) {
    let _ = mpv.set_property("vo", "null");
    let _ = mpv.set_property("force-window", "no");
    // #656: with vo=null, attached cover art would still be selected and
    // decoded (video/image=true per audio track) for no benefit.
    let _ = mpv.set_property("audio-display", "no");
    // Audio-sized demuxer cache: a headless host has no video window to
    // justify the video-sized budget below.
    if let Err(e) = mpv.set_property("demuxer-max-bytes", "10M") {
        tracing::warn!(name: "player.cache.configure_failed", target: "player", cache = "headless_forward", error = %e, "failed to configure mpv cache");
    }
    if let Err(e) = mpv.set_property("demuxer-max-back-bytes", "10M") {
        tracing::warn!(name: "player.cache.configure_failed", target: "player", cache = "headless_back", error = %e, "failed to configure mpv cache");
    }
}

fn configure_video_caches(mpv: &Mpv, config: &MpvRunConfig) {
    if let Err(e) = mpv.set_property(
        "demuxer-max-bytes",
        format!("{}M", config.video_cache_forward_mb),
    ) {
        tracing::warn!(name: "player.cache.configure_failed", target: "player", cache = "video_forward", error = %e, "failed to configure mpv cache");
    }
    if let Err(e) = mpv.set_property(
        "demuxer-max-back-bytes",
        format!("{}M", config.video_cache_back_mb),
    ) {
        tracing::warn!(name: "player.cache.configure_failed", target: "player", cache = "video_back", error = %e, "failed to configure mpv cache");
    }
}

/// Configure the audio output after init: PCM to `audio_pipe_path` when set
/// (mutually exclusive with `audio_device`), else a clocked ALSA device.
/// Returns whether startup must be pre-paused so the pipewriter's first read
/// attaches before playback starts.
fn configure_audio_output(mpv: &Mpv, config: &MpvRunConfig) -> Result<bool, PlayerError> {
    let armed = if let Some(path) = &config.audio_pipe_path {
        configure_audio_pipe(mpv, path, config)
    } else if let Some(device) = &config.audio_device {
        // Clocked ALSA output: the device identifier alone selects the
        // backend, so `ao` is left to mpv's own negotiation.
        if let Err(e) = mpv.set_property("audio-device", device.as_str()) {
            return Err(PlayerError::set_audio_device(device, e));
        }
        tracing::info!(name: "player.audio_output.configured", target: "player", device = %device, "using clocked ALSA audio output");
        false
    } else {
        false
    };
    if !armed {
        return Ok(false);
    }
    if let Err(e) = mpv.set_property("pause", true) {
        tracing::warn!(name: "player.audio_pipe.pause_failed", target: "player", error = %mpv_err_str(&e), "failed to pre-pause startup");
        return Ok(false);
    }
    Ok(true)
}

/// Configure the pipewriter output: PCM format, channels, samplerate and
/// resampler. Returns `true` only when every output property was accepted.
fn configure_audio_pipe(mpv: &Mpv, path: &str, config: &MpvRunConfig) -> bool {
    if let Err(e) = ensure_pipe(path) {
        tracing::warn!(name: "player.audio_pipe.disabled", target: "player", error = %e, "audio pipe disabled for this session");
        return false;
    }
    let rate = config.audio_pipe_samplerate.to_string();
    let (bitdepth, audio_format) = audio_pipe_format(config.audio_pipe_bitdepth);
    let failed = set_audio_pipe_properties(mpv, path, audio_format, &rate);
    log_audio_pipe_outcome(path, &rate, bitdepth, &failed)
}

/// Maps the configured bit depth onto the mpv audio format string.
fn audio_pipe_format(bitdepth: u8) -> (u8, &'static str) {
    match bitdepth {
        16 => (16u8, "s16"),
        24 => (24u8, "s24"),
        _ => (32u8, "s32"),
    }
}

/// Sets the PCM output properties, collecting `property: error` strings for
/// every one mpv rejected.
fn set_audio_pipe_properties(mpv: &Mpv, path: &str, audio_format: &str, rate: &str) -> Vec<String> {
    let mut failed = Vec::new();
    if let Err(e) = mpv.set_property("ao", "pcm") {
        failed.push(format!("ao: {}", mpv_err_str(&e)));
    }
    if let Err(e) = mpv.set_property("ao-pcm-file", path) {
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
    if let Err(e) = mpv.set_property("audio-samplerate", rate) {
        failed.push(format!("audio-samplerate: {}", mpv_err_str(&e)));
    }
    if let Err(e) = mpv.set_property("audio-swresample-o", "resampler=soxr,precision=28") {
        failed.push(format!("audio-swresample-o: {}", mpv_err_str(&e)));
    }
    failed
}

fn log_audio_pipe_outcome(path: &str, rate: &str, bitdepth: u8, failed: &[String]) -> bool {
    if failed.is_empty() {
        tracing::info!(name: "player.audio_pipe.configured", target: "player", { sample_rate_hz = %rate, bit_depth = bitdepth, file.path = %path }, "writing stereo PCM; blocks until a reader attaches");
        true
    } else {
        tracing::warn!(name: "player.audio_pipe.configure_failed", target: "player", { file.path = %path, failures = %failed.join(", ") }, "failed to configure PCM output");
        false
    }
}

/// Removes a leftover IPC socket and records what was found.
fn prepare_ipc_socket(ipc_path: &str) {
    let ipc_existed = Path::new(ipc_path).exists();
    if ipc_existed {
        let _ = std::fs::remove_file(ipc_path);
        tracing::info!(name: "player.ipc_socket.stale_removed", target: "player", { file.path = %ipc_path }, "removed stale IPC socket");
    }
    tracing::info!(name: "player.ipc_socket.checked", target: "player", { file.path = %ipc_path, existed = ipc_existed }, "checked IPC socket path");
}

/// Records which overlay-script policy this run uses. `no_scripts` wins over
/// `use_mpv_config` (both disable mbv's own scripts, for different reasons).
fn log_overlay_script_policy(no_scripts: bool, use_mpv_config: bool) {
    if no_scripts {
        tracing::warn!(name: "player.overlay_script.disabled", target: "player", { file.path = %mbv_config::osc_script_source().chosen.display() }, "mpv overlay scripts disabled by config and will not be handed to mpv");
    } else if use_mpv_config {
        tracing::warn!(name: "player.overlay_script.user_managed", target: "player", "user mpv config manages scripts; mbv hands mpv no overlay scripts");
    }
}

pub(super) fn init_mpv(config: &MpvRunConfig) -> Result<(Mpv, bool), PlayerError> {
    let ipc_path = mbv_config::mpv_ipc_path();
    let private_config_dir = prepare_mpv_config_dir(config.use_mpv_config, &ipc_path)?;
    prepare_ipc_socket(&ipc_path);

    let no_scripts = config.no_scripts;
    let use_mpv_config = config.use_mpv_config;
    log_overlay_script_policy(no_scripts, use_mpv_config);
    let mut init_err: Option<String> = None;
    let mpv = match Mpv::with_initializer(|init| {
        macro_rules! opt {
            ($k:expr_2021, $v:expr_2021) => {{
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
        opt!("gapless-audio", "weak");
        if no_scripts || !use_mpv_config {
            opt!("load-scripts", "no");
            opt!("osc", "no");
            opt!("osd-bar", "no");
        }
        if !no_scripts
            && !use_mpv_config
            && let Some(script) = resolve_overlay_scripts()
        {
            opt!("scripts", script.to_str().unwrap_or(""));
            let fonts = resolve_overlay_fonts();
            opt!("osd-fonts-dir", fonts.to_str().unwrap_or(""));
        }
        Ok(())
    }) {
        Ok(m) => m,
        Err(e) => {
            let msg =
                init_err.unwrap_or_else(|| format!("[player] mpv init error: {}", mpv_err_str(&e)));
            return Err(PlayerError::mpv_init(msg, e));
        }
    };

    // SAFETY: mpv.ctx is a live context owned by `mpv`, and `log_level` is a valid C string.
    unsafe {
        let log_level = if cfg!(debug_assertions) {
            c"warn"
        } else {
            c"error"
        };
        libmpv2_sys::mpv_request_log_messages(mpv.ctx.as_ptr(), log_level.as_ptr().cast())
    };

    configure_caches(&mpv, config);
    let startup_pause_armed = configure_audio_output(&mpv, config)?;

    Ok((mpv, startup_pause_armed))
}

pub(super) fn init_volume(mpv: &Mpv, status: &Arc<Mutex<PlayerStatus>>, initial_volume: u8) {
    let mut st = status.lock().unwrap();
    let raw_max = mpv.get_property::<i64>("volume-max").unwrap_or(130);
    st.volume_max = raw_max * raw_max / 100;
    let v = i64::from(initial_volume).clamp(0, st.volume_max);
    let raw = mbv_emby_model::saturating_i64_from_f64(
        (10.0 * mbv_emby_model::i64_to_f64_saturating(v).sqrt()).round(),
    );
    let _ = mpv.set_property("volume", mbv_emby_model::i64_to_f64_saturating(raw));
    st.volume = v;
}

pub(super) fn observe_properties(mpv: &Mpv, use_mpv_config: bool) {
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

pub(super) fn spawn_progress_reporter(reporter: SessionReporter) -> ProgressGuard {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let interval = Duration::from_secs(reporter.client.config.progress_interval_secs);
    // The progress reporter outlives the slot, so it must not inherit the
    // spawning slot's `playback` span — dispatcher only (design D5).
    let handle = thread::spawn(app_logging::carry_dispatcher(move || {
        loop {
            match stop_rx.recv_timeout(interval) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    reporter.report_progress("TimeUpdate");
                    reporter.report_ping();
                }
            }
        }
    }));
    ProgressGuard {
        stop_tx,
        handle: Some(handle),
    }
}

pub(super) fn handle_intro(
    ticks: i64,
    start: i64,
    end: i64,
    intro_state: &mut IntroState,
    always_skip: bool,
    mpv: &Mpv,
    event_tx: &mpsc::Sender<PlayerEvent>,
) {
    if end <= start {
        return;
    }
    if intro_state.is_pending() && ticks >= start {
        intro_state.shown();
        if ticks < end {
            let end_secs = mbv_emby_model::ticks_to_seconds(end);
            if always_skip {
                let _ = mpv.set_property("time-pos", end_secs);
            } else {
                let _ = event_tx.send(PlayerEvent::IntroStarted {
                    intro_end_ticks: end,
                });
                let _ = mpv.command("script-message", &["mbv-skip-intro", &end_secs.to_string()]);
            }
        } else {
            intro_state.dismissed();
        }
    }
    if intro_state == &IntroState::Shown && ticks >= end {
        intro_state.dismissed();
        let _ = event_tx.send(PlayerEvent::IntroEnded);
        let _ = mpv.command("script-message", &["mbv-skip-intro-dismiss"]);
    }
}

// ── PlaybackRun ─────────────────────────────────────────────────────────

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
