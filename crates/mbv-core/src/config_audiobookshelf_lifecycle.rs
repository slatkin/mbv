fn save_audiobookshelf_setup_at(
    setup: &AudiobookshelfSetup,
    path: &std::path::Path,
) -> Result<(), String> {
    if setup.server_url.trim().is_empty() {
        return Err("Audiobookshelf setup requires a server URL".into());
    }
    let mut doc: toml::Value = match std::fs::read_to_string(path) {
        Ok(text) => {
            toml::from_str(&text).map_err(|error| format!("parse {}: {error}", path.display()))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(error) => return Err(format!("read {}: {error}", path.display())),
    };
    let table = doc
        .as_table_mut()
        .ok_or_else(|| format!("update {}: root is not a table", path.display()))?;
    let section = table
        .entry("audiobookshelf")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| format!("update {}: audiobookshelf is not a table", path.display()))?;
    section.insert("url".into(), toml::Value::String(setup.server_url.clone()));
    section.insert(
        "revision".into(),
        toml::Value::Integer(setup.revision as i64),
    );
    section.remove("api_key");
    section.remove("user_id");
    let text =
        toml::to_string(&doc).map_err(|error| format!("serialize {}: {error}", path.display()))?;
    write_config_text_at(path, &text)
}

fn clear_audiobookshelf_setup_at(path: &std::path::Path) -> Result<(), String> {
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
    table.remove("audiobookshelf");
    let text =
        toml::to_string(&doc).map_err(|error| format!("serialize {}: {error}", path.display()))?;
    write_config_text_at(path, &text)
}

fn audiobookshelf_transaction<F>(operation: F) -> Result<(), String>
where
    F: FnOnce(&std::path::Path, &std::path::Path) -> Result<(), String>,
{
    let config = config_path();
    let secret = service_secret_path(ServiceKind::Audiobookshelf);
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
            (config, secret) => Err(format!(
                "{reason}; rollback failed (config={config:?}, secret={secret:?})"
            )),
        }
    };
    if let Err(error) = operation(&config, &secret) {
        return rollback(error);
    }
    Ok(())
}

/// Compute the next persisted revision for a committed Audiobookshelf setup:
/// `1` for a first setup, otherwise one more than the currently persisted
/// revision. Distinct from the in-memory `SetupGeneration`.
fn next_audiobookshelf_revision() -> Result<u64, String> {
    let existing = load_config()
        .ok()
        .and_then(|config| config.audiobookshelf_setup)
        .map(|setup| setup.revision);
    match existing {
        None => Ok(1),
        Some(revision) => revision
            .checked_add(1)
            .ok_or_else(|| "Audiobookshelf setup revision exhausted".to_string()),
    }
}

/// Commit a validated Audiobookshelf candidate. The candidate validator must
/// run before this boundary; this function only owns durable setup/secret IO.
/// Returns the committed revision so callers can reconcile a running owner.
pub fn persist_audiobookshelf_setup_and_secret(
    setup: &AudiobookshelfSetup,
    api_key: &str,
) -> Result<u64, String> {
    if api_key.trim().is_empty() {
        return Err("Audiobookshelf setup requires an API key".into());
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

/// Consume a validator result only after validation has succeeded. The
/// returned identity is runtime-only and is never serialized by this seam.
pub fn commit_audiobookshelf_candidate(
    candidate: crate::audiobookshelf::AudiobookshelfValidatedSetup,
) -> Result<(crate::audiobookshelf::AudiobookshelfUser, u64), String> {
    let (setup, user, api_key) = candidate.into_parts();
    let revision = persist_audiobookshelf_setup_and_secret(&setup, &api_key)?;
    Ok((user, revision))
}

pub fn repair_audiobookshelf_candidate(
    candidate: crate::audiobookshelf::AudiobookshelfValidatedSetup,
) -> Result<(crate::audiobookshelf::AudiobookshelfUser, u64), String> {
    commit_audiobookshelf_candidate(candidate)
}

/// Remove Audiobookshelf files without touching Emby, Feeds, or control state.
pub fn remove_audiobookshelf_setup_and_secret() -> Result<(), String> {
    remove_audiobookshelf_setup_and_secret_with_owned_state(|| Ok(()), || {})
}

pub fn remove_audiobookshelf_setup_and_secret_with_owned_state<C, R>(
    clear_owned_state: C,
    restore_owned_state: R,
) -> Result<(), String>
where
    C: FnOnce() -> Result<(), String>,
    R: FnOnce(),
{
    let result = audiobookshelf_transaction(|config, _secret| {
        clear_audiobookshelf_setup_at(config)?;
        clear_service_secret_result(ServiceKind::Audiobookshelf)
            .map_err(|error| format!("remove Audiobookshelf secret: {error}"))?;
        clear_owned_state()
    });
    if result.is_err() {
        restore_owned_state();
    }
    result
}

/// Replacement/removal seam for Audiobookshelf-owned local state. Cleanup is
/// deliberately between clearing the old durable setup and committing a new
/// one, and its rollback callback restores the owned state if persistence fails.
pub fn replace_audiobookshelf_setup_and_secret<C, R>(
    setup: &AudiobookshelfSetup,
    api_key: &str,
    clear_owned_state: C,
    restore_owned_state: R,
) -> Result<u64, String>
where
    C: FnOnce() -> Result<(), String>,
    R: FnOnce(),
{
    let revision = next_audiobookshelf_revision()?;
    let mut setup = setup.clone();
    setup.revision = revision;
    let result = audiobookshelf_transaction(|config, secret| {
        clear_audiobookshelf_setup_at(config)?;
        clear_service_secret_result(ServiceKind::Audiobookshelf)
            .map_err(|error| format!("remove Audiobookshelf secret: {error}"))?;
        clear_owned_state()?;
        save_audiobookshelf_setup_at(&setup, config)?;
        save_service_secret_at(api_key, secret)
    });
    if result.is_err() {
        restore_owned_state();
    }
    result.map(|()| revision)
}

/// Confirmed different-server replacement. Validation is represented by the
/// candidate type; confirmation belongs to the caller and must precede this
/// destructive boundary.
pub fn replace_audiobookshelf_candidate<C, R>(
    candidate: crate::audiobookshelf::AudiobookshelfValidatedSetup,
    clear_owned_state: C,
    restore_owned_state: R,
) -> Result<(crate::audiobookshelf::AudiobookshelfUser, u64), String>
where
    C: FnOnce() -> Result<(), String>,
    R: FnOnce(),
{
    let (setup, user, api_key) = candidate.into_parts();
    let revision = replace_audiobookshelf_setup_and_secret(
        &setup,
        &api_key,
        clear_owned_state,
        restore_owned_state,
    )?;
    Ok((user, revision))
}
