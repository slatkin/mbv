// Path helper functions extracted from config_types_paths.rs.
// This file is included via `include!("config_paths.rs")` in config.rs,
// so all items share the same module scope. Items from config_types_paths.rs
// (types, config_dir, cache_dir, state_dir, is_system_instance) are already
// in scope.

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

pub fn save_queue_state(state: &QueueState) -> Result<(), String> {
    let path = queue_state_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("create directory {}: {e}", dir.display()))?;
    }
    let json = serde_json::to_string(state).map_err(|e| format!("serialize queue state: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
}

pub fn load_queue_state() -> Option<QueueState> {
    let text = std::fs::read_to_string(queue_state_path()).ok()?;
    match serde_json::from_str(&text) {
        Ok(state) => Some(state),
        Err(e) => {
            log::warn!(target: "queue", "queue_state.json failed to parse, queue not restored: {e}");
            None
        }
    }
}

pub fn clear_queue_state() -> Result<(), String> {
    let path = queue_state_path();
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("remove {}: {error}", path.display())),
    }
}

/// Which remote connection (if any) was active when mbv last exited
/// (issue #236). `App::teardown` writes this; `App::new` reads it back at
/// the next launch when `Config.auto_reconnect` is true. The two
/// variants mirror `App`'s own separate `active_route` (#223 library
/// routing) and `connected_session_id`/`connected_session_state`
/// (Sessions-panel direct-remote/attached) fields -- #222 and #223 were
/// distinct features and stay distinct here, even though both are
/// restored under the same on/off switch.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum LastRemoteConnection {
    /// A #223 library route, keyed by the library name that was resolved
    /// active (`App.active_route`). Re-resolved fresh against current
    /// `library_routes` at startup, not replayed verbatim -- if the config
    /// changed since the last exit, the new config wins.
    LibraryRoute { library: String },
    /// A Sessions-panel direct-remote or attached session, keyed by the
    /// other device's name (`SessionInfo.device_name`), not its session id
    /// -- Emby session ids are ephemeral per-connection and would not
    /// still identify the same device at the next launch.
    DirectSession { device_name: String },
}

fn last_remote_connection_path() -> PathBuf {
    state_dir().join("last_remote_connection.json")
}

/// Persists (or, given `None`, clears) the connection active at exit.
/// Called from `App::teardown` only when `auto_reconnect` is
/// enabled -- when the feature is off, this file is never written or
/// read, by design (Task 1's `Global Constraints`).
fn save_last_remote_connection_at(
    path: &std::path::Path,
    conn: Option<&LastRemoteConnection>,
) -> Result<(), String> {
    let Some(conn) = conn else {
        return match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("remove {}: {e}", path.display())),
        };
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("create directory {}: {e}", dir.display()))?;
    }
    let json =
        serde_json::to_string(conn).map_err(|e| format!("serialize {}: {e}", path.display()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
}

pub fn save_last_remote_connection(conn: Option<&LastRemoteConnection>) -> Result<(), String> {
    save_last_remote_connection_at(&last_remote_connection_path(), conn)
}

fn load_last_remote_connection_at(
    path: &std::path::Path,
) -> Result<Option<LastRemoteConnection>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    match serde_json::from_str(&text) {
        Ok(conn) => Ok(Some(conn)),
        Err(e) => {
            std::fs::remove_file(path).map_err(|remove_error| {
                format!(
                    "parse {}: {e}; remove corrupt {}: {remove_error}",
                    path.display(),
                    path.display()
                )
            })?;
            Err(format!(
                "parse {}: {e}; corrupt file removed",
                path.display()
            ))
        }
    }
}

pub fn load_last_remote_connection() -> Result<Option<LastRemoteConnection>, String> {
    load_last_remote_connection_at(&last_remote_connection_path())
}

pub fn save_library_position_state(state: &LibraryPositionState) {
    let _ = save_library_position_state_result(state);
}

