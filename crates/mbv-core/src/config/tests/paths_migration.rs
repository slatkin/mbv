/// ── Legacy Emby token migration tests ──────────────────────────────

#[test]
fn legacy_token_migration_writes_new_secret_and_removes_legacy() {
    let _guard = TestStateDirGuard::new();
    // Write a legacy token.json
    let legacy = token_cache_path();
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(
        &legacy,
        r#"{"server_url":"http://emby.example","token":"migrated-token","user_id":"u-42"}"#,
    )
    .unwrap();

    assert!(legacy.exists(), "legacy token must exist before migration");
    migrate_legacy_emby_token().unwrap();

    // New secret must exist with the token value.
    let new_path = service_secret_path(ServiceKind::Emby);
    assert!(
        new_path.exists(),
        "new service secret must exist after migration"
    );
    assert_eq!(
        load_service_secret(ServiceKind::Emby).as_deref(),
        Some("migrated-token")
    );
    let setup = load_config().unwrap().emby_setup.unwrap();
    assert_eq!(setup.server_url, "http://emby.example");
    assert_eq!(setup.user_id, "u-42");

    // Legacy file must have been removed.
    assert!(
        !legacy.exists(),
        "legacy token must be removed after successful migration"
    );
}

#[test]
fn legacy_token_migration_no_legacy_file_is_not_an_error() {
    let _guard = TestStateDirGuard::new();
    // No token.json exists; migration is a no-op.
    assert!(!token_cache_path().exists());
    migrate_legacy_emby_token().unwrap();
    // New secret must not exist either.
    assert!(!service_secret_path(ServiceKind::Emby).exists());
}

#[test]
fn legacy_token_migration_with_empty_token_reports_error() {
    let _guard = TestStateDirGuard::new();
    let legacy = token_cache_path();
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(
        &legacy,
        r#"{"server_url":"http://emby.example","token":"","user_id":"u-42"}"#,
    )
    .unwrap();

    let err = migrate_legacy_emby_token().unwrap_err();
    assert!(err.contains("empty"), "error must mention empty token");
    assert!(err.contains("token"), "error must mention 'token' field");

    // Legacy file must survive the failed migration.
    assert!(
        legacy.exists(),
        "legacy token must survive failed migration"
    );
}

#[test]
fn failed_new_secret_write_retains_legacy_token() {
    let _guard = TestStateDirGuard::new();
    // Write a legacy token.json.
    let legacy = token_cache_path();
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(
        &legacy,
        r#"{"server_url":"http://emby.example","token":"keep-me","user_id":"u-42"}"#,
    )
    .unwrap();

    // Create a file at the secrets directory path so create_dir_all fails.
    let secrets_path = state_dir().join("secrets");
    std::fs::write(&secrets_path, b"I am a file not a directory").unwrap();

    let err = migrate_legacy_emby_token().unwrap_err();
    assert!(
        err.contains("migrate"),
        "error must mention migration context"
    );
    assert!(
        err.contains("secrets"),
        "error must mention the failing path"
    );

    // Legacy file must survive despite the write failure.
    assert!(
        legacy.exists(),
        "legacy token must survive failed new-secret write"
    );
    let text = std::fs::read_to_string(&legacy).unwrap();
    assert!(
        text.contains("keep-me"),
        "legacy token content must be unchanged"
    );

    // Clean up the file we planted
    std::fs::remove_file(&secrets_path).unwrap();
}

#[test]
fn failed_setup_write_retains_legacy_token() {
    let _guard = TestStateDirGuard::new();
    let legacy = token_cache_path();
    std::fs::write(
        &legacy,
        r#"{"server_url":"http://emby.example","token":"keep-me","user_id":"u-42"}"#,
    )
    .unwrap();
    std::fs::create_dir(config_path().with_extension("toml.tmp")).unwrap();

    let err = migrate_legacy_emby_token().unwrap_err();
    assert!(err.contains("setup") && err.contains("config.toml"));
    assert!(legacy.exists());
    assert!(!service_secret_path(ServiceKind::Emby).exists());
}

#[test]
fn migration_is_idempotent_and_does_not_overwrite_existing_setup_or_secret() {
    let _guard = TestStateDirGuard::new();
    save_emby_setup(&EmbySetup::new("http://new.example", "new-user")).unwrap();
    save_service_secret(ServiceKind::Emby, "new-token").unwrap();
    let legacy = token_cache_path();
    std::fs::write(
        &legacy,
        r#"{"server_url":"http://stale.example","token":"stale-token","user_id":"stale-user"}"#,
    )
    .unwrap();

    migrate_legacy_emby_token().unwrap();
    assert!(!legacy.exists());
    assert_eq!(
        load_service_secret(ServiceKind::Emby).as_deref(),
        Some("new-token")
    );
    assert_eq!(
        load_config().unwrap().emby_setup.unwrap().user_id,
        "new-user"
    );
    migrate_legacy_emby_token().unwrap();
}

#[test]
fn setup_only_migration_resumes_when_legacy_identity_matches() {
    let _guard = TestStateDirGuard::new();
    save_emby_setup(&EmbySetup::new("http://emby.example/", "u-42")).unwrap();
    let legacy = token_cache_path();
    std::fs::write(
        &legacy,
        r#"{"server_url":"http://emby.example/","token":"resumed-token","user_id":"u-42"}"#,
    )
    .unwrap();

    migrate_legacy_emby_token().unwrap();
    assert_eq!(
        load_service_secret(ServiceKind::Emby).as_deref(),
        Some("resumed-token")
    );
    assert!(!legacy.exists());
}

#[test]
fn setup_only_migration_retains_conflicting_legacy_without_writing_secret() {
    let _guard = TestStateDirGuard::new();
    save_emby_setup(&EmbySetup::new("http://current.example", "current-user")).unwrap();
    let legacy = token_cache_path();
    std::fs::write(
        &legacy,
        r#"{"server_url":"http://stale.example","token":"stale-token","user_id":"stale-user"}"#,
    )
    .unwrap();

    let err = migrate_legacy_emby_token().unwrap_err();
    assert!(err.contains("conflicts"));
    assert!(err.contains("config.toml"));
    assert!(legacy.exists());
    assert!(!service_secret_path(ServiceKind::Emby).exists());
}

#[test]
fn secret_only_migration_retains_legacy_without_guessing_setup() {
    let _guard = TestStateDirGuard::new();
    save_service_secret(ServiceKind::Emby, "authoritative-token").unwrap();
    let legacy = token_cache_path();
    std::fs::write(
        &legacy,
        r#"{"server_url":"http://legacy.example","token":"legacy-token","user_id":"legacy-user"}"#,
    )
    .unwrap();

    let err = migrate_legacy_emby_token().unwrap_err();
    assert!(err.contains("no persisted setup"));
    assert!(err.contains("emby.json"));
    assert!(legacy.exists());
    assert!(load_config().unwrap().emby_setup.is_none());
    assert_eq!(
        load_service_secret(ServiceKind::Emby).as_deref(),
        Some("authoritative-token")
    );
}
