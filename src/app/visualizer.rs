use super::App;
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const BAR_COUNT: usize = 64;
const ASCII_MAX: u16 = 1000;
const MAX_FRAME_BYTES: usize = 4096;
const FRAME_QUEUE_CAPACITY: usize = 2;
const STARTUP_TIMEOUT: Duration = Duration::from_millis(500);
const POLL_INTERVAL_MS: i32 = 100;
static RESOURCE_COUNTER: AtomicU64 = AtomicU64::new(0);

enum Startup {
    Started,
    Failed(String),
}

/// Owns one CAVA child, its private raw-output FIFO, and the bounded frame queue.
pub(super) struct CavaWorker {
    stop_tx: mpsc::Sender<()>,
    frames_rx: Receiver<Vec<f32>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl CavaWorker {
    pub(super) fn start() -> Result<Self, String> {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (startup_tx, startup_rx) = mpsc::channel();
        let (frames_tx, frames_rx) = mpsc::sync_channel(FRAME_QUEUE_CAPACITY);
        let handle = thread::spawn(move || run_worker(stop_rx, startup_tx, frames_tx));

        match startup_rx.recv_timeout(STARTUP_TIMEOUT) {
            Ok(Startup::Started) => Ok(Self {
                stop_tx,
                frames_rx,
                handle: Some(handle),
            }),
            Ok(Startup::Failed(error)) => {
                let _ = stop_tx.send(());
                let _ = handle.join();
                Err(error)
            }
            Err(error) => {
                let _ = stop_tx.send(());
                let _ = handle.join();
                Err(format!("CAVA startup readiness timed out: {error}"))
            }
        }
    }

    pub(super) fn take_latest_frame(&self) -> Result<Option<Vec<f32>>, ()> {
        let mut latest = None;
        loop {
            match self.frames_rx.try_recv() {
                Ok(frame) => latest = Some(frame),
                Err(TryRecvError::Empty) => return Ok(latest),
                Err(TryRecvError::Disconnected) => return Err(()),
            }
        }
    }

    pub(super) fn stop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.handle.take() {
            let started = Instant::now();
            let join_timeout = Duration::from_millis(500);
            let (done_tx, done_rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = handle.join();
                let _ = done_tx.send(());
            });
            if done_rx.recv_timeout(join_timeout).is_err() {
                log::warn!(
                    target: "visualizer",
                    "CAVA worker did not join within {}ms; detaching",
                    join_timeout.as_millis()
                );
                return;
            }
            log::debug!(
                target: "visualizer",
                "CAVA worker joined in {}ms",
                started.elapsed().as_millis()
            );
        }
    }
}

impl Drop for CavaWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_worker(
    stop_rx: Receiver<()>,
    startup_tx: mpsc::Sender<Startup>,
    frames_tx: SyncSender<Vec<f32>>,
) {
    let resources = match create_private_resources() {
        Ok(resources) => resources,
        Err(error) => {
            let _ = startup_tx.send(Startup::Failed(format!(
                "failed to create private CAVA resources: {error}"
            )));
            return;
        }
    };

    let mut fifo = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&resources.fifo)
    {
        Ok(file) => file,
        Err(error) => {
            let _ = startup_tx.send(Startup::Failed(format!(
                "failed to open private CAVA FIFO: {error}"
            )));
            cleanup_private_resources(&resources);
            return;
        }
    };

    let mut child_guard = match spawn_cava(&resources.config) {
        Ok(child) => ChildGuard::new(child),
        Err(error) => {
            let _ = startup_tx.send(Startup::Failed(format!("failed to start cava: {error}")));
            drop(fifo);
            cleanup_private_resources(&resources);
            return;
        }
    };
    let _ = startup_tx.send(Startup::Started);

    let mut frame = Vec::with_capacity(MAX_FRAME_BYTES.min(BAR_COUNT * 5));
    let mut discarding_frame = false;
    (|| loop {
        if stop_rx.try_recv().is_ok() {
            return;
        }

        if let Some(status) = child_guard.try_wait().ok().flatten() {
            if !status.success() {
                log::warn!(
                    target: "visualizer",
                    "CAVA exited unexpectedly with status {status}"
                );
            } else {
                log::info!(target: "visualizer", "CAVA exited");
            }
            return;
        }

        let mut pollfd = libc::pollfd {
            fd: fifo.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let poll_result = unsafe { libc::poll(&mut pollfd, 1, POLL_INTERVAL_MS) };
        if poll_result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                log::warn!(target: "visualizer", "CAVA FIFO poll failed: {error}");
                return;
            }
            continue;
        }
        if poll_result == 0 || pollfd.revents & libc::POLLIN == 0 {
            continue;
        }

        let mut bytes = [0u8; 1024];
        match fifo.read(&mut bytes) {
            Ok(0) => {
                log::warn!(target: "visualizer", "CAVA closed its raw-output FIFO");
                return;
            }
            Ok(count) => {
                for byte in &bytes[..count] {
                    if discarding_frame {
                        if *byte == b'\n' {
                            discarding_frame = false;
                        }
                        continue;
                    }
                    if *byte == b'\n' {
                        if let Some(parsed) = parse_ascii_frame(&frame) {
                            match frames_tx.try_send(parsed) {
                                Ok(()) | Err(TrySendError::Full(_)) => {}
                                Err(TrySendError::Disconnected(_)) => {
                                    return;
                                }
                            }
                        }
                        frame.clear();
                    } else if frame.len() == MAX_FRAME_BYTES {
                        frame.clear();
                        discarding_frame = true;
                    } else {
                        frame.push(*byte);
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                log::warn!(target: "visualizer", "CAVA FIFO read failed: {error}");
                return;
            }
        }
    })();
    drop(fifo);
    cleanup_private_resources(&resources);
}

