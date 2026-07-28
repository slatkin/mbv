use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

pub const BAR_COUNT: usize = 64;
const ASCII_MAX: u16 = 1000;
const MAX_FRAME_BYTES: usize = 4096;
const FRAME_QUEUE_CAPACITY: usize = 2;
const STARTUP_TIMEOUT: Duration = Duration::from_millis(500);
const POLL_INTERVAL_MS: i32 = 100;
const CHILD_STARTUP_GRACE: Duration = Duration::from_millis(100);
static RESOURCE_COUNTER: AtomicU64 = AtomicU64::new(0);

enum Startup {
    Started,
    Failed(String),
}

#[derive(Clone, Debug)]
pub enum CavaInput {
    System,
    Fifo(PathBuf),
}

/// Owns one CAVA child, its private raw-output FIFO, and the bounded frame queue.
pub struct CavaWorker {
    stop_tx: mpsc::Sender<()>,
    frames_rx: Receiver<Vec<f32>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl CavaWorker {
    pub fn start() -> Result<Self, String> {
        Self::start_with_input(CavaInput::System)
    }

    pub fn start_with_input(input: CavaInput) -> Result<Self, String> {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (startup_tx, startup_rx) = mpsc::channel();
        let (frames_tx, frames_rx) = mpsc::sync_channel(FRAME_QUEUE_CAPACITY);
        let handle = thread::spawn(move || run_worker(stop_rx, startup_tx, frames_tx, input));

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

    pub fn take_latest_frame(&self) -> Result<Option<Vec<f32>>, ()> {
        let mut latest = None;
        loop {
            match self.frames_rx.try_recv() {
                Ok(frame) => latest = Some(frame),
                Err(TryRecvError::Empty) => return Ok(latest),
                Err(TryRecvError::Disconnected) => return Err(()),
            }
        }
    }

    pub fn stop(&mut self) {
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
    input: CavaInput,
) {
    let resources = match create_private_resources(&input) {
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
                "failed to open CAVA output FIFO {}: {error}",
                resources.fifo.display()
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
    thread::sleep(CHILD_STARTUP_GRACE);
    if let Ok(Some(status)) = child_guard.try_wait() {
        let diagnostics = child_guard.take_stderr();
        let _ = startup_tx.send(Startup::Failed(format!(
            "CAVA exited during startup with {status}: {}",
            diagnostics.trim()
        )));
        drop(fifo);
        cleanup_private_resources(&resources);
        return;
    }
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

fn create_private_resources(input: &CavaInput) -> io::Result<PrivateResources> {
    let suffix = RESOURCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir_name = format!("mbv-cava-{}-{suffix}", std::process::id());
    let mut dir = base.join(&dir_name);
    if let Err(error) = fs::create_dir(&dir) {
        let fallback = std::env::temp_dir().join(&dir_name);
        if base == std::env::temp_dir() {
            return Err(error);
        }
        fs::create_dir(&fallback)?;
        dir = fallback;
    }
    #[cfg(unix)]
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;

    let fifo = dir.join("spectrum.fifo");
    let config = dir.join("config");
    let output_path = fifo.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CAVA FIFO path is not valid UTF-8",
        )
    })?;
    let cpath = std::ffi::CString::new(output_path)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "CAVA FIFO path contains NUL"))?;
    let result = unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) };
    if result != 0 && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists {
        let error = io::Error::last_os_error();
        let _ = fs::remove_dir(&dir);
        return Err(error);
    }
    let input_config = match input {
        CavaInput::System => "method = pulse".to_string(),
        CavaInput::Fifo(path) => format!("method = fifo\nsource = {}", path.display()),
    };
    if let Err(error) = fs::write(&config, cava_config(output_path, &input_config)) {
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

fn cava_config(raw_target: &str, input: &str) -> String {
    format!(
        "[general]\nframerate = 60\nbars = {BAR_COUNT}\nautosens = 1\nsensitivity = 200\n\n[input]\n{input}\n\n[output]\nmethod = raw\nraw_target = {raw_target}\ndata_format = ascii\nascii_max_range = {ASCII_MAX}\nbar_delimiter = 59\nframe_delimiter = 10\nchannels = mono\n"
    )
}

fn spawn_cava(config: &Path) -> io::Result<Child> {
    Command::new("cava")
        .arg("-p")
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
}

/// Ensures the supervised CAVA child is terminated and reaped even if the
/// worker thread unwinds before the main loop completes.
struct ChildGuard {
    child: Option<Child>,
    diagnostics: Arc<Mutex<String>>,
}

impl ChildGuard {
    fn new(mut child: Child) -> Self {
        let diagnostics = Arc::new(Mutex::new(String::new()));
        if let Some(mut stderr) = child.stderr.take() {
            let captured = diagnostics.clone();
            thread::spawn(move || {
                let mut bytes = Vec::new();
                let _ = std::io::Read::by_ref(&mut stderr)
                    .take(8192)
                    .read_to_end(&mut bytes);
                let text = String::from_utf8_lossy(&bytes);
                if let Ok(mut output) = captured.lock() {
                    output.push_str(&text);
                }
            });
        }
        Self {
            child: Some(child),
            diagnostics,
        }
    }

    fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        self.child
            .as_mut()
            .expect("CAVA child present until drop")
            .try_wait()
    }

    fn take_stderr(&mut self) -> String {
        self.diagnostics
            .lock()
            .map(|output| output.clone())
            .unwrap_or_default()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            terminate_child(&mut child);
        }
    }
}

