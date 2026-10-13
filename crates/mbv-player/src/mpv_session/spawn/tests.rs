//! The `spawn_args` contract (design D4): one pinned list per script-policy
//! combination that `init_mpv` handles, with the embedded-client defaults
//! from mpv 0.41's `[libmpv]` profile restored.

use std::path::Path;

use rstest::rstest;

use super::{SpawnPaths, spawn_args};
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
