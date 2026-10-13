//! Child-process spawn arguments (design D4 of `mpv-child-process`).
//!
//! `spawn_args` turns the option list that `init_mpv` sets through libmpv
//! into `mpv` command-line arguments with the same values. The child-process
//! backend (group 4) starts the child with this list; libmpv stays the live
//! backend until then.

use std::path::Path;

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

#[cfg(test)]
mod tests;
