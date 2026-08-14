#[test]
fn connect_action_is_the_only_service_administration_selector() {
    assert_eq!(
        parse_action(&["--connect".into(), "emby".into()]),
        Ok(Action::ConnectEmby)
    );
    for args in [
        vec!["--connect".into()],
        vec!["--connect".into(), "audiobookshelf".into()],
        vec!["--connect".into(), "emby".into(), "--quit".into()],
        vec![
            "--connect".into(),
            "emby".into(),
            "--export-shared-data".into(),
        ],
        vec!["--connect".into(), "emby".into(), "--audio-only".into()],
    ] {
        assert!(
            parse_action(&args).is_err(),
            "accepted invalid args: {args:?}"
        );
    }
}

#[test]
fn connect_diagnostics_redact_candidate_and_remote_material() {
    let raw =
        "401 username=alice password=hunter2 token=secret-token user_id=user-7 {\"raw\":true}";
    let diagnostic = classified_auth_error(raw);
    assert_eq!(diagnostic, "mbvd: Emby authentication rejected");
    for secret in ["alice", "hunter2", "secret-token", "user-7", "raw"] {
        assert!(!diagnostic.contains(secret), "diagnostic leaked {secret}");
    }
}

#[test]
fn exit_codes_are_stable_for_usage_validation_and_restart_outcomes() {
    assert_eq!(
        exit_code_for_error("mbvd: restart required (ctrl unavailable)"),
        3
    );
    assert_eq!(exit_code_for_error("mbvd: Emby authentication rejected"), 1);
    assert_eq!(
        exit_code_for_error("mbvd: requires an interactive terminal"),
        2
    );
    assert_eq!(
        exit_code_for_error("mbvd: unsupported Service; supported Services: emby"),
        2
    );
}
