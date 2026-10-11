use ksni::blocking::TrayMethods;
use mbv_ctrl::player::{PlayerCommand, PlayerStatus};
use mbv_ctrl::{AttachedClients, AttachedClientsCell, Direction, OwnerAction, TransportCommand};
use std::sync::mpsc::{Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TRAY_ICON: &[u8] = include_bytes!("../../../assets/tray_icon.bin");

/// How often the attached-Clients watcher re-reads the shared state to push a
/// menu rebuild, mirroring the daemon event loop's 1 s `TrayState::poll`
/// cadence for tray reconciliation.
const ATTACHED_CLIENTS_WATCH_INTERVAL: Duration = Duration::from_secs(1);

/// Whether the now-playing rows (`Playing` / `<Title>`) should be shown.
///
/// Per #168: only when playback is actually playing *and* a title is
/// available -- idle, paused, stopped, and no-title states must not show a
/// (misleading) now-playing row, but must keep the rest of the menu.
fn is_playing_with_title(status: &PlayerStatus) -> bool {
    status.active && !status.paused && !status.title.is_empty()
}

/// Label for the transport play/pause item, reflecting current state.
fn play_pause_label(status: &PlayerStatus) -> &'static str {
    if status.active && !status.paused {
        "Pause"
    } else {
        "Play"
    }
}

/// Label for the one Pin swap item: the direction the Owner's swap machine
/// will pick is derived from the registry at click time; this label only
/// mirrors the shared attached-Clients state (tray-open-terminal D1).
fn pin_swap_label(attached_clients: AttachedClients) -> &'static str {
    if attached_clients == AttachedClients::Pinned {
        "Unpin"
    } else {
        "Pin"
    }
}

struct MbvTray {
    shutdown_tx: SyncSender<()>,
    /// Snapshot of the in-process `Player`'s status, shared with the app's
    /// main loop (and mpris) -- read fresh each time the menu is opened.
    status: Arc<Mutex<PlayerStatus>>,
    /// Owner transport channel; relative steps are resolved by the daemon owner.
    transport_tx: Sender<TransportCommand>,
    /// Which local Clients are attached, refreshed by the ctrl registry on
    /// connect and disconnect (tray-open-terminal D1) -- read fresh each
    /// time the menu is opened, like `status`.
    attached_clients: Arc<AttachedClientsCell>,
}

impl MbvTray {
    fn send_command(&self, cmd: PlayerCommand) {
        if let Err(e) = self.transport_tx.send(TransportCommand::Player(cmd)) {
            tracing::debug!(
                name: "tray.player_command.dropped",
                target: "tray",
                error = %e,
                "player command dropped"
            );
        }
    }

    #[cfg(test)]
    fn toggle_play_pause(&self) {
        self.send_command(PlayerCommand::TogglePause);
    }

    #[cfg(test)]
    fn next(&self) {
        let _ = self
            .transport_tx
            .send(TransportCommand::Step(Direction::Next));
    }
}

/// Builds the **Open** item (only when no local Client is attached) and the
/// Pin swap item, in menu order, from one snapshot of the attached-Clients
/// state.
fn open_and_pin_swap_items(attached_clients: AttachedClients) -> Vec<ksni::MenuItem<MbvTray>> {
    use ksni::menu::StandardItem;

    let mut items: Vec<ksni::MenuItem<MbvTray>> = Vec::new();
    if attached_clients == AttachedClients::None {
        items.push(
            StandardItem {
                label: "Open".into(),
                activate: Box::new(|tray: &mut MbvTray| {
                    let _ = tray
                        .transport_tx
                        .send(TransportCommand::OwnerAction(OwnerAction::OpenTerminal));
                }),
                ..Default::default()
            }
            .into(),
        );
    }
    items.push(
        StandardItem {
            label: pin_swap_label(attached_clients).into(),
            activate: Box::new(|tray: &mut MbvTray| {
                // Owner actions route through the same transport channel
                // as the player commands; the daemon owner resolves the
                // swap direction and target from its registry, not from
                // this label (tray-pin-swap D3/D7).
                let _ = tray
                    .transport_tx
                    .send(TransportCommand::OwnerAction(OwnerAction::SwapPanel));
            }),
            ..Default::default()
        }
        .into(),
    );
    items
}

