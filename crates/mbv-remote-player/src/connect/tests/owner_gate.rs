//! The local Owner gate on the server hello (invariant 15): a version or
//! wire-identity mismatch refuses before the Client sends its hello, and a
//! build-identity difference alone never refuses.

use rstest::rstest;

use super::*;

// Issue #559 (design D1) and issue #949 item 3, with the wire-identity gate
// (ctrl-protocol delta, "A local Client never attaches to an Owner with a
// different version or wire"): the gate fires on the Owner's hello, before
// the Client sends its own hello or control credential. Each case pins the
// refusal and the prompt text the scenario produces.
#[rstest]
#[case::a_different_version(
    "different-owner-version",
    mbv_ctrl::wire_id(),
    "version different-owner-version"
)]
#[case::a_different_wire_identity(
    env!("CARGO_PKG_VERSION"),
    "a-different-wire-id",
    "wire a-different-wire-id"
)]
#[case::an_owner_without_a_wire_identity(env!("CARGO_PKG_VERSION"), "", "wire unknown")]
fn local_handshake_rejects_a_mismatched_owner_before_client_hello(
    #[case] owner_version: &str,
    #[case] owner_wire_id: &str,
    #[case] message_fragment: &str,
) {
    let (client, daemon) = UnixStream::pair().unwrap();
    let (received_tx, received_rx) = mpsc::channel();
    let owner_version = owner_version.to_string();
    let owner_wire_id = owner_wire_id.to_string();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let mut hello = CtrlHello::current();
        hello.app_version = owner_version;
        hello.wire_id = owner_wire_id;
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(hello)).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        received_tx.send(client_hello).unwrap();
    });

    let error = perform_handshake_with_role(
        SocketStream::Unix(client),
        || Ok("unused".to_string()),
        HandshakeRole::Client,
        PeerBuild::MustMatch,
        Some(&crate::PinSwapHello::default()),
    )
    .unwrap_err();

    assert!(error.is_owner_mismatch());
    assert!(
        error.to_string().contains(message_fragment),
        "the mismatch message names the Owner's identity: {error}"
    );
    let client = BuildInfo::current();
    assert!(
        error.to_string().contains(&format!(
            "but this terminal is version {} (build {}, wire {})",
            client.app_version, client.build_id, client.wire_id
        )),
        "the mismatch message names this Client's identity: {error}"
    );
    peer.join().unwrap();
    assert_eq!(received_rx.recv().unwrap(), "");
}

// Issue #949 item 3: a build-identity difference alone never refuses. The
// Owner's build identity is kept for Settings, so `owner_build()` returns the
// Owner's values as its hello reported them.
#[test]
fn local_handshake_accepts_an_owner_with_a_different_build_id() {
    let (client, daemon) = UnixStream::pair().unwrap();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        let mut hello = CtrlHello::current();
        hello.build_id = "owner-build-other".to_string();
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(hello)).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
                status: PlayerStatus::default(),
                slots: Vec::new(),
                active_slot: None,
                revision: 0,
                source: QueueSource::Unknown,
                lineage: mbv_queue::QueueLineage::default(),
                in_flight_transition: None,
                queued_latest_transition: None,
            }))
            .unwrap()
        )
        .unwrap();
    });

    let (remote, _events) = connect_stream(
        SocketStream::Unix(client),
        PeerBuild::MustMatch,
        Some(&crate::PinSwapHello::default()),
    )
    .unwrap();
    peer.join().unwrap();

    let expected = BuildInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        build_id: "owner-build-other".to_string(),
        wire_id: mbv_ctrl::wire_id().to_string(),
    };
    assert_eq!(remote.owner_build(), &expected);
}
