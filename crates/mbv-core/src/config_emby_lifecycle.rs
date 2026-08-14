/// A restorable snapshot of the files owned by Emby setup administration.
///
/// The bytes are retained rather than decoded and reconstructed so a failed
/// replacement restores unrelated configuration and unknown future fields
/// exactly as they were.
#[derive(Clone, Debug, Default)]
pub struct EmbyOwnedStateSnapshot {
    queue: Option<Vec<u8>>,
    library_positions: Option<Vec<u8>>,
    config: Option<Vec<u8>>,
    legacy_token: Option<Vec<u8>>,
    secret: Option<Vec<u8>>,
    image_cache: Vec<(String, Vec<u8>)>,
}

fn snapshot_file(path: &std::path::Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read owner state: {error}")),
    }
}

/// Capture Emby-owned local state before a different-server replacement.
pub fn snapshot_emby_owned_state() -> Result<EmbyOwnedStateSnapshot, String> {
    Ok(EmbyOwnedStateSnapshot {
        queue: snapshot_file(&queue_state_path())?,
        library_positions: snapshot_file(&library_position_state_path())?,
        config: snapshot_file(&config_path())?,
        legacy_token: snapshot_file(&token_cache_path())?,
        secret: snapshot_file(&service_secret_path(ServiceKind::Emby))?,
        image_cache: snapshot_emby_image_cache()?,
    })
}

fn restore_file(path: &std::path::Path, bytes: &Option<Vec<u8>>) -> Result<(), String> {
    match bytes {
        Some(bytes) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("restore owner state: {error}"))?;
            }
            let tmp = path.with_extension("restore.tmp");
            std::fs::write(&tmp, bytes).map_err(|error| format!("restore owner state: {error}"))?;
            std::fs::rename(&tmp, path).map_err(|error| format!("restore owner state: {error}"))
        }
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("restore owner state: {error}")),
        },
    }
}

/// Restore a snapshot after cleanup or persistence failed.
pub fn restore_emby_owned_state(snapshot: &EmbyOwnedStateSnapshot) -> Result<(), String> {
    restore_file(&queue_state_path(), &snapshot.queue)?;
    restore_file(&library_position_state_path(), &snapshot.library_positions)?;
    restore_file(&config_path(), &snapshot.config)?;
    restore_file(&token_cache_path(), &snapshot.legacy_token)?;
    restore_file(&service_secret_path(ServiceKind::Emby), &snapshot.secret)?;
    restore_emby_image_cache(&snapshot.image_cache)
}

/// Clear only state whose identity belongs to Emby. Feed and other Service
/// queue entries remain in the persisted queue snapshot.
pub fn clear_emby_owned_state() -> Result<(), String> {
    clear_emby_owned_state_inner()
}

fn clear_emby_owned_state_inner() -> Result<(), String> {
    if let Some(bytes) = snapshot_file(&queue_state_path())? {
        let state: QueueState = serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse Emby queue state: {error}"))?;
        save_queue_state(&state.without_emby())?;
    }

    for path in [token_cache_path(), library_position_state_path()] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("remove Emby-owned state: {error}")),
        }
    }

    let config = config_path();
    if let Ok(text) = std::fs::read_to_string(&config) {
        let mut document: toml::Value =
            toml::from_str(&text).map_err(|error| format!("parse owner configuration: {error}"))?;
        if let Some(table) = document.as_table_mut() {
            table.remove("library_routes");
        } else {
            return Err("owner configuration is not a table".to_string());
        }
        let text = toml::to_string(&document)
            .map_err(|error| format!("serialize owner configuration: {error}"))?;
        write_config_text_at(&config, &text)?;
    }
    clear_emby_image_cache();
    Ok(())
}

fn clear_emby_image_cache() {
    let cache = image_cache_dir();
    let Ok(entries) = std::fs::read_dir(cache) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if is_emby_cache(&name) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn image_cache_dir() -> std::path::PathBuf {
    cache_dir().join("images")
}

fn is_emby_cache(name: &str) -> bool {
    !name.starts_with("audiobookshelf_")
}

fn snapshot_emby_image_cache() -> Result<Vec<(String, Vec<u8>)>, String> {
    let Ok(entries) = std::fs::read_dir(image_cache_dir()) else {
        return Ok(Vec::new());
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            is_emby_cache(&name).then(|| {
                std::fs::read(entry.path())
                    .map(|bytes| (name, bytes))
                    .map_err(|error| format!("read Emby image cache: {error}"))
            })
        })
        .collect()
}

fn restore_emby_image_cache(snapshot: &[(String, Vec<u8>)]) -> Result<(), String> {
    let dir = image_cache_dir();
    std::fs::create_dir_all(&dir).map_err(|error| format!("restore Emby image cache: {error}"))?;
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if is_emby_cache(&entry.file_name().to_string_lossy()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    for (name, bytes) in snapshot {
        std::fs::write(dir.join(name), bytes)
            .map_err(|error| format!("restore Emby image cache: {error}"))?;
    }
    Ok(())
}

/// Clear Emby-owned state and then commit a validated setup. The existing
/// setup/secret transaction snapshots both durable halves; this outer seam
/// adds the owner-state snapshot and rollback required for replacement.
pub fn replace_emby_setup_and_secret(setup: &EmbySetup, token: &str) -> Result<(), String> {
    let snapshot = snapshot_emby_owned_state()?;
    if let Err(error) = clear_emby_owned_state_inner() {
        return restore_after_failure(error, &snapshot);
    }
    match persist_emby_setup_and_secret(setup, token) {
        Ok(()) => Ok(()),
        Err(error) => restore_after_failure(error, &snapshot),
    }
}

fn restore_after_failure(error: String, snapshot: &EmbyOwnedStateSnapshot) -> Result<(), String> {
    match restore_emby_owned_state(snapshot) {
        Ok(()) => Err(error),
        Err(restore_error) => Err(format!(
            "{error}; owner-state rollback failed: {restore_error}"
        )),
    }
}