pub fn save_library_position_state_result(state: &LibraryPositionState) -> Result<(), String> {
    let path = library_position_state_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("create directory {}: {error}", dir.display()))?;
    }
    let json =
        serde_json::to_string(state).map_err(|error| format!("serialize positions: {error}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|error| format!("write {}: {error}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .map_err(|error| format!("rename {} to {}: {error}", tmp.display(), path.display()))
}

pub fn load_library_position_state() -> LibraryPositionState {
    let text = match std::fs::read_to_string(library_position_state_path()) {
        Ok(text) => text,
        Err(_) => return LibraryPositionState::default(),
    };
    match serde_json::from_str(&text) {
        Ok(state) => state,
        Err(e) => {
            log::warn!(target: "library_position", "library_position_state.json failed to parse: {e}");
            LibraryPositionState::default()
        }
    }
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

/// ── Per-Service secrets ──────────────────────────────────────────────
/// Each configured Remote Service gets its own mode-0600 secret file
/// holding the API token/credential, separate from config.toml.
///
/// Path to the secret file for a given Service kind.
/// Files live under `state_dir()/secrets/` for isolation.
pub fn service_secret_path(kind: ServiceKind) -> PathBuf {
    state_dir()
        .join("secrets")
        .join(format!("{}.json", kind.secret_name()))
}

/// Atomically write a Service secret, restricting to owner-only
/// permissions on Unix. Uses the same tmp+rename pattern as
/// `save_queue_state` and `save_last_remote_connection_at`.
pub fn save_service_secret(kind: ServiceKind, secret: &str) -> Result<(), String> {
    save_service_secret_at(secret, &service_secret_path(kind))
}

fn save_service_secret_at(secret: &str, path: &std::path::Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("create secrets directory {}: {e}", dir.display()))?;
    }
    let json = serde_json::json!({"token": secret});
    let text = json.to_string();
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    // Restrict to owner-only before renaming into place
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod 0600 {}: {e}", tmp.display()))?;
    }
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
}

/// Load a Service secret from its dedicated file. Returns `None` when
/// the file does not exist or cannot be parsed.
pub fn load_service_secret(kind: ServiceKind) -> Option<String> {
    let path = service_secret_path(kind);
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let token = v["token"].as_str()?.to_string();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

/// Remove a Service's secret file. No error if absent.
pub fn clear_service_secret(kind: ServiceKind) {
    let _ = clear_service_secret_result(kind);
}

pub fn clear_service_secret_result(kind: ServiceKind) -> Result<(), String> {
    let path = service_secret_path(kind);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("remove {}: {e}", path.display())),
    }
    // Also clean up any orphaned tmp file
    match std::fs::remove_file(path.with_extension("json.tmp")) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("remove Emby secret temporary file: {e}")),
    }
    Ok(())
}

/// ── Control credential ───────────────────────────────────────────────
/// Per-Local-daemon secret authorising ctrl clients. Independent of all
/// Service credentials, so the daemon can be reached before any Remote
/// Service is configured.
///
/// Path to the Local daemon Control credential file. This lives under
/// `state_dir()` so it is per-user and survives daemon restarts.
pub fn control_credential_path() -> PathBuf {
    state_dir().join("control_credential.json")
}

fn write_control_credential_temp(secret: &str, path: &std::path::Path) -> Result<PathBuf, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("create directory {}: {e}", dir.display()))?;
    }
    let text = serde_json::json!({"credential": secret}).to_string();
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("control_credential.json");
    let tmp = path.with_file_name(format!("{name}.{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod 0600 {}: {e}", tmp.display()))?;
    }
    Ok(tmp)
}

/// Atomically write the Control credential (mode 0600) using a unique temp file.
pub fn save_control_credential(secret: &str) -> Result<(), String> {
    let path = control_credential_path();
    let tmp = write_control_credential_temp(secret, &path)?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
}

/// Load the stable Local-daemon Control credential, creating it on first use.
/// This credential has no relationship to any Remote Service secret.
pub fn load_or_create_control_credential() -> Result<String, String> {
    if let Some(credential) = load_control_credential() {
        return Ok(credential);
    }

    let path = control_credential_path();
    let credential = uuid::Uuid::new_v4().to_string();
    let tmp = write_control_credential_temp(&credential, &path)?;
    match std::fs::hard_link(&tmp, &path) {
        Ok(()) => {
            let _ = std::fs::remove_file(&tmp);
            Ok(credential)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(&tmp);
            load_control_credential().ok_or_else(|| {
                format!("load concurrently created Control credential {}", path.display())
            })
        }
        Err(error) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("publish {} as {}: {error}", tmp.display(), path.display()))
        }
    }
}