impl ksni::Tray for MbvTray {
    fn id(&self) -> String {
        "mbv".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: 24,
            height: 24,
            data: TRAY_ICON.to_vec(),
        }]
    }

    fn title(&self) -> String {
        "mbv".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{MenuItem, StandardItem};
        let status = self.status.lock().unwrap().clone();

        let mut items: Vec<MenuItem<Self>> = vec![
            StandardItem {
                label: play_pause_label(&status).into(),
                icon_name: if status.active && !status.paused {
                    "media-playback-pause".into()
                } else {
                    "media-playback-start".into()
                },
                activate: Box::new(|tray: &mut Self| tray.send_command(PlayerCommand::TogglePause)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next".into(),
                icon_name: "media-skip-forward".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray
                        .transport_tx
                        .send(TransportCommand::Step(Direction::Next));
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Previous".into(),
                icon_name: "media-skip-backward".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray
                        .transport_tx
                        .send(TransportCommand::Step(Direction::Previous));
                }),
                ..Default::default()
            }
            .into(),
        ];

        if is_playing_with_title(&status) {
            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: "Playing".into(),
                    enabled: false,
                    activate: Box::new(|_: &mut Self| {}),
                    ..Default::default()
                }
                .into(),
            );
            items.push(
                StandardItem {
                    label: status.title.clone(),
                    enabled: false,
                    activate: Box::new(|_: &mut Self| {}),
                    ..Default::default()
                }
                .into(),
            );
        }

        // Spec tray-open-terminal pin-swap "No Client attached": **Open**
        // sits directly above the Pin swap item and only while no local
        // Client is attached; Owner-action, admin, and TCP-only attachments
        // keep it hidden because the registry does not count them as
        // attached Clients.
        items.push(MenuItem::Separator);
        items.extend(open_and_pin_swap_items(self.attached_clients.load()));

        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    tracing::info!(
                        name: "tray.quit.requested",
                        target: "tray",
                        "quit requested from system tray"
                    );
                    let _ = tray.shutdown_tx.try_send(());
                }),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

/// Spawns the tray (#156 T7 / #168 T-phase-2).
///
/// The daemon enables the Tray when Stay-alive is on or "Show systray icon"
/// is on, and stops it when both turn off, so the returned box is the stop
/// operation: dropping it removes the icon (a bare ksni handle drop does not
/// stop the service loop), which is why it is wrapped in [`RunningTray`].
///
/// `transport_tx` routes controls through the local daemon owner; the tray
/// must stay on the local-daemon side of the architecture and must not
/// become a ctrl-socket client. `shutdown_tx` keeps the existing real-quit
/// behavior (equivalent to `mbv -q` / graceful shutdown), now driven by the
/// local daemon's own `shutdown_signal_tx` (see `local_daemon.rs`'s
/// `on_tray_ready` hook) rather than a self-SIGTERM inside `App`.
pub fn spawn(
    shutdown_tx: SyncSender<()>,
    status: Arc<Mutex<PlayerStatus>>,
    transport_tx: Sender<TransportCommand>,
    attached_clients: Arc<AttachedClientsCell>,
) -> Option<Box<dyn Send>> {
    let state_for_watcher = Arc::clone(&attached_clients);
    MbvTray {
        shutdown_tx,
        status,
        transport_tx,
        attached_clients,
    }
    .spawn()
    .map(|handle| {
        spawn_attached_clients_watcher(handle.clone(), state_for_watcher);
        Box::new(RunningTray(Some(handle))) as Box<dyn Send>
    })
    .map_err(|e| {
        tracing::warn!(
            name: "tray.availability.failed",
            target: "tray",
            error = %e,
            "not available"
        );
    })
    .ok()
}

/// Pushes a menu rebuild shortly after the shared attached-Clients state
/// changes, so the Open and Pin swap items are correct the next time the menu
/// opens (spec pin-swap: "The label SHALL reflect the attached Clients each
/// time the menu opens").
///
/// ksni serves the menu from a cache and re-runs `Tray::menu` only when
/// something asks for an update: a click, an `AboutToShow` from hosts that
/// signal one, or [`ksni::blocking::Handle::update`]. Nothing here can rely
/// on the host signaling `AboutToShow`, so the change is pushed instead: a
/// watcher thread polls the shared state -- the same shape as the daemon
/// event loop's 1 s `TrayState::poll`, but with its real clock, because the
/// injected-instant test seam cannot cross the ksni service boundary. The
/// rebuild diffs against the cached items, so a poll that finds no change
/// emits no D-Bus traffic. The thread ends within one interval of the tray
/// service stopping (`RunningTray` drop), and process exit reclaims it
/// regardless.
fn spawn_attached_clients_watcher(
    handle: ksni::blocking::Handle<MbvTray>,
    attached_clients: Arc<AttachedClientsCell>,
) {
    let _ = std::thread::Builder::new()
        .name("mbv-tray-clients-watch".into())
        .spawn(move || {
            let mut attached = attached_clients.load();
            loop {
                if handle.is_closed() {
                    return;
                }
                let now = attached_clients.load();
                if now != attached {
                    attached = now;
                    handle.update(|_| {});
                }
                std::thread::sleep(ATTACHED_CLIENTS_WATCH_INTERVAL);
            }
        });
}

