//! The Client Hello's claims: the Pin-swap advertisement, the swap token, and
//! `named-queues`.

use super::*;

/// A swap token no other Hello field can contain.
const OPEN_LAUNCH_TOKEN: &str = "open-launch-token-7f3a";

/// The Hello advertisement follows the connect's claims
/// (`extract-client-core` EDGE-1 fix; gui-named-queues design D7): the TUI
/// Client advertises `pin-swap` and never `named-queues`, and the GUI Client's
/// local link advertises `named-queues` and sends a plain Hello the Owner
/// never picks as a swap target. The swap token goes out with or without the
/// Pin-swap claim, so the GUI Client the Owner starts for Open completes that
/// launch (tray Open starts the GUI Client). The Owner's classification of the swap
/// capabilities into a swap surface is the daemon's `swap_surface_from_hello`
/// contract, already pinned there; its `named-queues` gate is the daemon's
/// `broadcast_named_queue_event`.
#[rstest]
#[case::tui_client(
    crate::ClientHelloClaims {
        pin_swap: Some(crate::PinSwapHello::default()),
        named_queues: false,
        swap_token: None,
    },
    (true, false, false)
)]
#[case::gui_local_link(
    crate::ClientHelloClaims {
        pin_swap: None,
        named_queues: true,
        swap_token: None,
    },
    (false, true, false)
)]
#[case::gui_open_launch(
    crate::ClientHelloClaims {
        pin_swap: None,
        named_queues: true,
        swap_token: Some(OPEN_LAUNCH_TOKEN.to_string()),
    },
    (false, true, true)
)]
fn the_client_hello_advertisement_follows_the_connect_claims(
    #[case] claims: crate::ClientHelloClaims,
    #[case] advertised: (bool, bool, bool),
) {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let (client, daemon) = UnixStream::pair().unwrap();
    let (hello_tx, hello_rx) = mpsc::channel::<String>();
    let peer = std::thread::spawn(move || {
        let mut writer = daemon.try_clone().unwrap();
        let mut reader = BufReader::new(daemon);
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::Hello(CtrlHello::current())).unwrap()
        )
        .unwrap();
        let mut client_hello = String::new();
        reader.read_line(&mut client_hello).unwrap();
        hello_tx.send(client_hello).unwrap();
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&CtrlEvent::UnifiedQueueState(UnifiedQueueStateData {
                queue: None,
                start_slot: None,
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

    perform_handshake_with_role(
        SocketStream::Unix(client),
        || Ok("unused".to_string()),
        HandshakeRole::Client,
        PeerBuild::MustMatch,
        &claims,
    )
    .unwrap();
    peer.join().unwrap();

    let client_hello = hello_rx.recv().unwrap();
    let (pin_swap, named_queues, swap_token) = advertised;
    assert_eq!(
        client_hello.contains(mbv_ctrl::CTRL_CAP_PIN_SWAP),
        pin_swap,
        "the Hello's pin-swap capability must follow the connect's pin-swap claim"
    );
    assert_eq!(
        client_hello.contains(mbv_ctrl::CTRL_CAP_NAMED_QUEUES),
        named_queues,
        "the Hello's named-queues capability must follow the connect's named-queues claim"
    );
    assert_eq!(
        client_hello.contains(OPEN_LAUNCH_TOKEN),
        swap_token,
        "the Hello's swap token must follow the connect's swap-token claim"
    );
}