/// Checks static requirements without starting any audio process.
pub fn daemon_spectrum_prerequisites(config: &crate::config::Config) -> Result<(), String> {
    if !config.audio_pipe_enabled {
        return Err("daemon spectrum requires audio_pipe_enabled = true".to_string());
    }
    for executable in ["cava", "snapclient"] {
        if !executable_in_path(executable) {
            return Err(format!(
                "required executable '{executable}' was not found in PATH"
            ));
        }
    }
    Ok(())
}

fn executable_in_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(name))
        .any(|path| {
            path.is_file() && {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    path.metadata()
                        .map(|m| m.permissions().mode() & 0o111 != 0)
                        .unwrap_or(false)
                }
                #[cfg(not(unix))]
                {
                    true
                }
            }
        })
}

pub struct SpectrumSnapclient {
    _guard: ChildGuard,
    fifo: PathBuf,
}

impl SpectrumSnapclient {
    pub fn start(config: &crate::config::Config) -> Result<Self, String> {
        let fifo = PathBuf::from(&config.spectrum_fifo_path);
        let path = fifo
            .to_str()
            .ok_or_else(|| "spectrum FIFO path is not valid UTF-8".to_string())?;
        let cpath = std::ffi::CString::new(path)
            .map_err(|_| "spectrum FIFO path contains NUL".to_string())?;
        match unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) } {
            0 => {}
            _ if io::Error::last_os_error().kind() == io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&fifo);
                let result = unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) };
                if result != 0 {
                    return Err(format!(
                        "failed to recreate spectrum FIFO {path}: {}",
                        io::Error::last_os_error()
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "failed to create spectrum FIFO {path}: {}",
                    io::Error::last_os_error()
                ))
            }
        }
        let child = match Command::new("snapclient")
            .arg("--host")
            .arg(&config.spectrum_snapserver_host)
            .arg("--port")
            .arg(config.spectrum_snapserver_port.to_string())
            .arg("--hostID")
            .arg(&config.spectrum_snapclient_host_id)
            .arg("--instance")
            .arg(config.spectrum_snapclient_instance.to_string())
            .arg("--player")
            .arg(format!("file:filename={path},mode=w"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                let _ = fs::remove_file(&fifo);
                return Err(format!("failed to start snapclient: {error}"));
            }
        };
        let mut guard = ChildGuard::new(child);
        thread::sleep(CHILD_STARTUP_GRACE);
        if let Ok(Some(status)) = guard.try_wait() {
            let diagnostics = guard.take_stderr();
            let _ = fs::remove_file(&fifo);
            return Err(format!(
                "snapclient exited during startup with {status}: {}",
                diagnostics.trim()
            ));
        }
        Ok(Self {
            _guard: guard,
            fifo,
        })
    }

    pub fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        self._guard.try_wait()
    }

    pub fn diagnostics(&mut self) -> String {
        self._guard.take_stderr()
    }
}

impl Drop for SpectrumSnapclient {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.fifo);
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

#[cfg(test)]
mod tests {
    use super::{
        cava_config, cleanup_private_resources, create_private_resources, parse_ascii_frame,
        spawn_cava, terminate_child, CavaInput, SpectrumSnapclient, BAR_COUNT,
    };

    fn frame(value: &str) -> Vec<u8> {
        std::iter::repeat_n(value, BAR_COUNT)
            .collect::<Vec<_>>()
            .join(";")
            .into_bytes()
    }

    #[test]
    fn cava_config_uses_default_pulse_input_without_source_or_routing() {
        let config = cava_config("/tmp/private-cava.fifo", "method = pulse");
        assert!(config.contains("method = pulse"));
        assert!(config.contains("method = raw"));
        assert!(config.contains("raw_target = /tmp/private-cava.fifo"));
        assert!(config.contains("autosens = 1"));
        assert!(config.contains("sensitivity = 200"));
        assert!(!config.contains("source ="));
        assert!(!config.contains("module-") && !config.contains("audio-device"));
    }

    #[test]
    fn cava_config_supports_daemon_fifo_input() {
        let config = cava_config(
            "/tmp/private-cava.fifo",
            "method = fifo\nsource = /tmp/mbv-spectrum.fifo",
        );
        assert!(config.contains("method = fifo"));
        assert!(config.contains("source = /tmp/mbv-spectrum.fifo"));
        assert!(config.contains("method = raw"));
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
        let resources =
            create_private_resources(&CavaInput::System).expect("private CAVA resources");
        let dir = resources.dir.clone();
        cleanup_private_resources(&resources);
        assert!(!dir.exists());
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

        let path = create_private_resources(&CavaInput::System).expect("private CAVA resources");
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
    fn snapclient_start_failure_removes_configured_fifo() {
        use std::env;

        let original_path = env::var_os("PATH");
        let mut empty_path = env::temp_dir();
        empty_path.push("mbv-snapclient-empty-path");
        let _ = std::fs::create_dir_all(&empty_path);
        env::set_var("PATH", &empty_path);

        let mut config = crate::config::Config::default();
        config.spectrum_fifo_path = env::temp_dir()
            .join(format!("mbv-spectrum-test-{}.fifo", std::process::id()))
            .display()
            .to_string();
        let fifo = std::path::PathBuf::from(&config.spectrum_fifo_path);
        let result = SpectrumSnapclient::start(&config);

        match original_path {
            Some(value) => env::set_var("PATH", value),
            None => env::remove_var("PATH"),
        }

        assert!(result.is_err());
        assert!(!fifo.exists());
    }
}
