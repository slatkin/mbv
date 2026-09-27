// Path helper functions extracted from config_types_paths.rs.
// Types and config_dir/cache_dir/state_dir/is_system_instance come from config_types_paths.rs.

use super::{cache_dir, config_dir, is_system_instance, state_dir};
use std::env;
use std::path::PathBuf;

pub fn data_dir_system_or_local() -> PathBuf {
    if is_system_instance() {
        return PathBuf::from("/var/lib/mbv");
    }
    let base = env::var("XDG_DATA_HOME").map_or_else(
        |_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".local").join("share")
        },
        PathBuf::from,
    );
    base.join("mbv")
}

#[must_use]
pub fn queue_state_path() -> PathBuf {
    state_dir().join("queue_state.json")
}

#[must_use]
pub fn stay_alive_queue_state_path() -> PathBuf {
    state_dir().join("stay_alive_queue_state.json")
}

#[must_use]
pub fn library_position_state_path() -> PathBuf {
    state_dir().join("library_position_state.json")
}

#[must_use]
pub fn home_latest_launch_path() -> PathBuf {
    state_dir().join("home_latest_launch.json")
}

/// Visibility/size of the now-playing panel, cycled with `h` and remembered across restarts.
pub(super) fn migrate_to_state(filename: &str) -> PathBuf {
    let dest = state_dir().join(filename);
    if dest.exists() {
        return dest;
    }
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let cache = cache_dir().join(filename);
    if cache.exists() {
        let _ = std::fs::rename(&cache, &dest);
        return dest;
    }
    let old = config_dir().join(filename);
    if old.exists() {
        let _ = std::fs::rename(&old, &dest);
    }
    dest
}

/// The outcome of resolving the mpv overlay script set (or its fonts):
/// the source handed to mpv, plus any ignored copy at the removed
/// installer's user-directory path (named in a startup warning, never used).
#[derive(Debug)]
pub struct ScriptSource {
    pub chosen: PathBuf,
    pub unused_legacy: Option<PathBuf>,
}

/// Pure resolution over injected candidates (B3): the checkout entry wins
/// when it exists, else the packaged path. `legacy` is never a candidate;
/// it is only reported when it exists so startup can warn that a
/// removed-installer copy is being ignored. No environment is read.
#[must_use]
pub fn resolve_script_source(checkout: PathBuf, package: PathBuf, legacy: PathBuf) -> ScriptSource {
    let chosen = if checkout.exists() { checkout } else { package };
    let unused_legacy = legacy.exists().then_some(legacy);
    ScriptSource {
        chosen,
        unused_legacy,
    }
}

/// Compile-time checkout root, derived from the manifest directory
/// (`<checkout>/crates/mbv-core`). Absent on installed systems, so the
/// packaged copy wins there.
pub(super) fn checkout_scripts_entry() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/mbv.lua"
    ))
}

pub(super) fn checkout_fonts_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fonts"))
}

#[must_use]
pub fn osc_script_source() -> ScriptSource {
    resolve_script_source(
        checkout_scripts_entry(),
        PathBuf::from("/usr/share/mbv/scripts/mbv.lua"),
        data_dir_system_or_local().join("scripts").join("mbv.lua"),
    )
}

#[must_use]
pub fn osc_fonts_source() -> ScriptSource {
    resolve_script_source(
        checkout_fonts_dir(),
        PathBuf::from("/usr/share/mbv/fonts"),
        data_dir_system_or_local().join("fonts"),
    )
}

#[must_use]
pub fn prefs_path() -> PathBuf {
    migrate_to_state("prefs.json")
}

#[must_use]
pub fn osc_fonts_dir() -> PathBuf {
    osc_fonts_source().chosen
}

pub(super) fn runtime_dir() -> String {
    if is_system_instance() {
        return "/run/mbv".to_string();
    }
    env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string())
}

#[must_use]
pub fn mpv_ipc_path() -> String {
    format!("{}/mbv-mpv.sock", runtime_dir())
}

#[must_use]
pub fn mpv_config_dir() -> PathBuf {
    PathBuf::from(runtime_dir()).join("mpv-config")
}

#[must_use]
pub fn control_socket_path() -> String {
    format!("{}/mbv-ctrl.sock", runtime_dir())
}

#[must_use]
pub fn token_cache_path() -> PathBuf {
    migrate_to_state("token.json")
}

#[must_use]
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}
