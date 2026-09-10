#[test]
fn library_position_state_round_trips_by_library() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::remove_var("MBV_SYSTEM");
    let temp = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::env::set_var("XDG_STATE_HOME", &temp);

    let mut state = LibraryPositionState::default();
    state.libraries.insert(
        "lib-movies".into(),
        LibraryPosition {
            levels: vec![LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                focused_item_id: Some("movie-2".into()),
                cursor_index: 7,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: None,
            }],
            feed_selected_group: 0,
            feed_video_cursor: 0,
            feed_video_scroll: 0,
        },
    );

    save_library_position_state(&state);

    assert_eq!(load_library_position_state(), state);

    std::env::remove_var("XDG_STATE_HOME");
    let _ = std::fs::remove_dir_all(temp);
}

#[cfg(test)]
fn transaction_test_paths() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("mbv-emby-transaction-{}", uuid::Uuid::new_v4()));
    let config = root.join("config.toml");
    let secret = root.join("secrets/emby.json");
    let _ = std::fs::create_dir_all(secret.parent().unwrap());
    (root, config, secret)
}

#[test]
fn emby_setup_persistence_removes_legacy_credentials_and_preserves_unrelated_toml() {
    let _guard = TestStateDirGuard::new();
    std::fs::write(
        config_path(),
        "[server]\nurl = \"old\"\nuser_id = \"old-user\"\nusername = \"alice\"\npassword = \"secret\"\napi_key = \"key\"\n[general]\nkeep = true\n",
    )
    .unwrap();
    persist_emby_setup_and_secret(&EmbySetup::new("https://new/", "new-user"), "new-token")
        .unwrap();
    let doc: toml::Value =
        toml::from_str(&std::fs::read_to_string(config_path()).unwrap()).unwrap();
    let server = doc.get("server").unwrap().as_table().unwrap();
    assert_eq!(server["url"].as_str(), Some("https://new"));
    assert_eq!(server["user_id"].as_str(), Some("new-user"));
    for key in ["username", "password", "api_key"] {
        assert!(!server.contains_key(key), "legacy key remained: {key}");
    }
    assert_eq!(doc["general"]["keep"].as_bool(), Some(true));
    assert_eq!(
        load_service_secret(ServiceKind::Emby).as_deref(),
        Some("new-token")
    );
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    assert_eq!(
        std::fs::metadata(service_secret_path(ServiceKind::Emby))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn emby_setup_transaction_restores_exact_files_on_either_write_failure() {
    let (root, config, secret) = transaction_test_paths();
    let old_config = b"[server]\nurl = \"old\"\nuser_id = \"old-user\"\n";
    let old_secret = b"old secret bytes";
    std::fs::write(&config, old_config).unwrap();
    std::fs::write(&secret, old_secret).unwrap();
    let setup = EmbySetup::new("https://new", "new-user");

    let result = persist_emby_setup_and_secret_at(
        &setup,
        "new-token",
        &config,
        &secret,
        |_setup, _path| Err("setup write rejected".into()),
        |_token, _path| Ok(()),
    );
    assert!(result.unwrap_err().contains("setup write rejected"));
    assert_eq!(std::fs::read(&config).unwrap(), old_config);
    assert_eq!(std::fs::read(&secret).unwrap(), old_secret);

    let result = persist_emby_setup_and_secret_at(
        &setup,
        "new-token",
        &config,
        &secret,
        save_emby_setup_at,
        |_token, _path| Err("secret write rejected".into()),
    );
    assert!(result.unwrap_err().contains("secret write rejected"));
    assert_eq!(std::fs::read(&config).unwrap(), old_config);
    assert_eq!(std::fs::read(&secret).unwrap(), old_secret);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn emby_setup_transaction_rejects_arbitrary_snapshot_read_errors_before_writing() {
    let (root, config, secret) = transaction_test_paths();
    std::fs::write(&config, b"placeholder").unwrap();
    std::fs::remove_file(&config).unwrap();
    std::fs::create_dir(&config).unwrap();
    let old_secret = b"old secret bytes";
    std::fs::write(&secret, old_secret).unwrap();
    let called = std::cell::Cell::new(false);
    let result = persist_emby_setup_and_secret_at(
        &EmbySetup::new("https://new", "new-user"),
        "new-token",
        &config,
        &secret,
        |_setup, _path| {
            called.set(true);
            Ok(())
        },
        |_token, _path| Ok(()),
    );
    assert!(result.unwrap_err().contains("read"));
    assert!(!called.get());
    assert_eq!(std::fs::read(&secret).unwrap(), old_secret);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn audiobookshelf_revision_advances_per_commit() {
    let _guard = TestStateDirGuard::new();
    assert_eq!(
        persist_audiobookshelf_setup_and_secret(
            &AudiobookshelfSetup::new("https://books.example"),
            "first-secret",
        )
        .unwrap(),
        1
    );
    // Same-server repair advances the persisted revision without a clear.
    assert_eq!(
        persist_audiobookshelf_setup_and_secret(
            &AudiobookshelfSetup::new("https://books.example"),
            "repaired-secret",
        )
        .unwrap(),
        2
    );
    // Different-server replacement advances again.
    assert_eq!(
        replace_audiobookshelf_setup_and_secret(
            &AudiobookshelfSetup::new("https://new-books.example"),
            "replacement-secret",
            || Ok(()),
            || {},
        )
        .unwrap(),
        3
    );
    assert_eq!(
        load_config()
            .unwrap()
            .audiobookshelf_setup
            .unwrap()
            .revision,
        3
    );
}

#[test]
fn audiobookshelf_lifecycle_isolated_and_ordered() {
    let _guard = TestStateDirGuard::new();
    std::fs::write(
        config_path(),
        "[server]\nurl = \"emby.example\"\nuser_id = \"emby-user\"\n[feeds]\nkeep = true\n",
    )
    .unwrap();
    save_service_secret(ServiceKind::Emby, "emby-secret").unwrap();
    persist_audiobookshelf_setup_and_secret(
        &AudiobookshelfSetup::new("https://books.example/"),
        "books-secret",
    )
    .unwrap();
    let doc: toml::Value =
        toml::from_str(&std::fs::read_to_string(config_path()).unwrap()).unwrap();
    assert_eq!(
        doc["audiobookshelf"]["url"].as_str(),
        Some("https://books.example")
    );
    assert!(!std::fs::read_to_string(config_path())
        .unwrap()
        .contains("books-secret"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(service_secret_path(ServiceKind::Audiobookshelf))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    let owned_state = std::sync::Arc::new(std::sync::Mutex::new("preserved"));
    persist_audiobookshelf_setup_and_secret(
        &AudiobookshelfSetup::new("https://books.example"),
        "repaired-books-secret",
    )
    .unwrap();
    assert_eq!(*owned_state.lock().unwrap(), "preserved");

    let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let clear_order = order.clone();
    let restore_order = order.clone();
    replace_audiobookshelf_setup_and_secret(
        &AudiobookshelfSetup::new("https://new-books.example"),
        "new-books-secret",
        move || {
            clear_order.lock().unwrap().push("clear");
            Ok(())
        },
        move || restore_order.lock().unwrap().push("restore"),
    )
    .unwrap();
    assert_eq!(&*order.lock().unwrap(), &["clear"]);
    assert_eq!(
        load_service_secret(ServiceKind::Audiobookshelf).as_deref(),
        Some("new-books-secret")
    );
    assert_eq!(
        load_service_secret(ServiceKind::Emby).as_deref(),
        Some("emby-secret")
    );

    let remove_order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let remove_clear = remove_order.clone();
    remove_audiobookshelf_setup_and_secret_with_owned_state(
        move || {
            remove_clear.lock().unwrap().push("clear");
            Ok(())
        },
        || panic!("owned-state restore is not expected"),
    )
    .unwrap();
    assert_eq!(&*remove_order.lock().unwrap(), &["clear"]);
    assert!(load_service_secret(ServiceKind::Audiobookshelf).is_none());
    let config = std::fs::read_to_string(config_path()).unwrap();
    assert!(!config.contains("audiobookshelf"));
    assert!(config.contains("emby.example"));
    assert!(config.contains("[feeds]"));
}

#[test]
fn failed_audiobookshelf_candidate_and_transaction_leave_working_state() {
    let _guard = TestStateDirGuard::new();
    persist_audiobookshelf_setup_and_secret(
        &AudiobookshelfSetup::new("https://working.example"),
        "working-secret",
    )
    .unwrap();
    let before = std::fs::read(config_path()).unwrap();
    let before_secret = load_service_secret(ServiceKind::Audiobookshelf);
    assert!(
        crate::audiobookshelf::AudiobookshelfClient::validate_setup_bounded(
            "",
            "candidate-secret",
            std::time::Duration::from_millis(1)
        )
        .is_err()
    );
    assert_eq!(std::fs::read(config_path()).unwrap(), before);
    assert_eq!(
        load_service_secret(ServiceKind::Audiobookshelf),
        before_secret
    );

    let result = audiobookshelf_transaction(|config, _secret| {
        save_audiobookshelf_setup_at(
            &AudiobookshelfSetup::new("https://candidate.example"),
            config,
        )?;
        Err("candidate persistence rejected".into())
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(config_path()).unwrap(), before);
    assert_eq!(
        load_service_secret(ServiceKind::Audiobookshelf),
        Some("working-secret".into())
    );
}

#[test]
fn load_library_position_state_defaults_for_missing_or_invalid_file() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::remove_var("MBV_SYSTEM");
    let temp = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let state_dir = temp.join("mbv");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::env::set_var("XDG_STATE_HOME", &temp);

    assert_eq!(
        load_library_position_state(),
        LibraryPositionState::default()
    );

    std::fs::write(state_dir.join("library_position_state.json"), "{not json").unwrap();

    assert_eq!(
        load_library_position_state(),
        LibraryPositionState::default()
    );

    std::env::remove_var("XDG_STATE_HOME");
    let _ = std::fs::remove_dir_all(temp);
}

/// #361 collapsed the old `{default, power}` two-scope shape down to a
/// bare `LibraryPosition` per library. A pre-#361 on-disk file still has
/// the nested shape; per decision 7 in the #361 plan, that file is not
/// migrated -- it loads as empty (all libraries reset to root) rather
/// than failing or panicking.
#[test]
fn legacy_nested_scope_shape_loads_as_empty_without_error() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::remove_var("MBV_SYSTEM");
    let temp = std::env::temp_dir().join(format!(
        "mbv-config-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let state_dir = temp.join("mbv");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::env::set_var("XDG_STATE_HOME", &temp);

    let legacy = serde_json::json!({
        "libraries": {
            "lib-1": {
                "default": { "levels": [{"parent_id": "p", "title": "t"}] },
                "power": { "levels": [{"parent_id": "p2", "title": "t2"}] }
            }
        }
    });
    std::fs::write(
        state_dir.join("library_position_state.json"),
        serde_json::to_string(&legacy).unwrap(),
    )
    .unwrap();

    let state = load_library_position_state();
    let restored = state.libraries.get("lib-1").expect("entry present");
    assert!(
        restored.levels.is_empty(),
        "legacy nested scopes must not be salvaged -- library resets to root"
    );

    std::env::remove_var("XDG_STATE_HOME");
    let _ = std::fs::remove_dir_all(temp);
}
