use super::{
    AudiobookshelfSetup, ConfigError, ServiceKind, clear_service_secret_result, config_path,
    load_config, save_service_secret_at, service_secret_path, write_config_text_at,
};

pub(super) fn save_audiobookshelf_setup_at(
    setup: &AudiobookshelfSetup,
    path: &std::path::Path,
) -> Result<(), ConfigError> {
    if setup.server_url.trim().is_empty() {
        return Err(ConfigError::lifecycle(
            "Audiobookshelf setup requires a server URL",
        ));
    }
    let mut doc: toml::Value = match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|error| {
            ConfigError::lifecycle(format!("parse {}: {error}", path.display()))
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(error) => {
            return Err(ConfigError::lifecycle(format!(
                "read {}: {error}",
                path.display()
            )));
        }
    };
    let table = doc.as_table_mut().ok_or_else(|| {
        ConfigError::lifecycle(format!("update {}: root is not a table", path.display()))
    })?;
    let section = table
        .entry("audiobookshelf")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| {
            ConfigError::lifecycle(format!(
                "update {}: audiobookshelf is not a table",
                path.display()
            ))
        })?;
    section.insert("url".into(), toml::Value::String(setup.server_url.clone()));
    section.insert(
        "revision".into(),
        toml::Value::Integer(i64::try_from(setup.revision).unwrap_or(i64::MAX)),
    );
    section.remove("api_key");
    section.remove("user_id");
    let text = toml::to_string(&doc).map_err(|error| {
        ConfigError::lifecycle(format!("serialize {}: {error}", path.display()))
    })?;
    write_config_text_at(path, &text)
}

fn clear_audiobookshelf_setup_at(path: &std::path::Path) -> Result<(), ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(ConfigError::lifecycle(format!(
                "read {}: {error}",
                path.display()
            )));
        }
    };
    let mut doc: toml::Value = toml::from_str(&text)
        .map_err(|error| ConfigError::lifecycle(format!("parse {}: {error}", path.display())))?;
    let table = doc.as_table_mut().ok_or_else(|| {
        ConfigError::lifecycle(format!("update {}: root is not a table", path.display()))
    })?;
    table.remove("audiobookshelf");
    let text = toml::to_string(&doc).map_err(|error| {
        ConfigError::lifecycle(format!("serialize {}: {error}", path.display()))
    })?;
    write_config_text_at(path, &text)
}

fn snapshot_file(path: &std::path::Path) -> Result<Option<Vec<u8>>, ConfigError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ConfigError::lifecycle(format!(
            "read {} for rollback: {error}",
            path.display()
        ))),
    }
}

fn restore_file(
    path: &std::path::Path,
    bytes: Option<&[u8]>,
    secret_path: &std::path::Path,
) -> Result<(), ConfigError> {
    match bytes {
        Some(bytes) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let tmp = path.with_extension("rollback.tmp");
            std::fs::write(&tmp, bytes)?;
            #[cfg(unix)]
            if path == secret_path {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
            }
            Ok(std::fs::rename(tmp, path)?)
        }
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ConfigError::from(error)),
        },
    }
}

fn rollback(
    config: &std::path::Path,
    old_config: Option<&[u8]>,
    secret: &std::path::Path,
    old_secret: Option<&[u8]>,
    reason: ConfigError,
) -> Result<(), ConfigError> {
    let config_result = restore_file(config, old_config, secret).map_err(|error| error.to_string());
    let secret_result = restore_file(secret, old_secret, secret).map_err(|error| error.to_string());
    match (config_result, secret_result) {
        (Ok(()), Ok(())) => Err(reason),
        (config, secret) => Err(ConfigError::lifecycle(format!(
            "{reason}; rollback failed (config={config:?}, secret={secret:?})"
        ))),
    }
}

pub(super) fn audiobookshelf_transaction<F>(operation: F) -> Result<(), ConfigError>
where
    F: FnOnce(&std::path::Path, &std::path::Path) -> Result<(), ConfigError>,
{
    let config = config_path();
    let secret = service_secret_path(ServiceKind::Audiobookshelf);
    let old_config = snapshot_file(&config)?;
    let old_secret = snapshot_file(&secret)?;
    if let Err(error) = operation(&config, &secret) {
        return rollback(
            &config,
            old_config.as_deref(),
            &secret,
            old_secret.as_deref(),
            error,
        );
    }
    Ok(())
}

