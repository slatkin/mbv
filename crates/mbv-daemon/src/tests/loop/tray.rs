// Tray wiring tests for the daemon event loop: a pinned local Client makes a
// daemon without stay-alive start its tray lazily.

use super::*;

/// Records the pin targets the daemon pushes to a started tray.
struct SpyTray {
    targets: mpsc::Sender<Option<std::path::PathBuf>>,
}

impl crate::TrayPort for SpyTray {
    fn set_pin_target(&self, socket: Option<std::path::PathBuf>) {
        let _ = self.targets.send(socket);
    }
}

/// Contract: the tray starts lazily, at most once, the first time a local
/// Client declares a pin to a daemon without stay-alive (local-daemon-tray
/// spec, "Pinned without stay-alive").
#[test]
fn tray_starts_lazily_once_when_pinned_without_stay_alive() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let starts = Arc::new(AtomicUsize::new(0));
    let (targets_tx, targets_rx) = mpsc::channel();
    let hook_starts = Arc::clone(&starts);
    let (shutdown_tx, _shutdown_rx) = mpsc::sync_channel(1);
    let mut t = test_loop_with_role(crate::DaemonRole::Local);
    t.event_loop.tray = crate::run::start_tray(
        &crate::owner_settings::fixed_reader(false),
        Box::new(move |_| {
            hook_starts.fetch_add(1, Ordering::SeqCst);
            Some(Box::new(SpyTray {
                targets: targets_tx,
            }) as Box<dyn crate::TrayPort>)
        }),
        shutdown_tx,
    );
    let (first, _first_rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());
    let (second, _second_rx) = connect_client(&mut t.event_loop.ctrl_clients.lock().unwrap());

    for (client_id, socket) in [
        (first, "/run/user/1000/pinwin/1.sock"),
        (second, "/run/user/1000/pinwin/2.sock"),
    ] {
        let (reply_tx, _reply_rx) = mpsc::channel();
        t.event_loop.handle_event(DaemonEvent::Ctrl(
            CtrlCmd::DeclarePinned {
                socket: std::path::PathBuf::from(socket),
            },
            client_id,
            reply_tx,
        ));
    }

    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(
        targets_rx.try_iter().collect::<Vec<_>>(),
        vec![
            Some(std::path::PathBuf::from("/run/user/1000/pinwin/1.sock")),
            Some(std::path::PathBuf::from("/run/user/1000/pinwin/2.sock")),
        ]
    );
}
