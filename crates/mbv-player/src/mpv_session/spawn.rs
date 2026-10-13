//! Child-process spawn arguments (design D4 of `mpv-child-process`).
//!
//! `spawn_args` turns the option list that `init_mpv` sets through libmpv
//! into `mpv` command-line arguments with the same values. The child-process
//! backend (group 4) starts the child with this list; libmpv stays the live
//! backend until then.

use std::io::{ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child as ProcessChild, Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::mpv_error::MpvError;
use crate::runtime::MpvRunConfig;

/// The path and script inputs that `init_mpv` resolves before it starts the
/// child. `spawn_args` stays a pure function of these values and the run
/// config: it reads no environment and touches no filesystem. `init_mpv`
/// keeps the filesystem work (`prepare_mpv_config_dir`,
/// `prepare_ipc_socket`) and hands the resolved values in.
pub(crate) struct SpawnPaths<'a> {
    /// The IPC socket path; also the value of `--input-ipc-server`, so user
    /// tools that talk to that socket keep working.
    pub(crate) ipc_path: &'a str,
    /// The mbv-owned config directory that `prepare_mpv_config_dir` filled.
    pub(crate) config_dir: &'a Path,
    /// The overlay script and its font directory, when mbv hands mpv its own
    /// overlay scripts (the `use_mpv_config` case with scripts enabled).
    pub(crate) overlay: Option<(&'a Path, &'a Path)>,
}

/// The command line for one `mpv` child process.
///
/// mpv's `mpv_create` (`player/client.c`) applies the `[libmpv]` profile from
/// `etc/builtin.conf` (checked at mpv 0.41) before `mpv_initialize`, and the
/// `mpv` program never applies that profile. The profile sets these defaults
/// for an embedded client, with the program default in parentheses:
///
/// - `config=no` (yes): mbv passes `--config=yes` with its own
///   `--config-dir`, so the child loads the same sanitized config that the
///   embedded client loaded. Nothing extra to restore.
/// - `idle=yes` (no): restored with `--idle=yes`. The child must stay alive
///   with no file and wait for IPC commands.
/// - `terminal=no` (yes): restored with `--no-terminal`.
/// - `input-terminal=no` (yes): restored, so the child never reads key
///   events from standard input.
/// - `osc=no` (yes): `init_mpv` passes `osc=no` whenever mbv manages the
///   scripts; the `use_mpv_config` case leaves scripts and the OSC to the
///   user's config on purpose, so `spawn_args` adds nothing there.
/// - `input-default-bindings=no` (yes) and `input-vo-keyboard=no` (yes):
///   `init_mpv` deliberately overrides both to `yes`, and `spawn_args` keeps
///   those values, so the child keeps the keyboard setup of today's
///   embedded session.
/// - `input-media-keys=no` and `media-controls=no`: macOS-only options. A
///   Linux `mpv` rejects them as unknown options, so `spawn_args` does not
///   pass them.
///
/// The remaining list below mirrors `init_mpv`'s `opt!` set in the same
/// order. The cache, `hwdec`, `vo`, audio-output, and `pause` settings stay
/// `set_property` calls after the start (design D4), so the order of effects
/// stays the same.
pub(crate) fn spawn_args(config: &MpvRunConfig, paths: &SpawnPaths<'_>) -> Vec<String> {
    let mut args = vec![
        "--config=yes".to_owned(),
        format!("--config-dir={}", paths.config_dir.to_str().unwrap_or("")),
        format!("--input-ipc-server={}", paths.ipc_path),
        "--input-default-bindings=yes".to_owned(),
        "--input-vo-keyboard=yes".to_owned(),
        "--wayland-app-id=mbv".to_owned(),
        "--gapless-audio=weak".to_owned(),
    ];
    if config.no_scripts || !config.use_mpv_config {
        args.push("--load-scripts=no".to_owned());
        args.push("--osc=no".to_owned());
        args.push("--osd-bar=no".to_owned());
    }
    if !config.no_scripts
        && !config.use_mpv_config
        && let Some((script, fonts)) = paths.overlay
    {
        args.push(format!("--scripts={}", script.to_str().unwrap_or("")));
        args.push(format!("--osd-fonts-dir={}", fonts.to_str().unwrap_or("")));
    }
    // Restores the embedded-client defaults that the `mpv` program lacks
    // (see the list above).
    args.push("--idle=yes".to_owned());
    args.push("--no-terminal".to_owned());
    args.push("--input-terminal=no".to_owned());
    args
}

