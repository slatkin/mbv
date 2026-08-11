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
        |setup, path| save_emby_setup_at(setup, path),
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

// ── System-instance path routing ─────────────────────────────────────────
//
// `std::env::set_var`/`var` read and write the process's single, global
// `environ` table with no synchronization of their own — mutating *any*
// env var on one thread can race with a read of a *different* env var on
// another thread (the underlying C `environ` array can be reallocated
// out from under a concurrent reader). So every test anywhere in the
// crate that touches ANY env var via these functions must serialize on
// one shared lock, not just tests that happen to touch the same variable
// name. This is THE single shared lock for that: src/app/action.rs,
// src/app/actions.rs, and src/api.rs all reference this same
// `SYS_ENV_LOCK` (via `crate::config::tests::SYS_ENV_LOCK`) rather than
// defining their own — independent per-file mutexes don't exclude each
// other and previously caused flaky cross-test env-var races (e.g. one
// test's queue_state.json read intermittently coming back empty because
// an unrelated, unguarded HOSTNAME mutation in api.rs raced it).
use std::sync::Mutex;
pub static SYS_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn is_system_instance_false_without_env_var() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::remove_var("MBV_SYSTEM");
    assert!(!is_system_instance());
}

#[test]
fn is_system_instance_true_with_env_var() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::set_var("MBV_SYSTEM", "1");
    let result = is_system_instance();
    std::env::remove_var("MBV_SYSTEM");
    assert!(result);
}

#[test]
fn cache_dir_uses_system_path_when_mbv_system_set() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::set_var("MBV_SYSTEM", "1");
    let path = cache_dir();
    std::env::remove_var("MBV_SYSTEM");
    assert_eq!(path, std::path::PathBuf::from("/var/cache/mbv"));
}

#[test]
fn cache_dir_uses_xdg_when_not_system() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::remove_var("MBV_SYSTEM");
    std::env::set_var("XDG_CACHE_HOME", "/tmp/xdg-test-cache");
    let path = cache_dir();
    std::env::remove_var("XDG_CACHE_HOME");
    assert_eq!(path, std::path::PathBuf::from("/tmp/xdg-test-cache/mbv"));
}

#[test]
fn data_dir_system_or_local_uses_system_path_when_mbv_system_set() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::set_var("MBV_SYSTEM", "1");
    let path = data_dir_system_or_local();
    std::env::remove_var("MBV_SYSTEM");
    assert_eq!(path, std::path::PathBuf::from("/var/lib/mbv"));
}

#[test]
fn config_path_uses_system_path_when_mbv_system_set() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::set_var("MBV_SYSTEM", "1");
    let path = config_path();
    std::env::remove_var("MBV_SYSTEM");
    assert_eq!(path, std::path::PathBuf::from("/etc/mbv/config.toml"));
}

#[test]
fn mpv_ipc_path_uses_run_dir_when_mbv_system_set() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::set_var("MBV_SYSTEM", "1");
    let path = mpv_ipc_path();
    std::env::remove_var("MBV_SYSTEM");
    assert_eq!(path, "/run/mbv/mbv-mpv.sock");
}

#[test]
fn control_socket_path_uses_run_dir_when_mbv_system_set() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::set_var("MBV_SYSTEM", "1");
    let path = control_socket_path();
    std::env::remove_var("MBV_SYSTEM");
    assert_eq!(path, "/run/mbv/mbv-ctrl.sock");
}

#[test]
fn daemon_server_tcp_listen_defaults_for_system_instance() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::set_var("MBV_SYSTEM", "1");
    let cfg = parse_config("[server]\nurl = \"http://host\"").unwrap();
    std::env::remove_var("MBV_SYSTEM");
    assert_eq!(
        cfg.daemon_server_tcp_listen,
        DEFAULT_SYSTEM_DAEMON_TCP_LISTEN
    );
}

