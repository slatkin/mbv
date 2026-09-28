#[cfg(test)]
use crate::{
    Config, ConfigSection, DEFAULT_SYSTEM_DAEMON_TCP_LISTEN, LastRemoteConnection,
    TestStateDirGuard, cache_dir, config_path, control_socket_path, data_dir_system_or_local,
    home_latest_launch_path, is_system_instance, load_home_latest_launch,
    load_last_remote_connection, load_last_remote_connection_at, load_queue_state, mpv_ipc_path,
    parse_config, queue_state_path, save_config_section_at, save_config_settings_at,
    save_home_latest_launch, save_last_remote_connection, save_last_remote_connection_at,
    save_queue_state, write_config_text_at,
};
#[cfg(test)]
use mbv_queue::{QueueSource, QueueState};
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
// `SYS_ENV_LOCK` (via `crate::tests::SYS_ENV_LOCK`) rather than
// defining their own — independent per-file mutexes don't exclude each
// other and previously caused flaky cross-test env-var races (e.g. one
// test's queue_state.json read intermittently coming back empty because
// an unrelated, unguarded HOSTNAME mutation in api.rs raced it).
use std::sync::Mutex;
pub static SYS_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn home_latest_launch_state_missing_or_malformed_has_no_baseline_and_round_trips() {
    let _guard = TestStateDirGuard::new();
    let path = home_latest_launch_path();
    assert_eq!(load_home_latest_launch(), None);

    std::fs::write(&path, "not json").unwrap();
    assert_eq!(load_home_latest_launch(), None);
    std::fs::write(&path, r#"{"version":99,"launch_secs":1234}"#).unwrap();
    assert_eq!(load_home_latest_launch(), None);

    save_home_latest_launch(1234).unwrap();
    assert_eq!(load_home_latest_launch(), Some(1234));
    save_home_latest_launch(5678).unwrap();
    assert_eq!(load_home_latest_launch(), Some(5678));
}

#[test]
fn home_latest_launch_rejects_zero_timestamp_as_launch_error() {
    let error = crate::save_home_latest_launch(0).unwrap_err();

    assert!(error.is_launch());
}

#[test]
fn is_system_instance_false_without_env_var() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    crate::remove_test_env_var("MBV_SYSTEM");
    assert!(!is_system_instance());
}

#[test]
fn is_system_instance_true_with_env_var() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    crate::set_test_env_var("MBV_SYSTEM", "1");
    let result = is_system_instance();
    crate::remove_test_env_var("MBV_SYSTEM");
    assert!(result);
}

#[test]
fn cache_dir_uses_system_path_when_mbv_system_set() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    crate::set_test_env_var("MBV_SYSTEM", "1");
    let path = cache_dir();
    crate::remove_test_env_var("MBV_SYSTEM");
    assert_eq!(path, std::path::PathBuf::from("/var/cache/mbv"));
}

#[test]
fn cache_dir_uses_xdg_when_not_system() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    crate::remove_test_env_var("MBV_SYSTEM");
    crate::set_test_env_var("XDG_CACHE_HOME", "/tmp/xdg-test-cache");
    let path = cache_dir();
    crate::remove_test_env_var("XDG_CACHE_HOME");
    assert_eq!(path, std::path::PathBuf::from("/tmp/xdg-test-cache/mbv"));
}

#[test]
fn data_dir_system_or_local_uses_system_path_when_mbv_system_set() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    crate::set_test_env_var("MBV_SYSTEM", "1");
    let path = data_dir_system_or_local();
    crate::remove_test_env_var("MBV_SYSTEM");
    assert_eq!(path, std::path::PathBuf::from("/var/lib/mbv"));
}

#[test]
fn config_path_uses_system_path_when_mbv_system_set() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    crate::set_test_env_var("MBV_SYSTEM", "1");
    let path = config_path();
    crate::remove_test_env_var("MBV_SYSTEM");
    assert_eq!(path, std::path::PathBuf::from("/etc/mbv/config.toml"));
}

#[test]
fn mpv_ipc_path_uses_run_dir_when_mbv_system_set() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    crate::set_test_env_var("MBV_SYSTEM", "1");
    let path = mpv_ipc_path();
    crate::remove_test_env_var("MBV_SYSTEM");
    assert_eq!(path, "/run/mbv/mbv-mpv.sock");
}

#[test]
fn control_socket_path_uses_run_dir_when_mbv_system_set() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    crate::set_test_env_var("MBV_SYSTEM", "1");
    let path = control_socket_path();
    crate::remove_test_env_var("MBV_SYSTEM");
    assert_eq!(path, "/run/mbv/mbv-ctrl.sock");
}

