use std::sync::mpsc;
use std::time::Duration;

use super::ClientWaker;
use crate::app::waking_relay::spawn_waking_relay;

// A bounded wait for the relay worker to forward one item and fire the
// waker; never a sleep, so wiring regressions fail instead of hanging CI.
const RELAY_TIMEOUT: Duration = Duration::from_millis(500);

// The waker contract (design D7): a send onto an Owner-snapshot link must
// reach the drain in order and wake the Client after it; a dropped
// upstream still reads Disconnected.
#[test]
fn relay_forwards_in_order_wakes_and_disconnects() {
    let (wake_tx, wake_rx) = mpsc::channel();
    let waker = ClientWaker::new();
    waker.install(move || {
        let _ = wake_tx.send(());
    });
    let (tx, upstream) = mpsc::channel();
    let relayed = spawn_waking_relay(upstream, waker);

    tx.send(1).unwrap();
    tx.send(2).unwrap();
    // The waker's own send lands only after the relay forwarded the item,
    // so the wake signal never races the delivery.
    wake_rx
        .recv_timeout(RELAY_TIMEOUT)
        .expect("the waker must fire after the first forwarded item");
    assert_eq!(relayed.recv().unwrap(), 1);
    assert_eq!(relayed.recv().unwrap(), 2);

    drop(tx);
    relayed.recv().unwrap_err();
}