#[test]
fn mpv_ipc_path_uses_xdg_runtime_dir_when_not_system() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    std::env::remove_var("MBV_SYSTEM");
    std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");
    let path = mpv_ipc_path();
    std::env::remove_var("XDG_RUNTIME_DIR");
    assert_eq!(path, "/run/user/1000/mbv-mpv.sock");
}

#[test]
fn save_and_load_last_remote_connection_round_trips_library_route() {
    let _guard = TestStateDirGuard::new();
    let conn = LastRemoteConnection::LibraryRoute {
        library: "music".to_string(),
    };

    assert!(save_last_remote_connection(Some(&conn)).is_ok());

    assert_eq!(load_last_remote_connection().unwrap(), Some(conn));
}

#[test]
fn save_and_load_last_remote_connection_round_trips_direct_session() {
    let _guard = TestStateDirGuard::new();
    let conn = LastRemoteConnection::DirectSession {
        device_name: "living-room-mbv".to_string(),
    };

    assert!(save_last_remote_connection(Some(&conn)).is_ok());

    assert_eq!(load_last_remote_connection().unwrap(), Some(conn));
}

#[test]
fn save_last_remote_connection_none_clears_a_previously_saved_record() {
    let _guard = TestStateDirGuard::new();
    assert!(
        save_last_remote_connection(Some(&LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        }))
        .is_ok()
    );

    assert!(save_last_remote_connection(None).is_ok());

    assert_eq!(load_last_remote_connection().unwrap(), None);
}

#[test]
fn load_last_remote_connection_returns_none_when_no_file_exists() {
    let _guard = TestStateDirGuard::new();
    assert_eq!(load_last_remote_connection().unwrap(), None);
}