struct PrivateResources {
    dir: PathBuf,
    fifo: PathBuf,
    config: PathBuf,
}

fn create_private_resources() -> io::Result<PrivateResources> {
    let suffix = RESOURCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("mbv-cava-{}-{suffix}", std::process::id()));
    fs::create_dir(&dir)?;
    #[cfg(unix)]
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;

    let fifo = dir.join("spectrum.fifo");
    let config = dir.join("config");
    let path = fifo.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CAVA FIFO path is not valid UTF-8",
        )
    })?;
    let cpath = std::ffi::CString::new(path)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "CAVA FIFO path contains NUL"))?;
    let result = unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) };
    if result != 0 {
        let error = io::Error::last_os_error();
        let _ = fs::remove_dir(&dir);
        return Err(error);
    }
    if let Err(error) = fs::write(&config, cava_config(path)) {
        let resources = PrivateResources { dir, fifo, config };
        cleanup_private_resources(&resources);
        return Err(error);
    }
    Ok(PrivateResources { dir, fifo, config })
}

fn cleanup_private_resources(resources: &PrivateResources) {
    if let Err(error) = fs::remove_file(&resources.fifo) {
        if error.kind() != io::ErrorKind::NotFound {
            log::warn!(
                target: "visualizer",
                "failed to remove CAVA FIFO {}: {error}",
                resources.fifo.display()
            );
        }
    }
    if let Err(error) = fs::remove_file(&resources.config) {
        if error.kind() != io::ErrorKind::NotFound {
            log::warn!(
                target: "visualizer",
                "failed to remove CAVA config {}: {error}",
                resources.config.display()
            );
        }
    }
    if let Err(error) = fs::remove_dir(&resources.dir) {
        if error.kind() != io::ErrorKind::NotFound {
            log::warn!(
                target: "visualizer",
                "failed to remove CAVA resource directory {}: {error}",
                resources.dir.display()
            );
        }
    }
}

fn cava_config(raw_target: &str) -> String {
    format!(
        "[general]\nframerate = 60\nbars = {BAR_COUNT}\nautosens = 1\nsensitivity = 200\n\n[input]\nmethod = pulse\n\n[output]\nmethod = raw\nraw_target = {raw_target}\ndata_format = ascii\nascii_max_range = {ASCII_MAX}\nbar_delimiter = 59\nframe_delimiter = 10\nchannels = mono\n"
    )
}

fn spawn_cava(config: &Path) -> io::Result<Child> {
    Command::new("cava")
        .arg("-p")
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

/// Ensures the supervised CAVA child is terminated and reaped even if the
/// worker thread unwinds before the main loop completes.
struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        self.child
            .as_mut()
            .expect("CAVA child present until drop")
            .try_wait()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            terminate_child(&mut child);
        }
    }
}

fn terminate_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn parse_ascii_frame(raw: &[u8]) -> Option<Vec<f32>> {
    let text = std::str::from_utf8(raw).ok()?.trim_end_matches('\r');
    let mut values = text.split(';').collect::<Vec<_>>();
    if values.last().is_some_and(|value| value.is_empty()) {
        values.pop();
    }
    if values.len() != BAR_COUNT {
        return None;
    }
    values
        .into_iter()
        .map(|value| {
            let value = value.parse::<u16>().ok()?;
            (value <= ASCII_MAX).then_some(value as f32 / ASCII_MAX as f32)
        })
        .collect()
}

impl App {
    pub(super) fn sync_visualizer(&mut self) {
        if let Some(worker) = self.visualizer.as_ref() {
            match worker.take_latest_frame() {
                Ok(Some(frame)) => self.visualizer_frame = frame,
                Ok(None) => {}
                Err(()) => {
                    log::warn!(target: "visualizer", "CAVA worker stopped; visualizer disabled for this playback");
                    self.visualizer_failed = true;
                    self.stop_visualizer_worker();
                }
            }
        }

        let is_local = !self.player.is_remote() && self.connected_session_id.is_none();
        let audio_pipe_enabled = self.client.lock().unwrap().config.audio_pipe_enabled;
        let active = self.player.status.lock().unwrap().active;
        let should_run = self.visualizer_enabled && is_local && active && !audio_pipe_enabled;
        if !should_run {
            self.stop_visualizer_worker();
            self.visualizer_failed = false;
            return;
        }
        if self.visualizer.is_none() && !self.visualizer_failed {
            match CavaWorker::start() {
                Ok(worker) => {
                    log::info!(target: "visualizer", "started CAVA system-audio worker");
                    self.visualizer = Some(worker);
                }
                Err(error) => {
                    log::warn!(target: "visualizer", "system-audio visualizer unavailable: {error}");
                    self.visualizer_failed = true;
                }
            }
        }
    }