/// Connects to the `mpv` IPC socket, retrying up to `attempts` times with
/// `interval` between tries. mpv creates the socket some time after it starts,
/// so the first tries can find no path yet (design D4). Tests pass a zero
/// interval.
pub(crate) fn connect_with_retry(
    socket_path: &Path,
    attempts: u32,
    interval: Duration,
) -> Result<UnixStream, MpvError> {
    let attempts = attempts.max(1);
    let mut attempt = 0;
    loop {
        match connect_once(socket_path) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                attempt += 1;
                if attempt >= attempts {
                    return Err(error);
                }
                thread::sleep(interval);
            }
        }
    }
}

/// One connect attempt. A path that does not exist yet is `NotFound`, so the
/// retry loop above can tell it from a real socket fault.
fn connect_once(socket_path: &Path) -> Result<UnixStream, MpvError> {
    UnixStream::connect(socket_path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            MpvError::NotFound(format!(
                "the mpv IPC socket {} was not found",
                socket_path.display()
            ))
        } else {
            MpvError::Io(error)
        }
    })
}

/// A started `mpv` child process and its connected IPC socket (design D4 and
/// D5). One playback run owns one `Child`.
pub(crate) struct Child {
    process: ProcessChild,
    stream: UnixStream,
}

impl Child {
    /// Starts `program` with `args` and connects to the IPC socket it serves.
    /// Standard input, output, and error go to the null device: the child
    /// reports on its socket and mbv captures no terminal (design D4).
    pub(crate) fn start(
        program: &str,
        args: &[String],
        socket_path: &Path,
        attempts: u32,
        interval: Duration,
    ) -> Result<Self, MpvError> {
        let mut process = start_process(program, args)?;
        match connect_with_retry(socket_path, attempts, interval) {
            Ok(stream) => Ok(Self { process, stream }),
            // `std::process::Child` has no `Drop`, so a failed connect must
            // reap the process itself. Otherwise mpv stays alive (it runs with
            // `--idle=yes`) or becomes a zombie (design D5).
            Err(error) => {
                reap(&mut process);
                Err(error)
            }
        }
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        // A best-effort quit lets mpv release the audio device and its socket
        // before the kill below. The run loop already waited its own shutdown
        // grace period, so this Drop does not wait (design D5).
        let _ = writeln!(self.stream, r#"{{"command":["quit"]}}"#);
        reap(&mut self.process);
    }
}

/// Kills and reaps the child. Killing an exited child is harmless, and the
/// wait reaps it so no zombie stays.
fn reap(process: &mut ProcessChild) {
    let _ = process.kill();
    let _ = process.wait();
}

/// Starts one `mpv` child process with the null device for every standard
/// stream and a `SIGKILL` on the parent thread's death.
fn start_process(program: &str, args: &[String]) -> Result<ProcessChild, MpvError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: `pre_exec` runs in the forked child before `exec`. The closure
    // calls only `prctl`, which is async-signal-safe and touches no shared
    // state. `PR_SET_PDEATHSIG` makes the kernel send `SIGKILL` to this child
    // when the thread that started it ends, so an `mpv` child never outlives
    // the long-lived spawner thread that owns it (design D5).
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        })
    };
    command
        .spawn()
        .map_err(|error| spawn_error(program, &error))
}

/// Maps a spawn fault. A program that is not on the search path names `mpv`
/// for the start error (the spec says the error names the missing program);
/// every other fault is a general spawn error.
fn spawn_error(program: &str, error: &std::io::Error) -> MpvError {
    if error.kind() == ErrorKind::NotFound {
        MpvError::NotFound(format!(
            "the {program} program was not found on the search path"
        ))
    } else {
        MpvError::Spawn(format!("cannot start the {program} program: {error}"))
    }
}

#[cfg(test)]
mod tests;