/// Load the Control credential. Returns `None` when the file does not
/// exist or cannot be parsed.
pub fn load_control_credential() -> Option<String> {
    let path = control_credential_path();
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let cred = v["credential"].as_str()?.to_string();
    if cred.is_empty() {
        return None;
    }
    Some(cred)
}

/// Remove the Control credential file.
pub fn clear_control_credential() {
    let _ = clear_control_credential_result();
}

pub fn clear_control_credential_result() -> Result<(), String> {
    let path = control_credential_path();
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("remove {}: {error}", path.display())),
    }
    match std::fs::remove_file(path.with_extension("json.tmp")) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("remove Control credential temporary file: {error}")),
    }
    Ok(())
}

/// ── Legacy Emby token migration ──────────────────────────────────────
/// One-time migration from the legacy flat `token.json` (server_url +
/// token + user_id) to the per-Service `secrets/emby.json` + future
/// `[server]` in config.toml. Write-new-before-remove-old ordering:
/// the legacy file is only deleted after the new secret is durably
/// written.
///
/// Migrate legacy setup metadata and token without prompting. Existing new
/// data is authoritative: stale legacy data is never used to overwrite it.
pub fn migrate_legacy_emby_token() -> Result<(), String> {
    let legacy_path = token_cache_path();
    let text = match std::fs::read_to_string(&legacy_path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("read legacy token {}: {e}", legacy_path.display())),
    };

    let v: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return Err(format!("parse legacy token {}: {e}", legacy_path.display())),
    };

    let token = match v["token"].as_str() {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return Err(format!(
                "legacy token {} has empty or missing 'token' field",
                legacy_path.display()
            ))
        }
    };

    let server_url = v["server_url"].as_str().unwrap_or_default().trim();
    let user_id = v["user_id"].as_str().unwrap_or_default().trim();
    if server_url.is_empty() || user_id.is_empty() {
        return Err(format!(
            "legacy token {} is missing server_url or user_id; legacy data retained",
            legacy_path.display()
        ));
    }

    let setup_path = config_path();
    let setup = load_config()?.emby_setup;
    let secret_path = service_secret_path(ServiceKind::Emby);
    let secret_file_exists = secret_path.exists();
    let secret = load_service_secret(ServiceKind::Emby);
    if secret_file_exists && secret.is_none() {
        return Err(format!(
            "cannot migrate {}: existing Emby secret {} is unreadable; setup/secret partial state retained",
            legacy_path.display(),
            secret_path.display()
        ));
    }

    let legacy_setup = EmbySetup::new(server_url, user_id);
    match (setup, secret) {
        (Some(_), Some(_)) => {
            // Already migrated. Neither authoritative record is touched.
        }
        (Some(existing), None) => {
            if existing != legacy_setup {
                return Err(format!(
                    "cannot migrate {}: legacy setup ({}, {}) conflicts with existing setup in {}; secret not written and legacy data retained",
                    legacy_path.display(),
                    legacy_setup.server_url,
                    legacy_setup.user_id,
                    setup_path.display()
                ));
            }
            // Resume a setup-first migration only when its identity matches.
            save_service_secret(ServiceKind::Emby, &token)
                .map_err(|e| format!("migrate token from {}: {e}", legacy_path.display()))?;
        }
        (None, Some(_)) => {
            return Err(format!(
                "cannot migrate {}: existing Emby secret {} has no persisted setup; identity conflict, setup not written and legacy data retained",
                legacy_path.display(),
                secret_path.display()
            ));
        }
        (None, None) => {
            // Write setup first, then secret; legacy removal happens below.
            save_emby_setup(&legacy_setup)
                .map_err(|e| format!("migrate setup into {}: {e}", setup_path.display()))?;
            save_service_secret(ServiceKind::Emby, &token)
                .map_err(|e| format!("migrate token from {}: {e}", legacy_path.display()))?;
        }
    }

    // Only now remove the legacy file. If this fails the new secret
    // is already safe on disk; inform the caller but do not roll back.
    if let Err(e) = std::fs::remove_file(&legacy_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(format!(
                "token migrated to {}, but could not remove legacy {}: {e}",
                service_secret_path(ServiceKind::Emby).display(),
                legacy_path.display()
            ));
        }
    }

    Ok(())
}

