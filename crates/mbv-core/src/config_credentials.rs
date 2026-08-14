// Per-Service secret files and the Local-daemon Control credential.
// Included via `include!("config_credentials.rs")` in config.rs, so all
// items share the same module scope as the other config_*.rs files.

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
    std::fs::rename(&tmp, path)
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
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("control_credential.json");
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
                format!(
                    "load concurrently created Control credential {}",
                    path.display()
                )
            })
        }
        Err(error) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!(
                "publish {} as {}: {error}",
                tmp.display(),
                path.display()
            ))
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