/// Compute the next persisted revision for a committed Audiobookshelf setup:
/// `1` for a first setup, otherwise one more than the currently persisted
/// revision. Distinct from the in-memory `SetupGeneration`.
///
/// ponytail: read-modify-write is not atomic across processes. Safe in practice
/// because mbvd serializes via `administration_lock("abs")` and the bare-mode
/// TUI writes a disjoint (non-system) config file; add a cross-process lock if
/// concurrent same-file writers ever appear.
fn next_audiobookshelf_revision() -> Result<u64, ConfigError> {
    let existing = load_config()
        .ok()
        .and_then(|config| config.audiobookshelf_setup)
        .map(|setup| setup.revision);
    match existing {
        None => Ok(1),
        Some(revision) => revision
            .checked_add(1)
            .ok_or_else(|| ConfigError::lifecycle("Audiobookshelf setup revision exhausted")),
    }
}

/// Commit a validated Audiobookshelf candidate. The candidate validator must
/// run before this boundary; this function only owns durable setup/secret IO.
/// Returns the committed revision so callers can reconcile a running owner.
pub fn persist_audiobookshelf_setup_and_secret(
    setup: &AudiobookshelfSetup,
    api_key: &str,
) -> Result<u64, ConfigError> {
    if api_key.trim().is_empty() {
        return Err(ConfigError::lifecycle(
            "Audiobookshelf setup requires an API key",
        ));
    }
    let revision = next_audiobookshelf_revision()?;
    let mut setup = setup.clone();
    setup.revision = revision;
    audiobookshelf_transaction(|config, secret| {
        save_audiobookshelf_setup_at(&setup, config)?;
        save_service_secret_at(api_key, secret)
    })?;
    Ok(revision)
}

/// Remove Audiobookshelf files without touching Emby, Feeds, or control state.
pub fn remove_audiobookshelf_setup_and_secret() -> Result<(), ConfigError> {
    remove_audiobookshelf_setup_and_secret_with_owned_state(
        || -> Result<(), ConfigError> { Ok(()) },
        || {},
    )
}

pub fn remove_audiobookshelf_setup_and_secret_with_owned_state<C, R, E>(
    clear_owned_state: C,
    restore_owned_state: R,
) -> Result<(), ConfigError>
where
    C: FnOnce() -> Result<(), E>,
    E: std::fmt::Display,
    R: FnOnce(),
{
    let result = audiobookshelf_transaction(|config, _secret| {
        clear_audiobookshelf_setup_at(config)?;
        clear_service_secret_result(ServiceKind::Audiobookshelf).map_err(|error| {
            ConfigError::lifecycle(format!("remove Audiobookshelf secret: {error}"))
        })?;
        clear_owned_state().map_err(|error| ConfigError::lifecycle(error.to_string()))
    });
    if result.is_err() {
        restore_owned_state();
    }
    result
}

/// Replacement/removal seam for Audiobookshelf-owned local state. Cleanup is
/// deliberately between clearing the old durable setup and committing a new
/// one, and its rollback callback restores the owned state if persistence fails.
pub fn replace_audiobookshelf_setup_and_secret<C, R, E>(
    setup: &AudiobookshelfSetup,
    api_key: &str,
    clear_owned_state: C,
    restore_owned_state: R,
) -> Result<u64, ConfigError>
where
    C: FnOnce() -> Result<(), E>,
    E: std::fmt::Display,
    R: FnOnce(),
{
    let revision = next_audiobookshelf_revision()?;
    let mut setup = setup.clone();
    setup.revision = revision;
    let result = audiobookshelf_transaction(|config, secret| {
        clear_audiobookshelf_setup_at(config)?;
        clear_service_secret_result(ServiceKind::Audiobookshelf).map_err(|error| {
            ConfigError::lifecycle(format!("remove Audiobookshelf secret: {error}"))
        })?;
        clear_owned_state().map_err(|error| ConfigError::lifecycle(error.to_string()))?;
        save_audiobookshelf_setup_at(&setup, config)?;
        save_service_secret_at(api_key, secret)
    });
    if result.is_err() {
        restore_owned_state();
    }
    result.map(|()| revision)
}