/// Persist the singleton Emby setup in the existing `[server]` table while
/// preserving unrelated TOML sections and keys.
pub fn save_emby_setup(setup: &EmbySetup) -> Result<(), String> {
    save_emby_setup_at(setup, &config_path())
}

fn clear_emby_setup_at(path: &std::path::Path) -> Result<(), String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("read {}: {error}", path.display())),
    };
    let mut doc: toml::Value =
        toml::from_str(&text).map_err(|error| format!("parse {}: {error}", path.display()))?;
    let table = doc
        .as_table_mut()
        .ok_or_else(|| format!("update {}: root is not a table", path.display()))?;
    table.remove("server");
    let text =
        toml::to_string(&doc).map_err(|error| format!("serialize {}: {error}", path.display()))?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|error| format!("write {}: {error}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|error| format!("rename {} to {}: {error}", tmp.display(), path.display()))
}

fn save_emby_setup_at(setup: &EmbySetup, path: &std::path::Path) -> Result<(), String> {
    if setup.server_url.trim().is_empty() || setup.user_id.trim().is_empty() {
        return Err("Emby setup requires a server URL and user ID".to_string());
    }
    let mut doc: toml::Value = match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let table = doc
        .as_table_mut()
        .ok_or_else(|| format!("update {}: root is not a table", path.display()))?;
    let server = table
        .entry("server".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| format!("update {}: server is not a table", path.display()))?;
    server.insert(
        "url".to_string(),
        toml::Value::String(setup.server_url.clone()),
    );
    server.insert(
        "user_id".to_string(),
        toml::Value::String(setup.user_id.clone()),
    );
    server.insert(
        "revision".to_string(),
        toml::Value::Integer(setup.revision as i64),
    );
    for key in ["username", "password", "api_key"] {
        server.remove(key);
    }
    let text = toml::to_string(&doc).map_err(|e| format!("serialize {}: {e}", path.display()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create directory {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
}

/// Commit the two persisted halves of an Emby setup as one practical
/// transaction. If the secret write fails, restore both files to their exact
/// prior bytes (or absence), so a validated replacement cannot leave a mixed
/// setup behind.
pub fn persist_emby_setup_and_secret(setup: &EmbySetup, token: &str) -> Result<(), String> {
    persist_emby_setup_and_secret_at(
        setup,
        token,
        &config_path(),
        &service_secret_path(ServiceKind::Emby),
        |setup, path| save_emby_setup_at(setup, path),
        |token, path| save_service_secret_at(token, path),
    )
}

/// Remove the Emby setup and only the Emby Service secret as one practical
/// transaction.  The caller can therefore perform the destructive in-memory
/// cleanup only after this boundary succeeds.
pub fn remove_emby_setup_and_secret() -> Result<(), String> {
    let config = config_path();
    let secret = service_secret_path(ServiceKind::Emby);
    let snapshot = |path: &std::path::Path| match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read {} for rollback: {error}", path.display())),
    };
    let old_config = snapshot(&config)?;
    let old_secret = snapshot(&secret)?;
    let restore = |path: &std::path::Path, bytes: &Option<Vec<u8>>| -> Result<(), String> {
        match bytes {
            Some(bytes) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                let tmp = path.with_extension("rollback.tmp");
                std::fs::write(&tmp, bytes).map_err(|error| error.to_string())?;
                #[cfg(unix)]
                if path == secret {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
                        .map_err(|error| error.to_string())?;
                }
                std::fs::rename(tmp, path).map_err(|error| error.to_string())
            }
            None => match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.to_string()),
            },
        }
    };
    let rollback = |reason: String| {
        let config_result = restore(&config, &old_config);
        let secret_result = restore(&secret, &old_secret);
        match (config_result, secret_result) {
            (Ok(()), Ok(())) => Err(reason),
            (config_result, secret_result) => Err(format!(
                "{reason}; rollback failed (config={config_result:?}, secret={secret_result:?})"
            )),
        }
    };
    if let Err(error) = clear_emby_setup_at(&config) {
        return rollback(format!("remove Emby setup: {error}"));
    }
    if let Err(error) = clear_service_secret_result(ServiceKind::Emby) {
        return rollback(format!("remove Emby secret: {error}"));
    }
    Ok(())
}

