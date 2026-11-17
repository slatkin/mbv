//! Ctrl accept-path behaviour: Hello classification, admission, and the
//! per-connection seeds a newly-attaching Client receives.

use super::*;
use crate::ctrl::CtrlClients;

/// Contract: spec pin-swap "Attached pinned Client does not support Pin
/// swap" — a pinned-surface-only Hello (a pre-upgrade pinned Client) is
/// a Pinned swap target, so the shared state flips and Unpin targets it;
/// the swap then abandons at the deadline. A Terminal surface still
/// requires `pin-swap`, and a Client with neither capability has no
/// surface.
#[test]
fn a_pinned_surface_only_hello_is_a_pinned_swap_target() {
    let pinned_only = hello_with_caps(&[mbv_ctrl::CTRL_CAP_PINNED_SURFACE]);
    let pinned_and_swap = hello_with_caps(&[
        mbv_ctrl::CTRL_CAP_PIN_SWAP,
        mbv_ctrl::CTRL_CAP_PINNED_SURFACE,
    ]);
    let swap_only = hello_with_caps(&[mbv_ctrl::CTRL_CAP_PIN_SWAP]);
    let neither = hello_with_caps(&[]);

    assert_eq!(
        swap_surface_from_hello(&pinned_only),
        Some(SwapSurface::Pinned)
    );
    assert_eq!(
        swap_surface_from_hello(&pinned_and_swap),
        Some(SwapSurface::Pinned)
    );
    assert_eq!(
        swap_surface_from_hello(&swap_only),
        Some(SwapSurface::Terminal)
    );
    assert_eq!(swap_surface_from_hello(&neither), None);
}

/// A pinned-surface-only Hello registered as a Client sets the shared
/// attached-Clients state to `Pinned`, so the Tray shows **Unpin** while
/// that pre-upgrade Client is attached.
#[test]
fn a_pinned_surface_only_client_sets_the_pinned_state() {
    let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
    let state = Arc::new(mbv_ctrl::AttachedClientsCell::default());
    let mut clients = CtrlClients::new(
        merged_tx,
        Arc::clone(&state),
        Arc::new(crate::PendingSwapToken::default()),
    );
    let (client_tx, _client_rx) = mpsc::channel::<CtrlOutbound>();
    clients.connect_with_role(
        client_tx,
        CtrlTransport::Local,
        false,
        false,
        crate::ctrl::PeerHelloCapabilities::default(),
        swap_surface_from_hello(&hello_with_caps(&[mbv_ctrl::CTRL_CAP_PINNED_SURFACE])),
        CtrlConnectionRole::Client,
    );

    assert_eq!(state.load(), mbv_ctrl::AttachedClients::Pinned);
}

fn hello_with_caps(caps: &[&str]) -> CtrlHello {
    let mut hello = CtrlHello::current();
    hello.capabilities = caps.iter().map(|cap| (*cap).to_string()).collect();
    hello
}

/// A classified Hello that advertised (or did not advertise)
/// `named-queues`.
fn classified_hello(named_queues: bool) -> HelloClassification {
    HelloClassification {
        audiobookshelf: CtrlAudiobookshelfCapabilities::default(),
        capabilities: crate::ctrl::PeerHelloCapabilities {
            named_queues: crate::ctrl::NamedQueues::of(named_queues),
            ..Default::default()
        },
        role: CtrlConnectionRole::Client,
        swap_surface: None,
        swap_token: None,
    }
}

/// The one event a seed sent.
fn recv_seed(rx: &mpsc::Receiver<CtrlOutbound>) -> CtrlEvent {
    match rx.try_recv().expect("the seed was sent") {
        CtrlOutbound::Event(json) => serde_json::from_str(&json).unwrap(),
        CtrlOutbound::Flush(_) => panic!("expected a seed event"),
    }
}

/// Contract: design D3 of `gui-named-queues` (`named-queues`, "sent on
/// attach") — a Client that negotiated `named-queues` is handed the Owner's
/// list when it attaches, and a Client that did not receives nothing new.
#[test]
fn the_named_queue_list_reaches_a_client_that_attaches_with_the_capability() {
    let shared_queue = crate::tests::shared_queue_state();
    let current = mbv_queue::NamedQueueId::from_raw(1);
    shared_queue.note_named_queue_list(crate::core::NamedQueueListSeed {
        queues: vec![mbv_ctrl::NamedQueueEntry {
            id: current,
            name: "Default".to_string(),
            len: 0,
        }],
        current,
    });

    let (capable_tx, capable_rx) = mpsc::channel();
    seed_named_queue_list(&shared_queue, &classified_hello(true), &capable_tx);
    let (plain_tx, plain_rx) = mpsc::channel();
    seed_named_queue_list(&shared_queue, &classified_hello(false), &plain_tx);

    match recv_seed(&capable_rx) {
        CtrlEvent::NamedQueueList {
            queues,
            current: seed,
        } => {
            assert_eq!(seed, current);
            assert_eq!(queues.len(), 1);
        }
        other => panic!("expected the named-queue list, got {other:?}"),
    }
    assert!(
        plain_rx.try_recv().is_err(),
        "a Client without the capability receives nothing new"
    );
}

