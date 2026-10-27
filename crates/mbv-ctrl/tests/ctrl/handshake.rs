use super::*;

// emby-session-output-followups task 4.1: `build_id` is additive (design
// D2 keeps CTRL_PROTOCOL_VERSION at 10), so it must round-trip and a peer
// that predates the field must decode an empty value.
#[test]
fn hello_build_id_round_trips_and_a_missing_field_decodes_empty() {
    let hello = CtrlHello::current();
    assert_eq!(hello.build_id, BUILD_ID);

    let wire = serde_json::to_string(&hello).unwrap();
    let decoded: CtrlHello = serde_json::from_str(&wire).unwrap();
    assert_eq!(decoded.build_id, BUILD_ID);

    let stale: CtrlHello =
        serde_json::from_str(r#"{"protocol_version":10,"app_version":"0.0.0","capabilities":[]}"#)
            .unwrap();
    assert_eq!(stale.build_id, "");
}

#[test]
fn current_hello_validates() {
    CtrlHello::current().validate_peer().unwrap();
    assert_eq!(CtrlHello::current().protocol_version, CTRL_PROTOCOL_VERSION);
}

#[test]
fn hello_rejects_incompatible_protocol_version() {
    let mut hello = CtrlHello::current();
    hello.protocol_version += 1;
    assert!(hello.validate_peer().is_err());
}

#[test]
fn hello_rejects_missing_capability() {
    let mut hello = CtrlHello::current();
    hello.capabilities.retain(|cap| cap != CTRL_CAP_START_INDEX);
    assert!(hello.validate_peer().is_err());
}

// queue-content-shuffle task 2.2: the queue-shuffle capability is additive,
// so the current hello advertises it and a peer without it reads false.
#[test]
fn peer_without_queue_shuffle_capability_reads_false() {
    assert!(CtrlHello::current().supports_queue_shuffle());

    let mut peer = CtrlHello::current();
    peer.capabilities
        .retain(|cap| cap != CTRL_CAP_QUEUE_SHUFFLE);
    assert!(!peer.supports_queue_shuffle());
    peer.validate_peer().unwrap();
}

#[test]
fn current_hello_has_no_service_credential_field() {
    let json = serde_json::to_string(&CtrlHello::current()).unwrap();
    assert!(!json.contains("auth_token"));
    assert!(!json.contains("token-123"));
}

#[test]
fn service_setup_reconciliation_wire_has_only_kind_and_revision() {
    let json = serde_json::to_string(&CtrlCmd::ApplyServiceSetup {
        kind: mbv_queue::ServiceKind::Emby,
        revision: 42,
    })
    .unwrap();
    assert_eq!(
        json,
        r#"{"ApplyServiceSetup":{"kind":"Emby","revision":42}}"#
    );
    assert!(!json.contains("setup"));
    assert!(!json.contains("token"));
    assert!(!json.contains("credential"));
}

#[test]
fn service_setup_reconciliation_responses_round_trip() {
    let applied = CtrlEvent::ServiceSetupApplied {
        kind: mbv_queue::ServiceKind::Emby,
        revision: 7,
    };
    let rejected = CtrlEvent::ServiceSetupRejected {
        kind: mbv_queue::ServiceKind::Emby,
        revision: 7,
        reason: ServiceSetupRejection::RevisionMismatch,
    };
    assert!(matches!(
        serde_json::from_str::<CtrlEvent>(&serde_json::to_string(&applied).unwrap()).unwrap(),
        CtrlEvent::ServiceSetupApplied { .. }
    ));
    assert!(matches!(
        serde_json::from_str::<CtrlEvent>(&serde_json::to_string(&rejected).unwrap()).unwrap(),
        CtrlEvent::ServiceSetupRejected {
            reason: ServiceSetupRejection::RevisionMismatch,
            ..
        }
    ));
}

#[test]
fn capable_client_hello_uses_control_credential_field() {
    let hello = CtrlHello::current_control_client("control-123".into());
    assert!(hello.supports_control_auth());
    assert_eq!(hello.control_token.as_deref(), Some("control-123"));
}

// tray-pin-swap 1.1: older Clients never send `swap_token`; the field must
// stay optional so they remain admissible.
#[test]
fn hello_without_swap_token_parses_to_none() {
    let mut json = serde_json::to_value(CtrlHello::current()).unwrap();
    json.as_object_mut()
        .unwrap()
        .remove("swap_token")
        .expect("current hello serializes swap_token");
    let hello: CtrlHello = serde_json::from_value(json).unwrap();
    assert_eq!(hello.swap_token, None);
}

#[test]
fn pin_swap_builder_advertises_swap_capabilities_and_token() {
    let hello = CtrlHello::current_control_client("control-123".into())
        .with_pin_swap(true, Some("t".into()));
    assert!(hello.capabilities.iter().any(|c| c == CTRL_CAP_PIN_SWAP));
    assert!(
        hello
            .capabilities
            .iter()
            .any(|c| c == CTRL_CAP_PINNED_SURFACE)
    );
    assert_eq!(hello.swap_token.as_deref(), Some("t"));
}

// tray-pin-swap 1.3: each Owner action has exactly one CLI flag (spec
// `owner-actions`).
#[test]
fn owner_action_cli_flags_round_trip() {
    for action in OwnerAction::ALL {
        assert_eq!(OwnerAction::from_cli_flag(action.cli_flag()), Some(*action));
    }
}

#[test]
fn owner_action_hello_advertises_owner_action_capability() {
    let hello = CtrlHello::current_owner_action("control-123".into());
    assert!(hello.supports_owner_action());
    assert!(!hello.supports_service_setup_admin());
}

#[test]
fn audio_only_capability_is_optional_and_unknown_capabilities_are_accepted() {
    let mut hello = CtrlHello::current();
    assert!(!hello.supports_audio_only());
    hello.capabilities.push(CTRL_CAP_AUDIO_ONLY.to_string());
    hello.capabilities.push("future-capability".to_string());
    hello.validate_peer().unwrap();
    assert!(hello.supports_audio_only());
    assert!(!hello.compatibility().unwrap().supports_audio_only);
}

#[test]
fn invalid_control_credential_is_rejected_without_emby_validation() {
    let hello = CtrlHello::current_control_client("not-the-control-secret".into());
    assert!(hello.validate_control_credential("control-secret").is_err());
}

// Guards for task 4.2: capability advertisement is static protocol support —
// it says a peer can decode the wire shapes, not that a daemon owner is
// eligible to bind or play an Audiobookshelf item (see daemon_admits in
// daemon_control_queue.rs, tested separately in daemon_tests_abs_queue.rs).

#[test]
fn hello_current_advertises_abs_capabilities() {
    let hello = CtrlHello::current();
    assert!(hello.supports_abs_queue(), "hello must advertise abs-queue");
    assert!(
        hello.supports_abs_progress(),
        "hello must advertise abs-progress"
    );
    assert!(hello.supports_owner_queue_load());
    assert!(hello.capabilities.iter().any(|c| c == CTRL_CAP_ABS_QUEUE));
    assert!(
        hello
            .capabilities
            .iter()
            .any(|c| c == CTRL_CAP_ABS_PROGRESS)
    );
}

#[test]
fn ctrl_compatibility_current_supports_abs_capabilities() {
    let compat = CtrlCompatibility::current();
    assert!(compat.audiobookshelf.queue);
    assert!(compat.audiobookshelf.progress);
}

// Guards for task 3.3: prove that the Audiobookshelf ctrl wire types contain
// exactly their expected fields — no api key, authorization header, resolved
// URL, or playback sessionId can be added without breaking these tests.

#[test]
fn audiobookshelf_progress_event_wire_fields_exact() {
    let event = AudiobookshelfProgressEvent {
        library_item_id: "li_abc".to_string(),
        episode_id: "ep_123".to_string(),
        position_ticks: 5_000_000,
        is_finished: false,
        setup_generation: 1,
    };
    let json = serde_json::to_value(&event).unwrap();
    let obj = json.as_object().unwrap();
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "episode_id",
            "is_finished",
            "library_item_id",
            "position_ticks",
            "setup_generation"
        ],
        "AudiobookshelfProgressEvent must contain exactly these wire fields — \
         adding any credential, session id, or resolved url will break this guard"
    );
}

#[test]
fn audiobookshelf_queue_item_wire_fields_exact() {
    use mbv_queue::AudiobookshelfQueueItem;
    let item = AudiobookshelfQueueItem {
        library_item_id: "li_abc".to_string(),
        episode_id: "ep_123".to_string(),
        title: "Episode 1".to_string(),
        show_title: Some("My Podcast".to_string()),
        author: Some("Author".to_string()),
        description: None,
        duration_ticks: Some(600_000_000),
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    };
    let json = serde_json::to_value(&item).unwrap();
    let obj = json.as_object().unwrap();
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "author",
            "cover_path",
            "description",
            "duration_ticks",
            "episodeId",
            "is_finished",
            "libraryItemId",
            "played",
            "position_ticks",
            "pub_date_secs",
            "show_title",
            "title"
        ],
        "AudiobookshelfQueueItem must contain exactly these wire fields — \
         adding any credential, session id, or resolved url will break this guard"
    );
}
