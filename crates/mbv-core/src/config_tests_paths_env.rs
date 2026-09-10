// Path-routing and save/load error-path tests. Included into `config::tests`
// (see `config.rs`).

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
fn save_and_load_last_cast_receiver_round_trips() {
    let _guard = TestStateDirGuard::new();

    assert!(save_last_cast_receiver(Some("device-1")).is_ok());

    assert_eq!(
        load_last_cast_receiver().unwrap(),
        Some("device-1".to_string())
    );
}

#[test]
fn load_last_cast_receiver_returns_none_when_no_file_exists() {
    let _guard = TestStateDirGuard::new();
    assert_eq!(load_last_cast_receiver().unwrap(), None);
}

#[test]
fn save_last_cast_receiver_none_clears_a_previously_saved_record() {
    let _guard = TestStateDirGuard::new();
    assert!(save_last_cast_receiver(Some("device-1")).is_ok());

    assert!(save_last_cast_receiver(None).is_ok());

    assert_eq!(load_last_cast_receiver().unwrap(), None);
}

#[test]
fn save_config_settings_reports_rename_failure_with_path() {
    let dir = std::env::temp_dir().join(format!("mbv-save-config-error-{}", uuid::Uuid::new_v4()));
    // Make the destination a non-empty directory so `rename` fails. The tmp
    // file is derived from `path`, so keeping `path` inside `dir` is what
    // keeps the cleanup below complete -- a bare directory path would put the
    // tmp file alongside it as an orphan.
    let path = dir.join("config.toml");
    std::fs::create_dir_all(&path).unwrap();
    let error = write_config_text_at(&path, "").unwrap_err();
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
        last_played_content_id: None,
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
        last_played_content_id: None,
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
