//! The `spawn_args` contract (design D4): one pinned list per script-policy
//! combination that `init_mpv` handles, with the embedded-client defaults
//! from mpv 0.41's `[libmpv]` profile restored.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rstest::rstest;

use super::{Child, SpawnPaths, connect_once, spawn_args};
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

/// The args that every run passes, independent of the script policy.
fn shared_args() -> Vec<String> {
    [
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
    .iter()
    .map(|arg| (*arg).to_owned())
    .collect()
}

/// The script-policy args one config adds, in place. One case per distinct
/// branch outcome; the shared list above is pinned once.
#[rstest]
#[case::scripts_disabled_by_config(
    true,
    true,
    &["--load-scripts=no", "--osc=no", "--osd-bar=no"]
)]
#[case::mbv_manages_scripts(
    false,
    false,
    &[
        "--load-scripts=no",
        "--osc=no",
        "--osd-bar=no",
        "--scripts=/opt/mbv/osc.lua",
        "--osd-fonts-dir=/opt/mbv/fonts",
    ]
)]
#[case::user_config_manages_scripts(true, false, &[])]
fn pins_script_policy_arguments(
    #[case] use_mpv_config: bool,
    #[case] no_scripts: bool,
    #[case] expected: &[&str],
) {
    let args = spawn_args(&config(use_mpv_config, no_scripts), &paths());
    let policy: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|arg| {
            arg.starts_with("--load-scripts")
                || arg.starts_with("--osc=")
                || arg.starts_with("--osd-bar")
                || arg.starts_with("--scripts=")
                || arg.starts_with("--osd-fonts-dir")
        })
        .collect();
    assert_eq!(policy, expected);
}

/// A config with no script policy uses exactly the shared argument list.
#[test]
fn pins_shared_argument_list() {
    let args = spawn_args(&config(true, false), &paths());

    assert_eq!(args, shared_args());
}

/// A path that no process serves, so a connect finds no socket there.
fn missing_socket_path() -> PathBuf {
    std::env::temp_dir().join(format!("mbv-mpv-missing-{}.sock", std::process::id()))
}

#[test]
fn connect_to_a_missing_path_is_socket_not_found() {
    let result = connect_once(&missing_socket_path());

    assert!(matches!(result, Err(MpvError::SocketNotFound(_))));
}

#[test]
fn start_with_a_missing_program_is_program_not_found() {
    let result = Child::start(
        "mbv-test-no-such-mpv-program",
        &[],
        &missing_socket_path(),
        1,
        Duration::ZERO,
    );

    assert!(matches!(result, Err(MpvError::ProgramNotFound(_))));
}

#[test]
fn start_that_cannot_exec_is_a_spawn_error() {
    // A directory is never executable, so `spawn` fails for a reason other
    // than `NotFound` and the fault is a spawn error, not a missing program.
    let result = Child::start("/", &[], &missing_socket_path(), 1, Duration::ZERO);

    assert!(matches!(result, Err(MpvError::Spawn(_))));
}

// Design D7: a child that exits during start-up is the fault, so the start
// error names its exit status and keeps the stderr tail instead of waiting
// out the connect retries and blaming the socket.
#[test]
fn start_that_exits_immediately_is_a_child_exited_error() {
    let result = Child::start(
        "/bin/sh",
        &[
            "-c".to_owned(),
            "echo start-up failed >&2; exit 3".to_owned(),
        ],
        &missing_socket_path(),
        100_000,
        Duration::ZERO,
    );

    match result {
        Err(MpvError::ChildExited(message)) => {
            assert!(message.contains("exit status: 3"), "{message}");
            assert!(message.contains("start-up failed"), "{message}");
        }
        Err(other) => panic!("expected a child-exited error, got {other:?}"),
        Ok(_) => panic!("expected a child-exited error"),
    }
}