/// Contract: spec `named-queues` "Named queues are local-Owner only" and
/// design D3 — only the local Owner advertises `named-queues`. A Client
/// that connects to a packaged `mbvd` sees one queue and no create, rename,
/// or close of peer queues.
#[test]
fn only_the_local_owner_advertises_named_queues() {
    let packaged = daemon_hello(
        DaemonHelloCaps::default(),
        DaemonRole::Packaged,
        Some("control-token"),
    );
    let local = daemon_hello(
        DaemonHelloCaps::default(),
        DaemonRole::Local,
        Some("control-token"),
    );

    assert!(!packaged.supports_named_queues());
    assert!(local.supports_named_queues());
}

/// A registry with one local Client attached, so `has_driver` is true and
/// the `ExclusiveOwner` refusal fires under Stay-alive off.
fn registry_with_driver() -> (CtrlClients, mpsc::Receiver<CtrlOutbound>) {
    let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
    let mut clients = CtrlClients::new(
        merged_tx,
        Arc::new(mbv_ctrl::AttachedClientsCell::default()),
        Arc::new(crate::PendingSwapToken::default()),
    );
    let (client_tx, client_rx) = mpsc::channel::<CtrlOutbound>();
    clients.connect_with_role(
        client_tx,
        CtrlTransport::Local,
        false,
        false,
        crate::ctrl::PeerHelloCapabilities::default(),
        None,
        CtrlConnectionRole::Client,
    );
    (clients, client_rx)
}

/// Contract: spec daemon-lifecycle "Pin swap with Stay Alive off" /
/// design D4 — a Hello carrying the pending one-shot swap token is the
/// replacement Client the Owner itself started, so it is admitted
/// despite `ExclusiveOwner`, and the refusal event is not sent.
#[test]
fn matching_swap_token_is_admitted_despite_exclusive_owner() {
    let (clients, _client_rx) = registry_with_driver();
    clients.pending_swap().publish("token-1".to_string());
    let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

    let outcome = send_admission_refusal(
        &clients,
        DaemonRole::Local,
        CtrlTransport::Local,
        false,
        CtrlConnectionRole::Client,
        Some("token-1"),
        &ev_tx,
    );

    assert!(matches!(outcome, AdmissionOutcome::AdmittedReplacement(_)));
    assert!(ev_rx.try_recv().is_err());
}

/// Contract: design D4 — the token is consumed at once, so it works for
/// one connection only; a second Hello presenting the same token is
/// refused with the exclusive-owner reason like any user-started Client.
#[test]
fn a_swap_token_is_spent_on_first_use() {
    let (clients, _client_rx) = registry_with_driver();
    clients.pending_swap().publish("token-1".to_string());
    let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

    let first = send_admission_refusal(
        &clients,
        DaemonRole::Local,
        CtrlTransport::Local,
        false,
        CtrlConnectionRole::Client,
        Some("token-1"),
        &ev_tx,
    );
    let second = send_admission_refusal(
        &clients,
        DaemonRole::Local,
        CtrlTransport::Local,
        false,
        CtrlConnectionRole::Client,
        Some("token-1"),
        &ev_tx,
    );

    assert!(matches!(first, AdmissionOutcome::AdmittedReplacement(_)));
    assert!(matches!(second, AdmissionOutcome::Refused));
    // The second Hello was refused with the exclusive-owner reason.
    ev_rx.try_recv().unwrap();
}

/// Contract: design D4 — the token is consumed whatever the admission
/// outcome. Under Stay-alive on nothing refuses the replacement, but the
/// swap must still complete when it attaches (it guards against the
/// machine waiting out its deadline after a successful attach).
#[test]
fn the_swap_token_is_consumed_when_stay_alive_admits_normally() {
    let (clients, _client_rx) = registry_with_driver();
    clients.pending_swap().publish("token-1".to_string());
    let (ev_tx, _ev_rx) = mpsc::channel::<CtrlOutbound>();

    let outcome = send_admission_refusal(
        &clients,
        DaemonRole::Local,
        CtrlTransport::Local,
        true,
        CtrlConnectionRole::Client,
        Some("token-1"),
        &ev_tx,
    );

    assert!(matches!(outcome, AdmissionOutcome::AdmittedReplacement(_)));
    assert!(
        !clients.pending_swap().take_matching(Some("token-1")),
        "the first Hello spent the token"
    );
}

/// Contract: design D4 — the shutting-down refusal still wins, even for
/// a Hello that carries the pending swap token.
#[test]
fn shutting_down_refusal_wins_over_the_swap_token() {
    let (mut clients, _client_rx) = registry_with_driver();
    clients.shutting_down = true;
    clients.pending_swap().publish("token-1".to_string());
    let (ev_tx, ev_rx) = mpsc::channel::<CtrlOutbound>();

    let outcome = send_admission_refusal(
        &clients,
        DaemonRole::Local,
        CtrlTransport::Local,
        false,
        CtrlConnectionRole::Client,
        Some("token-1"),
        &ev_tx,
    );

    assert!(matches!(outcome, AdmissionOutcome::Refused));
    // The shutting-down refusal event was sent to the Hello.
    ev_rx.try_recv().unwrap();
    // The token stays pending: a refused Hello does not spend it.
    assert!(clients.pending_swap().take_matching(Some("token-1")));
}
