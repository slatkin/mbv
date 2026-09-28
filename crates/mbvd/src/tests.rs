use super::*;

#[test]
fn log_level_flag_accepts_supported_values_and_rejects_invalid_values() {
    for (value, expected) in [
        ("error", applog::Level::Error),
        ("warn", applog::Level::Warn),
        ("info", applog::Level::Info),
        ("debug", applog::Level::Debug),
    ] {
        assert_eq!(
            parse_action(&["--log-level".into(), value.into()]).unwrap(),
            Action::Serve {
                audio_only: false,
                log_level: expected,
            }
        );
    }
    parse_action(&["--log-level".into(), "trace".into()]).unwrap_err();
    parse_action(&["--log-level".into()]).unwrap_err();
}

#[test]
fn service_administration_selectors_are_parsed_and_validated() {
    assert_eq!(
        parse_action(&["--connect".into(), "emby".into()]).unwrap(),
        Action::ConnectEmby
    );
    assert_eq!(
        parse_action(&["--connect".into(), "abs".into()]).unwrap(),
        Action::ConnectAbs
    );
    assert_eq!(
        parse_action(&["--disconnect".into(), "abs".into()]).unwrap(),
        Action::DisconnectAbs
    );
    for args in [
        vec!["--connect".into()],
        vec!["--connect".into(), "audiobookshelf".into()],
        vec!["--disconnect".into()],
        vec!["--disconnect".into(), "emby".into()],
        vec!["--connect".into(), "emby".into(), "--quit".into()],
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
    use mbv_emby::{EmbyError, EmbyFailure, EmbyFailureClass};

    let raw =
        "401 username=alice password=hunter2 token=secret-token user_id=user-7 {\"raw\":true}";
    let error = EmbyError::from(EmbyFailure {
        class: EmbyFailureClass::AuthenticationRejected,
        message: raw.into(),
    });
    let diagnostic = DaemonError::from(error).to_string();
    assert_eq!(diagnostic, "mbvd: Emby authentication rejected");
    for secret in ["alice", "hunter2", "secret-token", "user-7", "raw"] {
        assert!(!diagnostic.contains(secret), "diagnostic leaked {secret}");
    }
}

#[test]
fn abs_diagnostics_classify_auth_rejection_and_other_failures() {
    use mbv_audiobookshelf::AudiobookshelfFailureClass;

    let auth = classified_abs_error(&mbv_audiobookshelf::AudiobookshelfError::from_class(
        AudiobookshelfFailureClass::AuthenticationRejected,
    ));
    assert_eq!(auth, "mbvd: Audiobookshelf authentication rejected");

    // One representative of the single fallback branch: every non-auth class
    // maps to the same message (Persistence included, added with its kind).
    let other = classified_abs_error(&mbv_audiobookshelf::AudiobookshelfError::from_class(
        AudiobookshelfFailureClass::Persistence,
    ));
    assert_eq!(
        other,
        "mbvd: Audiobookshelf server unavailable or returned an invalid response"
    );
}

#[test]
fn connect_emby_rejects_non_interactive_terminal_without_touching_state() {
    if interactive_terminal() {
        return;
    }
    let error = connect_emby().unwrap_err();
    assert!(error.is_usage_error());
    assert_eq!(
        error.to_string(),
        "mbvd: --connect emby requires an interactive terminal"
    );
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
    assert!(error.is_usage_error());
    assert_eq!(
        error.to_string(),
        "mbvd: --connect abs requires an interactive terminal"
    );
}

#[test]
fn disconnect_abs_rejects_non_interactive_terminal_without_touching_state() {
    if interactive_terminal() {
        return;
    }
    let error = disconnect_abs().unwrap_err();
    assert!(error.is_usage_error());
    assert_eq!(
        error.to_string(),
        "mbvd: --disconnect abs requires an interactive terminal"
    );
}

#[test]
fn reconcile_outcome_distinguishes_applied_rejected_and_unrelated_events() {
    use mbv_ctrl::{CtrlEvent, ServiceSetupRejection};

    assert!(matches!(
        reconcile_event_outcome(&CtrlEvent::ServiceSetupApplied {
            kind: mbv_queue::ServiceKind::Emby,
            revision: 1,
        }),
        Some(Ok(()))
    ));
    let rejected = reconcile_event_outcome(&CtrlEvent::ServiceSetupRejected {
        kind: mbv_queue::ServiceKind::Emby,
        revision: 1,
        reason: ServiceSetupRejection::RevisionMismatch,
    })
    .unwrap()
    .unwrap_err();
    assert!(rejected.is_restart_required());
    assert_eq!(
        rejected.to_string(),
        "mbvd: restart required (live setup rejected: RevisionMismatch)"
    );
    assert!(
        reconcile_event_outcome(&CtrlEvent::StatusOnly(
            mbv_ctrl::player::PlayerStatus::default()
        ))
        .is_none()
    );
}

fn assert_exit_code(error: &DaemonError, message: &str, code: i32) {
    assert_eq!(error.to_string(), message);
    assert_eq!(
        error.kind_name(),
        match code {
            3 => "mbvd.restart_required",
            2 => "mbvd.usage",
            _ => "mbvd.failure",
        }
    );
    assert_eq!(exit_code_for_error(error), code);
}

#[test]
fn exit_codes_derive_from_kinds_and_preserve_matched_messages() {
    assert_exit_code(
        &DaemonError::restart_required("mbvd: restart required (ctrl unavailable)"),
        "mbvd: restart required (ctrl unavailable)",
        3,
    );
    assert_exit_code(
        &DaemonError::failure("mbvd: Emby authentication rejected"),
        "mbvd: Emby authentication rejected",
        1,
    );
    assert_exit_code(
        &DaemonError::usage("mbvd: requires an interactive terminal"),
        "mbvd: requires an interactive terminal",
        2,
    );
    assert_exit_code(
        &DaemonError::usage("mbvd: unsupported Service; supported Services: emby, abs"),
        "mbvd: unsupported Service; supported Services: emby, abs",
        2,
    );
    assert_exit_code(
        &DaemonError::restart_required(
            "mbvd: restart required (live setup rejected); the running process may retain the deleted key in memory",
        ),
        "mbvd: restart required (live setup rejected); the running process may retain the deleted key in memory",
        3,
    );
    assert_exit_code(
        &DaemonError::failure("mbvd: Audiobookshelf authentication rejected"),
        "mbvd: Audiobookshelf authentication rejected",
        1,
    );
    assert_exit_code(
        &DaemonError::usage("mbvd: unsupported Service; supported Services: abs"),
        "mbvd: unsupported Service; supported Services: abs",
        2,
    );
    assert_exit_code(
        &DaemonError::usage("mbvd: --disconnect abs requires an interactive terminal"),
        "mbvd: --disconnect abs requires an interactive terminal",
        2,
    );
    assert_exit_code(
        &DaemonError::restart_required(
            "mbvd: restart required (live setup rejected: RevisionMismatch)",
        ),
        "mbvd: restart required (live setup rejected: RevisionMismatch)",
        3,
    );
}