#[test]
fn daemon_server_tcp_listen_defaults_for_system_instance() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    crate::set_test_env_var("MBV_SYSTEM", "1");
    let cfg = parse_config("[server]\nurl = \"http://host\"").unwrap();
    crate::remove_test_env_var("MBV_SYSTEM");
    assert_eq!(
        cfg.daemon_server_tcp_listen,
        DEFAULT_SYSTEM_DAEMON_TCP_LISTEN
    );
}

#[test]
fn mpv_ipc_path_uses_xdg_runtime_dir_when_not_system() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    crate::remove_test_env_var("MBV_SYSTEM");
    crate::set_test_env_var("XDG_RUNTIME_DIR", "/run/user/1000");
    let path = mpv_ipc_path();
    crate::remove_test_env_var("XDG_RUNTIME_DIR");
    assert_eq!(path, "/run/user/1000/mbv-mpv.sock");
}

#[test]
fn save_and_load_last_remote_connection_round_trips_library_route() {
    let _guard = TestStateDirGuard::new();
    let conn = LastRemoteConnection::LibraryRoute {
        library: "music".to_string(),
    };

    save_last_remote_connection(Some(&conn)).unwrap();

    assert_eq!(load_last_remote_connection().unwrap(), Some(conn));
}

#[test]
fn save_and_load_last_remote_connection_round_trips_direct_session() {
    let _guard = TestStateDirGuard::new();
    let conn = LastRemoteConnection::DirectSession {
        device_name: "living-room-mbv".to_string(),
    };

    save_last_remote_connection(Some(&conn)).unwrap();

    assert_eq!(load_last_remote_connection().unwrap(), Some(conn));
}

#[test]
fn save_last_remote_connection_none_clears_a_previously_saved_record() {
    let _guard = TestStateDirGuard::new();
    save_last_remote_connection(Some(&LastRemoteConnection::LibraryRoute {
        library: "music".to_string(),
    }))
    .unwrap();

    save_last_remote_connection(None).unwrap();

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
    let error = save_last_remote_connection_at(&dir, None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("remove"));
    assert!(error.contains(dir.to_str().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn load_last_remote_connection_reports_read_failure_with_path() {
    let dir = std::env::temp_dir().join(format!("mbv-load-state-error-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let error = load_last_remote_connection_at(&dir).unwrap_err();
    assert!(error.is_state());
    let error = error.to_string();
    assert!(error.starts_with("read "));
    assert!(error.contains(dir.to_str().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
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
    assert!(error.is_save());
    let error = error.to_string();
    assert!(error.contains("rename"));
    assert!(error.contains(dir.to_str().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_config_settings_reports_read_failure_with_path() {
    let dir = std::env::temp_dir().join(format!("mbv-read-config-error-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let error = save_config_settings_at(&Config::default(), &dir).unwrap_err();
    assert!(error.is_save());
    let error = error.to_string();
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
    assert!(error.is_save());
    let error = error.to_string();
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
    let error = save_config_settings_at(&Config::default(), &path)
        .unwrap_err()
        .to_string();
    assert!(error.contains("write"));
    assert!(error.contains(path.with_extension("toml.tmp").to_str().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}

// A section-scoped save must not erase sections another concurrently running
// client wrote after this client loaded its snapshot (stay-alive keeps a
// background client alive next to the foreground one). Regression coverage
// for the multi-client config.toml clobbering bug: a full rewrite from a
// stale snapshot removed `[audiobookshelf]` and reset `[queue]`.
#[test]
fn save_config_section_preserves_sections_written_by_another_client() {
    let dir = std::env::temp_dir().join(format!("mbv-section-save-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        "[audiobookshelf]\nurl = \"http://abs.example\"\nrevision = 3\n\n[queue]\nalways_play_next = true\n",
    )
    .unwrap();
    let cfg = Config {
        subtitle_mode: "Always".to_string(),
        ..Default::default()
    };
    save_config_section_at(&cfg, &path, ConfigSection::Playback).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(
        saved.contains("[audiobookshelf]"),
        "lost foreign section: {saved}"
    );
    assert!(
        saved.contains("always_play_next = true"),
        "clobbered foreign queue section: {saved}"
    );
    assert!(
        saved.contains("subtitle_mode = \"Always\""),
        "missing own write: {saved}"
    );
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
        positions: std::collections::HashMap::default(),
    };

    save_queue_state(&original).unwrap();

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
        positions: std::collections::HashMap::default(),
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