fn persist_emby_setup_and_secret_at<FS, FT>(
    setup: &EmbySetup,
    token: &str,
    config: &std::path::Path,
    secret: &std::path::Path,
    save_setup: FS,
    save_secret: FT,
) -> Result<(), String>
where
    FS: Fn(&EmbySetup, &std::path::Path) -> Result<(), String>,
    FT: Fn(&str, &std::path::Path) -> Result<(), String>,
{
    let snapshot = |path: &std::path::Path| match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read {} for rollback: {error}", path.display())),
    };
    let old_config = snapshot(config)?;
    let old_secret = snapshot(secret)?;

    let restore = |path: &std::path::Path, bytes: &Option<Vec<u8>>| -> Result<(), String> {
        match bytes {
            Some(bytes) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let tmp = path.with_extension("rollback.tmp");
                std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
                #[cfg(unix)]
                if path == secret {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
                        .map_err(|e| e.to_string())?;
                }
                std::fs::rename(tmp, path).map_err(|e| e.to_string())
            }
            None => {
                let _ = std::fs::remove_file(path);
                Ok(())
            }
        }
    };
    let rollback = |reason: String| {
        let config_restore = restore(config, &old_config);
        let secret_restore = restore(secret, &old_secret);
        let _ = std::fs::remove_file(secret.with_extension("json.tmp"));
        let _ = std::fs::remove_file(config.with_extension("toml.tmp"));
        let _ = std::fs::remove_file(config.with_extension("rollback.tmp"));
        let _ = std::fs::remove_file(secret.with_extension("rollback.tmp"));
        match (config_restore, secret_restore) {
            (Ok(()), Ok(())) => Err(reason),
            (config, secret) => Err(format!(
                "{reason}; rollback failed (config={config:?}, secret={secret:?})"
            )),
        }
    };

    if let Err(error) = save_setup(setup, config) {
        return rollback(format!("persist Emby setup: {error}"));
    }
    if let Err(error) = save_secret(token, secret) {
        return rollback(format!("persist Emby secret: {error}"));
    }
    Ok(())
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

impl QueueState {
    fn without_items<F>(&self, keep: F) -> Self
    where
        F: Fn(&crate::playback_queue::QueueItem) -> bool,
    {
        let items: Vec<crate::playback_queue::QueueItem> = self
            .items
            .iter()
            .filter(|item| keep(item))
            .cloned()
            .collect();
        let positions = self
            .positions
            .iter()
            .filter(|(id, _)| items.iter().any(|item| item.id() == **id))
            .map(|(id, position)| (id.clone(), *position))
            .collect();
        Self {
            source: if items.is_empty() {
                QueueSource::Unknown
            } else {
                self.source.clone()
            },
            cursor: self.cursor.min(items.len().saturating_sub(1)),
            last_played_content_id: self
                .last_played_content_id
                .as_ref()
                .filter(|id| items.iter().any(|item| item.content_id() == **id))
                .cloned(),
            last_played_item_id: self
                .last_played_item_id
                .as_ref()
                .filter(|id| items.iter().any(|item| item.id() == **id))
                .cloned(),
            last_played_completed: self.last_played_completed && !items.is_empty(),
            items,
            positions,
        }
    }

    /// Remove only Emby slots and native-ID keyed positions. Feed and
    /// Audiobookshelf snapshots remain intact for mixed queue restoration.
    /// After this change Emby removal preserves non-Emby items (Feed +
    /// Audiobookshelf) as required by the Audiobookshelf lifecycle.
    pub fn without_emby(&self) -> Self {
        self.without_items(|item| !matches!(item, crate::playback_queue::QueueItem::Emby(_)))
    }

    /// Remove only Audiobookshelf slots and their keyed positions. Emby and
    /// Feed items remain intact. Used on confirmed Audiobookshelf Service
    /// replacement/removal to purge Service-owned queue state without
    /// affecting other Services.
    pub fn without_audiobookshelf(&self) -> Self {
        self.without_items(|item| {
            !matches!(item, crate::playback_queue::QueueItem::Audiobookshelf(_))
        })
    }

    /// Remove both Emby and Audiobookshelf items, keeping only Feed entries.
    /// Composition of `without_emby` and `without_audiobookshelf` would also
    /// work, but this single-pass form avoids double allocation.
    pub fn without_emby_and_audiobookshelf(&self) -> Self {
        self.without_items(|item| matches!(item, crate::playback_queue::QueueItem::Feed(_)))
    }
}