    pub(super) fn stop_visualizer_worker(&mut self) {
        if let Some(mut worker) = self.visualizer.take() {
            worker.stop();
        }
        self.visualizer_frame.clear();
    }

    pub(super) fn toggle_visualizer(&mut self) {
        self.visualizer_enabled = !self.visualizer_enabled;
        self.visualizer_failed = false;
        if !self.visualizer_enabled {
            self.stop_visualizer_worker();
        } else {
            self.sync_visualizer();
        }
        self.save_prefs();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cava_config, cleanup_private_resources, create_private_resources, parse_ascii_frame,
        spawn_cava, terminate_child, ChildGuard, BAR_COUNT,
    };

    fn frame(value: &str) -> Vec<u8> {
        std::iter::repeat_n(value, BAR_COUNT)
            .collect::<Vec<_>>()
            .join(";")
            .into_bytes()
    }

    #[test]
    fn cava_config_uses_default_pulse_input_without_source_or_routing() {
        let config = cava_config("/tmp/private-cava.fifo");
        assert!(config.contains("method = pulse"));
        assert!(config.contains("method = raw"));
        assert!(config.contains("raw_target = /tmp/private-cava.fifo"));
        assert!(config.contains("autosens = 1"));
        assert!(config.contains("sensitivity = 200"));
        assert!(!config.contains("source ="));
        assert!(!config.contains("module-") && !config.contains("audio-device"));
    }

    #[test]
    fn parser_rejects_incomplete_and_malformed_frames() {
        assert!(parse_ascii_frame(b"1;2").is_none());
        assert!(parse_ascii_frame(&frame("not-a-number")).is_none());
        assert!(parse_ascii_frame(&frame("1001")).is_none());
        assert!(parse_ascii_frame(&frame("1;2")).is_none());
    }

    #[test]
    fn parser_normalizes_complete_frames_and_accepts_trailing_delimiter() {
        let mut raw = frame("500");
        raw.push(b';');
        let parsed = parse_ascii_frame(&raw).expect("complete frame");
        assert_eq!(parsed.len(), BAR_COUNT);
        assert!(parsed
            .iter()
            .all(|value| (*value - 0.5).abs() < f32::EPSILON));
    }

    #[test]
    fn private_resources_are_removed_after_worker_shutdown() {
        let resources = create_private_resources().expect("private CAVA resources");
        let dir = resources.dir.clone();
        cleanup_private_resources(&resources);
        assert!(!dir.exists());
    }

    #[test]
    fn child_guard_terminates_running_process_on_drop() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let child = match Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                eprintln!("skipping child_guard_terminates_running_process_on_drop: {error}");
                return;
            }
        };
        let pid = child.id();
        let guard = ChildGuard::new(child);
        drop(guard);

        let start = Instant::now();
        let mut reaped = false;
        while start.elapsed() < Duration::from_secs(2) {
            let mut cmd = Command::new("kill");
            cmd.arg("-0").arg(pid.to_string());
            if let Ok(status) = cmd.status() {
                if !status.success() {
                    reaped = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(reaped, "ChildGuard did not reap CAVA child after drop");
    }

    #[test]
    fn terminate_child_handles_exited_process_without_blocking() {
        use std::process::{Command, Stdio};

        let mut child = Command::new("/bin/true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn /bin/true");
        let _ = child.wait();
        terminate_child(&mut child);
    }

    #[test]
    fn spawn_cava_reports_missing_executable() {
        use std::env;

        let original_path = env::var_os("PATH");
        let mut with_empty = env::temp_dir();
        with_empty.push("mbv-cava-empty-path");
        let _ = std::fs::create_dir_all(&with_empty);
        env::set_var("PATH", &with_empty);

        let path = create_private_resources().expect("private CAVA resources");
        let result = spawn_cava(&path.config);
        cleanup_private_resources(&path);

        match original_path {
            Some(value) => env::set_var("PATH", value),
            None => env::remove_var("PATH"),
        }

        let err = result.expect_err("spawn must fail when cava is absent");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn audio_pipe_playback_does_not_start_cava() {
        let mut app = crate::app::tests::make_app_stub();
        app.visualizer_enabled = true;
        app.player.status.lock().unwrap().active = true;
        app.client.lock().unwrap().config.audio_pipe_enabled = true;
        app.sync_visualizer();
        assert!(app.visualizer.is_none());
    }

    #[test]
    fn remote_playback_does_not_start_cava() {
        let mut app = crate::app::tests::make_remote_app_stub(Vec::new(), Vec::new());
        app.visualizer_enabled = true;
        app.sync_visualizer();
        assert!(app.visualizer.is_none());
    }
}
