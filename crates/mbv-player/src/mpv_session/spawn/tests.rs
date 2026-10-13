//! The `spawn_args` contract (design D4): one pinned list per script-policy
//! combination that `init_mpv` handles, with the embedded-client defaults
//! from mpv 0.41's `[libmpv]` profile restored.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rstest::rstest;

use super::{Child, SpawnPaths, connect_with_retry, spawn_args};
use crate::mpv_error::MpvError;
use crate::runtime::MpvRunConfig;

const IPC_PATH: &str = "/run/mbv/mbv-mpv.sock";
const CONFIG_DIR: &str = "/run/mbv/mpv-config";
const SCRIPT: &str = "/opt/mbv/osc.lua";
const FONTS: &str = "/opt/mbv/fonts";

fn paths() -> SpawnPaths<'static> {
    SpawnPaths {
        ipc_path: IPC_PATH,
        config_dir: Path::new(CONFIG_DIR),
        overlay: Some((Path::new(SCRIPT), Path::new(FONTS))),
    }
}

fn config(use_mpv_config: bool, no_scripts: bool) -> MpvRunConfig {
    MpvRunConfig {
        headless: false,
        use_mpv_config,
        video_cache_forward_mb: 0,
        video_cache_back_mb: 0,
        no_scripts,
        always_skip_intro: false,
        audio_pipe_path: None,
        audio_pipe_samplerate: 0,
        audio_pipe_bitdepth: 0,
        audio_device: None,
    }
}

#[rstest]
#[case::scripts_disabled_by_config(
    true,
    true,
    &[
        "--config=yes",
        "--config-dir=/run/mbv/mpv-config",
        "--input-ipc-server=/run/mbv/mbv-mpv.sock",
        "--input-default-bindings=yes",
        "--input-vo-keyboard=yes",
        "--wayland-app-id=mbv",
        "--gapless-audio=weak",
        "--load-scripts=no",
        "--osc=no",
        "--osd-bar=no",
        "--idle=yes",
        "--no-terminal",
        "--input-terminal=no",
    ]
)]
#[case::mbv_manages_scripts(
    false,
    false,
    &[
        "--config=yes",
        "--config-dir=/run/mbv/mpv-config",
        "--input-ipc-server=/run/mbv/mbv-mpv.sock",
        "--input-default-bindings=yes",
        "--input-vo-keyboard=yes",
        "--wayland-app-id=mbv",
        "--gapless-audio=weak",
        "--load-scripts=no",
        "--osc=no",
        "--osd-bar=no",
        "--scripts=/opt/mbv/osc.lua",
        "--osd-fonts-dir=/opt/mbv/fonts",
        "--idle=yes",
        "--no-terminal",
        "--input-terminal=no",
    ]
)]
#[case::user_config_manages_scripts(
    true,
    false,
    &[
        "--config=yes",
        "--config-dir=/run/mbv/mpv-config",
        "--input-ipc-server=/run/mbv/mbv-mpv.sock",
        "--input-default-bindings=yes",
        "--input-vo-keyboard=yes",
        "--wayland-app-id=mbv",
        "--gapless-audio=weak",
        "--idle=yes",
        "--no-terminal",
        "--input-terminal=no",
    ]
)]
fn pins_argument_list(
    #[case] use_mpv_config: bool,
    #[case] no_scripts: bool,
    #[case] expected: &[&str],
) {
    let args = spawn_args(&config(use_mpv_config, no_scripts), &paths());
    let expected: Vec<String> = expected.iter().map(|arg| (*arg).to_owned()).collect();
    assert_eq!(args, expected);
}

/// A path that no process serves, so a connect finds no socket there.
fn missing_socket_path() -> PathBuf {
    std::env::temp_dir().join(format!("mbv-mpv-missing-{}.sock", std::process::id()))
}

#[test]
fn connect_to_a_missing_path_is_not_found() {
    let result = connect_with_retry(&missing_socket_path(), 1, Duration::ZERO);

    assert!(matches!(result, Err(MpvError::NotFound(_))));
}

#[test]
fn start_with_a_missing_program_is_not_found() {
    let result = Child::start(
        "mbv-test-no-such-mpv-program",
        &[],
        &missing_socket_path(),
        1,
        Duration::ZERO,
    );

    assert!(matches!(result, Err(MpvError::NotFound(_))));
}

#[test]
fn start_that_cannot_exec_is_a_spawn_error() {
    // A directory is never executable, so `spawn` fails for a reason other
    // than `NotFound` and the fault is a spawn error, not a missing program.
    let result = Child::start("/", &[], &missing_socket_path(), 1, Duration::ZERO);

    assert!(matches!(result, Err(MpvError::Spawn(_))));
}
