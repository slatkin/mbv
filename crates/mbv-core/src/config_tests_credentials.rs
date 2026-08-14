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
fn concurrent_control_credential_creation_converges_on_persisted_winner() {
    let _env_guard = SYS_ENV_LOCK.lock().unwrap();
    let old_state_home = std::env::var_os("XDG_STATE_HOME");
    let old_system = std::env::var_os("MBV_SYSTEM");
    let temp = std::env::temp_dir().join(format!("mbv-control-race-{}", uuid::Uuid::new_v4()));
    std::env::set_var("XDG_STATE_HOME", &temp);
    std::env::remove_var("MBV_SYSTEM");

    let start = std::sync::Arc::new(std::sync::Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                load_or_create_control_credential()
            })
        })
        .collect();
    start.wait();
    let credentials: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap().unwrap())
        .collect();

    let path = control_credential_path();
    assert_eq!(credentials[0], credentials[1]);
    assert!(path.is_file());
    assert_eq!(
        load_control_credential().as_deref(),
        Some(credentials[0].as_str())
    );

    clear_control_credential_result().unwrap();
    let _ = std::fs::remove_dir_all(&temp);
    match old_state_home {
        Some(value) => std::env::set_var("XDG_STATE_HOME", value),
        None => std::env::remove_var("XDG_STATE_HOME"),
    }
    match old_system {
        Some(value) => std::env::set_var("MBV_SYSTEM", value),
        None => std::env::remove_var("MBV_SYSTEM"),
    }
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