/// Owning wrapper around the ksni handle: dropping it stops the tray
/// service and waits for that to complete, so dropping the boxed value
/// removes the icon. Dropping a raw ksni handle does not stop the service
/// (ksni 0.3.6 `service.rs` ignores a closed handle channel).
///
/// That wait is a thread join, and the drop is reachable on the daemon's
/// tick path (`TrayState::reconcile`), so the stop runs on a short-lived
/// killer thread instead of the dropping thread: a wedged tray service
/// thread must not stall every Client behind the daemon loop. A killer
/// thread outlived by process exit needs no joining — the process's D-Bus
/// connection closing removes the icon too.
struct RunningTray(Option<ksni::blocking::Handle<MbvTray>>);

impl Drop for RunningTray {
    fn drop(&mut self) {
        let Some(handle) = self.0.take() else { return };
        // Thread creation failing (resource exhaustion) drops the handle
        // without the graceful stop; the icon then goes only when the
        // process's D-Bus connection closes. Practically unreachable.
        let _ = std::thread::Builder::new()
            .name("mbv-tray-stop".into())
            .spawn(move || handle.shutdown().wait());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksni::Tray as _;
    use rstest::rstest;

    fn status(active: bool, paused: bool, title: &str) -> PlayerStatus {
        PlayerStatus {
            active,
            paused,
            title: title.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn now_playing_rows_shown_only_when_playing_with_title() {
        assert!(is_playing_with_title(&status(true, false, "A Song")));
        assert!(!is_playing_with_title(&status(false, false, "A Song")));
        assert!(!is_playing_with_title(&status(true, true, "A Song")));
        assert!(!is_playing_with_title(&status(true, false, "")));
    }

    /// Builds a tray wired to a fresh command channel, so tests can assert
    /// on what `toggle_play_pause`/`next`/`activate_item_labelled` actually
    /// send without a real mpv thread. Mirrors `PlayerProxy::spy_on_commands`
    /// (crates/mbv-player/src/proxy.rs).
    fn spy_tray(
        st: PlayerStatus,
        attached_clients: AttachedClients,
    ) -> (MbvTray, std::sync::mpsc::Receiver<TransportCommand>) {
        spy_tray_with_cell(Arc::new(AttachedClientsCell::new(attached_clients)), st)
    }

    /// Like [`spy_tray`], but the caller keeps the shared cell `Arc`, so a
    /// test can flip it after construction.
    fn spy_tray_with_cell(
        attached_clients: Arc<AttachedClientsCell>,
        st: PlayerStatus,
    ) -> (MbvTray, std::sync::mpsc::Receiver<TransportCommand>) {
        let (transport_tx, cmd_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, _shutdown_rx) = std::sync::mpsc::sync_channel(1);
        let tray = MbvTray {
            shutdown_tx,
            status: Arc::new(Mutex::new(st)),
            transport_tx,
            attached_clients,
        };
        (tray, cmd_rx)
    }

    /// Labels of the menu's enabled standard items, in order.
    fn menu_labels(tray: &MbvTray) -> Vec<String> {
        tray.menu()
            .into_iter()
            .filter_map(|item| match item {
                ksni::menu::MenuItem::Standard(item) if item.enabled => Some(item.label),
                _ => None,
            })
            .collect()
    }

    /// Activates the menu's standard item with `label` on `tray`.
    fn activate_item_labelled(tray: &mut MbvTray, label: &str) {
        for item in tray.menu() {
            if let ksni::menu::MenuItem::Standard(item) = item
                && item.label == label
            {
                (item.activate)(tray);
            }
        }
    }

    // Regression test for a bug caught in review: toggle_play_pause used to
    // compute a target-state via `toggle_to_reach`, which is meant for
    // remote-command dedup (only send if state actually differs) but was
    // fed an inverted target, so it was a no-op in both the "playing" and
    // "paused" cases -- the only two states a user would ever click it in.
    #[test]
    fn toggle_play_pause_sends_toggle_pause_while_playing() {
        let (tray, rx) = spy_tray(status(true, false, "A Song"), AttachedClients::None);
        tray.toggle_play_pause();
        assert!(matches!(
            rx.try_recv(),
            Ok(TransportCommand::Player(PlayerCommand::TogglePause))
        ));
    }

    #[test]
    fn next_emits_relative_next_command() {
        let (tray, rx) = spy_tray(status(true, false, "A Song"), AttachedClients::None);
        tray.next();
        assert!(matches!(
            rx.try_recv(),
            Ok(TransportCommand::Step(Direction::Next))
        ));
    }

    /// Tray-open-terminal pin-swap spec "No Client attached" / "A terminal
    /// Client is attached" / "A pinned Client is attached": exactly one Pin
    /// swap item, labelled from the attached-Clients state, and **Open** only
    /// in the `None` state, directly above it; activating the Pin swap item
    /// sends `OwnerAction(SwapPanel)`, the same Owner action the CLI flag
    /// runs.
    #[rstest]
    #[case::no_client(AttachedClients::None, &["Open", "Pin"], "Pin")]
    #[case::unpinned_client_attached(AttachedClients::Unpinned, &["Pin"], "Pin")]
    #[case::pinned_client_attached(AttachedClients::Pinned, &["Unpin"], "Unpin")]
    fn pin_swap_item_label_and_activation_follow_the_attached_clients(
        #[case] attached_clients: AttachedClients,
        #[case] expected_swap_items: &[&str],
        #[case] pin_swap_label: &str,
    ) {
        let (mut tray, rx) = spy_tray(status(true, false, "A Song"), attached_clients);

        let labels = menu_labels(&tray);
        assert_eq!(labels.last().map(String::as_str), Some("Quit"));
        let swap_items: Vec<&str> = labels
            .iter()
            .map(String::as_str)
            .filter(|label| matches!(*label, "Open" | "Pin" | "Unpin"))
            .collect();
        assert_eq!(swap_items, expected_swap_items);

        activate_item_labelled(&mut tray, pin_swap_label);
        assert!(matches!(
            rx.try_recv(),
            Ok(TransportCommand::OwnerAction(OwnerAction::SwapPanel))
        ));
    }

    /// Contract: tray-pin-swap spec "The label SHALL reflect the attached
    /// Clients each time the menu opens" -- the label is computed from the
    /// shared state at each menu build, not captured at construction. This
    /// pins the read side of the fix; the write side (pushing a rebuild when
    /// the state changes) is `spawn_attached_clients_watcher`, whose polling
    /// loop needs the real ksni service and is exercised only live.
    #[test]
    fn pin_swap_label_follows_a_state_change_after_construction() {
        let state = Arc::new(AttachedClientsCell::default());
        let (tray, _rx) = spy_tray_with_cell(Arc::clone(&state), status(true, false, "A Song"));

        assert_eq!(pin_swap_label_of(&tray), "Pin");
        state.store(AttachedClients::Pinned);
        assert_eq!(pin_swap_label_of(&tray), "Unpin");
        state.store(AttachedClients::None);
        assert_eq!(pin_swap_label_of(&tray), "Pin");
    }

    /// Tray-open-terminal pin-swap spec "No Client attached": activating the
    /// **Open** item sends `OwnerAction(OpenTerminal)`, the Owner action the
    /// `--open` CLI flag runs.
    #[test]
    fn activating_open_sends_open_terminal() {
        let (mut tray, rx) = spy_tray(status(true, false, "A Song"), AttachedClients::None);

        activate_item_labelled(&mut tray, "Open");
        assert!(matches!(
            rx.try_recv(),
            Ok(TransportCommand::OwnerAction(OwnerAction::OpenTerminal))
        ));
    }

    /// The Pin swap item's label of the current menu build (the item sits
    /// before the Quit separator, hence second to last).
    fn pin_swap_label_of(tray: &MbvTray) -> String {
        let labels = menu_labels(tray);
        labels[labels.len() - 2].clone()
    }
}