#[test]
fn save_last_remote_connection_reports_remove_failure_with_path() {
    let dir = std::env::temp_dir().join(format!("mbv-save-state-error-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let error = save_last_remote_connection_at(&dir, None).unwrap_err();
    assert!(error.contains("remove"));
    assert!(error.contains(dir.to_str().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn load_last_remote_connection_reports_read_failure_with_path() {
    let dir = std::env::temp_dir().join(format!("mbv-load-state-error-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let error = load_last_remote_connection_at(&dir).unwrap_err();
    assert!(error.starts_with("read "));
    assert!(error.contains(dir.to_str().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_config_settings_reports_rename_failure_with_path() {
    let dir = std::env::temp_dir().join(format!("mbv-save-config-error-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let error = write_config_text_at(&dir, "").unwrap_err();
    assert!(error.contains("rename"));
    assert!(error.contains(dir.to_str().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_config_settings_reports_read_failure_with_path() {
    let dir = std::env::temp_dir().join(format!("mbv-read-config-error-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let error = save_config_settings_at(&Config::default(), &dir).unwrap_err();
    assert!(error.contains("read"));
    assert!(error.contains(dir.to_str().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_config_settings_reports_parse_failure_with_path() {
    let dir = std::env::temp_dir().join(format!("mbv-parse-config-error-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "this = [is malformed").unwrap();
    let error = save_config_settings_at(&Config::default(), &path).unwrap_err();
    assert!(error.contains("parse"));
    assert!(error.contains(path.to_str().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_config_settings_reports_write_failure_with_path() {
    let dir = std::env::temp_dir().join(format!("mbv-write-config-error-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "").unwrap();
    std::fs::create_dir(path.with_extension("toml.tmp")).unwrap();
    let error = save_config_settings_at(&Config::default(), &path).unwrap_err();
    assert!(error.contains("write"));
    assert!(error.contains(path.with_extension("toml.tmp").to_str().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_queue_state_preserves_previous_snapshot_on_write_failure() {
    let _guard = TestStateDirGuard::new();
    let original = QueueState {
        source: QueueSource::Playlist {
            id: Some("pl-1".into()),
            name: "Test".into(),
        },
        items: vec![],
        cursor: 0,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    };

    assert!(save_queue_state(&original).is_ok());

    // Make the tmp path a directory so the write will fail
    let path = queue_state_path();
    let tmp = path.with_extension("json.tmp");
    std::fs::create_dir(&tmp).unwrap();

    let modified = QueueState {
        source: QueueSource::Unknown,
        items: vec![],
        cursor: 5,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    };
    let result = save_queue_state(&modified);
    assert!(result.is_err(), "write to directory should fail");

    // Previous snapshot must survive
    let loaded = load_queue_state().expect("previous snapshot should survive failed write");
    assert_eq!(
        loaded.source,
        QueueSource::Playlist {
            id: Some("pl-1".into()),
            name: "Test".into(),
        }
    );

    // Clean up the directory we created
    std::fs::remove_dir(&tmp).unwrap();
}

// ── Service-independent startup tests (tasks 1.2–1.4) ────────────────
#[test]
fn service_secret_write_and_read_round_trips() {
    let _guard = TestStateDirGuard::new();
    let secret = "emby-token-abc123";
    save_service_secret(ServiceKind::Emby, secret).unwrap();
    assert_eq!(
        load_service_secret(ServiceKind::Emby).as_deref(),
        Some(secret)
    );
}
#[test]
fn service_secret_read_returns_none_when_absent() {
    let _guard = TestStateDirGuard::new();
    assert_eq!(load_service_secret(ServiceKind::Emby), None);
}
#[test]
fn service_secret_file_has_correct_name() {
    let _guard = TestStateDirGuard::new();
    let path = service_secret_path(ServiceKind::Emby);
    assert_eq!(path.file_name().unwrap(), "emby.json");
    assert_eq!(path.parent().unwrap().file_name().unwrap(), "secrets");
}
#[cfg(unix)]
#[test]
fn service_secret_permissions_are_mode_0600() {
    let _guard = TestStateDirGuard::new();
    save_service_secret(ServiceKind::Emby, "secret-token").unwrap();
    let path = service_secret_path(ServiceKind::Emby);
    let meta = std::fs::metadata(&path).unwrap();
    let perms = meta.permissions();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        perms.mode() & 0o777,
        0o600,
        "service secret must be owner-only: {:?} has mode {:o}",
        path,
        perms.mode()
    );
}
#[test]
fn control_credential_write_and_read_round_trips() {
    let _guard = TestStateDirGuard::new();
    let cred = load_or_create_control_credential().unwrap();
    for (kind, secret) in [
        (ServiceKind::Emby, "emby-token"),
        (ServiceKind::Audiobookshelf, "audiobookshelf-token"),
    ] {
        save_service_secret(kind, secret).unwrap();
    }
    let loaded = load_or_create_control_credential().unwrap();
    assert_eq!(loaded, cred);
    assert_eq!(load_control_credential().as_deref(), Some(cred.as_str()));
}
#[test]
fn control_credential_read_returns_none_when_absent() {
    let _guard = TestStateDirGuard::new();
    assert_eq!(load_control_credential(), None);
}
#[test]
fn clear_service_secret_removes_secret_file() {
    let _guard = TestStateDirGuard::new();
    save_service_secret(ServiceKind::Emby, "to-clear").unwrap();
    assert!(load_service_secret(ServiceKind::Emby).is_some());
    clear_service_secret(ServiceKind::Emby);
    assert_eq!(load_service_secret(ServiceKind::Emby), None);
}

#[test]
fn clear_control_credential_removes_credential_file() {
    let _guard = TestStateDirGuard::new();
    save_control_credential("to-clear").unwrap();
    assert!(load_control_credential().is_some());
    clear_control_credential_result().unwrap();
    assert_eq!(load_control_credential(), None);
}

#[cfg(unix)]
#[test]
fn control_credential_permissions_are_mode_0600() {
    let _guard = TestStateDirGuard::new();
    save_control_credential("ctrl-secret").unwrap();
    let path = control_credential_path();
    let meta = std::fs::metadata(&path).unwrap();
    let perms = meta.permissions();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        perms.mode() & 0o777,
        0o600,
        "control credential must be owner-only: {:?} has mode {:o}",
        path,
        perms.mode()
    );
}
