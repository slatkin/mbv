// Emby setup persistence: legacy token migration and transactional
// setup+secret writes.

use super::*;

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
    let Some((legacy_path, token, legacy_setup)) = read_legacy_emby_setup_and_token()? else {
        return Ok(());
    };
    migrate_legacy_emby_setup_and_token(&legacy_path, &token, &legacy_setup)?;
    remove_migrated_legacy_emby_token(&legacy_path)
}

// Read and validate the legacy record before inspecting or writing new state.
fn read_legacy_emby_setup_and_token(
) -> Result<Option<(std::path::PathBuf, String, EmbySetup)>, String> {
    let legacy_path = token_cache_path();
    let text = match std::fs::read_to_string(&legacy_path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
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

    Ok(Some((
        legacy_path,
        token,
        EmbySetup::new(server_url, user_id),
    )))
}

fn migrate_legacy_emby_setup_and_token(
    legacy_path: &std::path::Path,
    token: &str,
    legacy_setup: &EmbySetup,
) -> Result<(), String> {
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

    match (setup, secret) {
        (Some(_), Some(_)) => {
            // Already migrated. Neither authoritative record is touched.
        }
        (Some(existing), None) => {
            if existing != *legacy_setup {
                return Err(format!(
                    "cannot migrate {}: legacy setup ({}, {}) conflicts with existing setup in {}; secret not written and legacy data retained",
                    legacy_path.display(),
                    legacy_setup.server_url,
                    legacy_setup.user_id,
                    setup_path.display()
                ));
            }
            // Resume a setup-first migration only when its identity matches.
            save_service_secret(ServiceKind::Emby, token)
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
            save_emby_setup(legacy_setup)
                .map_err(|e| format!("migrate setup into {}: {e}", setup_path.display()))?;
            save_service_secret(ServiceKind::Emby, token)
                .map_err(|e| format!("migrate token from {}: {e}", legacy_path.display()))?;
        }
    }
    Ok(())
}

// Delete the old record only after the new setup and secret are durable.
fn remove_migrated_legacy_emby_token(legacy_path: &std::path::Path) -> Result<(), String> {
    // If removal fails the new secret is already safe on disk; inform the caller but do not roll back.
    if let Err(e) = std::fs::remove_file(legacy_path) {
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

fn load_emby_setup_doc(path: &std::path::Path) -> Result<toml::Value, String> {
    Ok(match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    })
}

fn write_emby_setup_doc(doc: &toml::Value, path: &std::path::Path) -> Result<(), String> {
    let text = toml::to_string(doc).map_err(|e| format!("serialize {}: {e}", path.display()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create directory {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
}

pub(super) fn save_emby_setup_at(setup: &EmbySetup, path: &std::path::Path) -> Result<(), String> {
    if setup.server_url.trim().is_empty() || setup.user_id.trim().is_empty() {
        return Err("Emby setup requires a server URL and user ID".to_string());
    }
    let mut doc: toml::Value = load_emby_setup_doc(path)?;
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
    write_emby_setup_doc(&doc, path)
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
        save_emby_setup_at,
        save_service_secret_at,
    )
}

/// Remove the Emby setup and only the Emby Service secret as one practical
/// transaction.  The caller can therefore perform the destructive in-memory
/// cleanup only after this boundary succeeds.
fn snapshot_for_rollback(path: &std::path::Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read {} for rollback: {error}", path.display())),
    }
}

fn restore_removed_file(
    path: &std::path::Path,
    secret: &std::path::Path,
    bytes: Option<&[u8]>,
) -> Result<(), String> {
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
}

fn rollback_emby_removal(
    reason: String,
    config: &std::path::Path,
    secret: &std::path::Path,
    old_config: Option<&[u8]>,
    old_secret: Option<&[u8]>,
) -> Result<(), String> {
    let config_result = restore_removed_file(config, secret, old_config);
    let secret_result = restore_removed_file(secret, secret, old_secret);
    match (config_result, secret_result) {
        (Ok(()), Ok(())) => Err(reason),
        (config_result, secret_result) => Err(format!(
            "{reason}; rollback failed (config={config_result:?}, secret={secret_result:?})"
        )),
    }
}

pub fn remove_emby_setup_and_secret() -> Result<(), String> {
    let config = config_path();
    let secret = service_secret_path(ServiceKind::Emby);
    let old_config = snapshot_for_rollback(&config)?;
    let old_secret = snapshot_for_rollback(&secret)?;
    if let Err(error) = clear_emby_setup_at(&config) {
        return rollback_emby_removal(
            format!("remove Emby setup: {error}"),
            &config,
            &secret,
            old_config.as_deref(),
            old_secret.as_deref(),
        );
    }
    if let Err(error) = clear_service_secret_result(ServiceKind::Emby) {
        return rollback_emby_removal(
            format!("remove Emby secret: {error}"),
            &config,
            &secret,
            old_config.as_deref(),
            old_secret.as_deref(),
        );
    }
    Ok(())
}

pub(super) fn persist_emby_setup_and_secret_at<FS, FT>(
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
    let old_config = snapshot_for_rollback(config)?;
    let old_secret = snapshot_for_rollback(secret)?;

    if let Err(error) = save_setup(setup, config) {
        return rollback_emby_setup_and_secret(
            format!("persist Emby setup: {error}"),
            config,
            secret,
            old_config.as_deref(),
            old_secret.as_deref(),
        );
    }
    if let Err(error) = save_secret(token, secret) {
        return rollback_emby_setup_and_secret(
            format!("persist Emby secret: {error}"),
            config,
            secret,
            old_config.as_deref(),
            old_secret.as_deref(),
        );
    }
    Ok(())
}

fn restore_emby_setup_file(
    path: &std::path::Path,
    secret: &std::path::Path,
    bytes: Option<&[u8]>,
) -> Result<(), String> {
    if let Some(bytes) = bytes {
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
    } else {
        let _ = std::fs::remove_file(path);
        Ok(())
    }
}

fn rollback_emby_setup_and_secret(
    reason: String,
    config: &std::path::Path,
    secret: &std::path::Path,
    old_config: Option<&[u8]>,
    old_secret: Option<&[u8]>,
) -> Result<(), String> {
    let config_restore = restore_emby_setup_file(config, secret, old_config);
    let secret_restore = restore_emby_setup_file(secret, secret, old_secret);
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
}
