//! The Client Hello's Pin-swap advertisement.

use super::*;

/// The Hello advertisement follows the connect's Pin-swap argument
/// (`extract-client-core` EDGE-1 fix): a Client that takes no part in Pin swap
/// (`None` — the GUI Client, design D9) sends a plain Hello the Owner never
/// picks as a swap target, and a participating Client (`Some` — the TUI)
/// advertises `pin-swap`. The Owner's classification of those capabilities
/// into a swap surface is the daemon's `swap_surface_from_hello` contract,
/// already pinned there.
#[rstest]
#[case::no_pin_swap(None, false)]
#[case::pin_swap(Some(&crate::PinSwapHello::default()), true)]
fn the_client_hello_pin_swap_advertisement_follows_the_connect_argument(
    #[case] pin_swap: Option<&crate::PinSwapHello>,
    #[case] advertised: bool,
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
        pin_swap,
    )
    .unwrap();
    peer.join().unwrap();

    let client_hello = hello_rx.recv().unwrap();
    assert_eq!(
        client_hello.contains(mbv_ctrl::CTRL_CAP_PIN_SWAP),
        advertised,
        "the Hello's pin-swap capability must follow the connect's Pin-swap argument"
    );
}
