// Path helper functions extracted from config_types_paths.rs.
// Included via `include!("config_paths.rs")` in config.rs, so all
// items share the same module scope. Types and config_dir/cache_dir/
// state_dir/is_system_instance come from config_types_paths.rs.

pub fn data_dir_system_or_local() -> PathBuf {
    if is_system_instance() {
        return PathBuf::from("/var/lib/mbv");
    }
    let base = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".local").join("share")
        });
    base.join("mbv")
}

pub fn queue_state_path() -> PathBuf {
    state_dir().join("queue_state.json")
}

pub fn library_position_state_path() -> PathBuf {
    state_dir().join("library_position_state.json")
}

/// Visibility/size of the now-playing panel, cycled with `h` and remembered across restarts.
fn migrate_to_state(filename: &str) -> PathBuf {
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

pub fn osc_script_path() -> PathBuf {
    let user = data_dir_system_or_local().join("scripts").join("mbv.lua");
    if user.exists() {
        return user;
    }
    let dev = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/mbv.lua"));
    if dev.exists() {
        return dev;
    }
    PathBuf::from("/usr/share/mbv/scripts/mbv.lua")
}

pub fn prefs_path() -> PathBuf {
    migrate_to_state("prefs.json")
}

pub fn osc_fonts_dir() -> PathBuf {
    let user = data_dir_system_or_local().join("fonts");
    if user.exists() {
        return user;
    }
    PathBuf::from("/usr/share/mbv/fonts")
}

fn runtime_dir() -> String {
    if is_system_instance() {
        return "/run/mbv".to_string();
    }
    env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string())
}

pub fn mpv_ipc_path() -> String {
    format!("{}/mbv-mpv.sock", runtime_dir())
}

pub fn mpv_config_dir() -> PathBuf {
    PathBuf::from(runtime_dir()).join("mpv-config")
}

pub fn control_socket_path() -> String {
    format!("{}/mbv-ctrl.sock", runtime_dir())
}

pub fn token_cache_path() -> PathBuf {
    migrate_to_state("token.json")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}
