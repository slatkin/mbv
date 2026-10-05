use ksni::blocking::TrayMethods;
use mbv_ctrl::player::{PlayerCommand, PlayerStatus};
use mbv_ctrl::{Direction, TransportCommand};
use std::sync::mpsc::{Sender, SyncSender};
use std::sync::{Arc, Mutex};

const TRAY_ICON: &[u8] = include_bytes!("../../../assets/tray_icon.bin");

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

struct MbvTray {
    shutdown_tx: SyncSender<()>,
    /// Snapshot of the in-process `Player`'s status, shared with the app's
    /// main loop (and mpris) -- read fresh each time the menu is opened.
    status: Arc<Mutex<PlayerStatus>>,
    /// Owner transport channel; relative steps are resolved by the daemon owner.
    transport_tx: Sender<TransportCommand>,
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
) -> Option<Box<dyn Send>> {
    MbvTray {
        shutdown_tx,
        status,
        transport_tx,
    }
    .spawn()
    .map(|tray| Box::new(RunningTray(Some(tray))) as Box<dyn Send>)
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
    /// on what `toggle_play_pause`/`next`/`previous` actually send without a
    /// real mpv thread. Mirrors `PlayerProxy::spy_on_commands`
    /// (crates/mbv-player/src/proxy.rs).
    fn spy_tray(st: PlayerStatus) -> (MbvTray, std::sync::mpsc::Receiver<TransportCommand>) {
        let (transport_tx, cmd_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, _shutdown_rx) = std::sync::mpsc::sync_channel(1);
        let tray = MbvTray {
            shutdown_tx,
            status: Arc::new(Mutex::new(st)),
            transport_tx,
        };
        (tray, cmd_rx)
    }

    // Regression test for a bug caught in review: toggle_play_pause used to
    // compute a target-state via `toggle_to_reach`, which is meant for
    // remote-command dedup (only send if state actually differs) but was
    // fed an inverted target, so it was a no-op in both the "playing" and
    // "paused" cases -- the only two states a user would ever click it in.
    #[test]
    fn toggle_play_pause_sends_toggle_pause_while_playing() {
        let (tray, rx) = spy_tray(status(true, false, "A Song"));
        tray.toggle_play_pause();
        assert!(matches!(
            rx.try_recv(),
            Ok(TransportCommand::Player(PlayerCommand::TogglePause))
        ));
    }

    #[test]
    fn next_emits_relative_next_command() {
        let (tray, rx) = spy_tray(status(true, false, "A Song"));
        tray.next();
        assert!(matches!(
            rx.try_recv(),
            Ok(TransportCommand::Step(Direction::Next))
        ));
    }
}
