#[test]
fn service_administration_selectors_are_parsed_and_validated() {
    assert_eq!(
        parse_action(&["--connect".into(), "emby".into()]),
        Ok(Action::ConnectEmby)
    );
    assert_eq!(
        parse_action(&["--connect".into(), "abs".into()]),
        Ok(Action::ConnectAbs)
    );
    assert_eq!(
        parse_action(&["--disconnect".into(), "abs".into()]),
        Ok(Action::DisconnectAbs)
    );
    for args in [
        vec!["--connect".into()],
        vec!["--connect".into(), "audiobookshelf".into()],
        vec!["--disconnect".into()],
        vec!["--disconnect".into(), "emby".into()],
        vec!["--connect".into(), "emby".into(), "--quit".into()],
        vec![
            "--connect".into(),
            "emby".into(),
            "--export-shared-data".into(),
        ],
        vec!["--connect".into(), "emby".into(), "--audio-only".into()],
        vec!["--connect".into(), "abs".into(), "--audio-only".into()],
        vec!["--disconnect".into(), "abs".into(), "--quit".into()],
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
fn abs_diagnostics_classify_auth_rejection_and_other_failures() {
    use mbv_core::audiobookshelf::{AudiobookshelfError, AudiobookshelfFailureClass};

    let auth = classified_abs_error(&AudiobookshelfError {
        class: AudiobookshelfFailureClass::AuthenticationRejected,
    });
    assert_eq!(auth, "mbvd: Audiobookshelf authentication rejected");

    for class in [
        AudiobookshelfFailureClass::Connectivity,
        AudiobookshelfFailureClass::Server,
        AudiobookshelfFailureClass::Protocol,
        AudiobookshelfFailureClass::MalformedResponse,
        AudiobookshelfFailureClass::Unavailable,
    ] {
        let other = classified_abs_error(&AudiobookshelfError { class });
        assert_eq!(
            other,
            "mbvd: Audiobookshelf server unavailable or returned an invalid response"
        );
    }
}

#[test]
fn connect_abs_rejects_non_interactive_terminal_without_touching_state() {
    // In the test harness stdin/stdout are not terminals, so the command
    // rejects up front. Guard on interactivity so this never hangs if a
    // developer runs the suite from a real TTY.
    if interactive_terminal() {
        return;
    }
    let error = connect_abs().unwrap_err();
    assert!(
        error.contains("requires an interactive terminal"),
        "got: {error}"
    );
}

#[test]
fn disconnect_abs_rejects_non_interactive_terminal_without_touching_state() {
    if interactive_terminal() {
        return;
    }
    let error = disconnect_abs().unwrap_err();
    assert!(
        error.contains("requires an interactive terminal"),
        "got: {error}"
    );
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
        exit_code_for_error("mbvd: unsupported Service; supported Services: emby, abs"),
        2
    );
    assert_eq!(
        exit_code_for_error(
            "mbvd: restart required (live setup rejected); the running process may retain the deleted key in memory"
        ),
        3
    );
    assert_eq!(
        exit_code_for_error("mbvd: Audiobookshelf authentication rejected"),
        1
    );
    assert_eq!(
        exit_code_for_error("mbvd: unsupported Service; supported Services: abs"),
        2
    );
    assert_eq!(
        exit_code_for_error("mbvd: --disconnect abs requires an interactive terminal"),
        2
    );
    assert_eq!(
        exit_code_for_error("mbvd: restart required (live setup rejected: RevisionMismatch)"),
        3
    );
}
