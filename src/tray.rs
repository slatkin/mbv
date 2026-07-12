use ksni::blocking::TrayMethods;
use mbv_core::player::{PlayerCommand, PlayerStatus};
use std::sync::mpsc::{Sender, SyncSender};
use std::sync::{Arc, Mutex};

const TRAY_ICON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tray_icon.bin"));

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
    /// The in-process `Player`'s own command channel, captured once while
    /// `PlayerProxy` was known to be local (see
    /// `PlayerProxy::local_cmd_tx`). Transport actions send directly into
    /// this channel -- never through `PlayerProxy`/the ctrl socket -- so
    /// they keep working, and stay local-only, across any later
    /// local/remote `PlayerProxy` swap on the app side.
    cmd_tx: Arc<Mutex<Option<Sender<PlayerCommand>>>>,
}

impl MbvTray {
    fn send_command(&self, cmd: PlayerCommand) {
        if let Some(tx) = self.cmd_tx.lock().unwrap().as_ref() {
            if let Err(e) = tx.send(cmd) {
                log::debug!(target: "tray", "player command dropped: {e}");
            }
        }
    }

    fn toggle_play_pause(&self) {
        self.send_command(PlayerCommand::TogglePause);
    }

    fn next(&self) {
        let idx = self.status.lock().unwrap().next_idx();
        if let Some(idx) = idx {
            self.send_command(PlayerCommand::JumpTo(idx));
        }
    }

    fn previous(&self) {
        let idx = self.status.lock().unwrap().previous_idx();
        if let Some(idx) = idx {
            self.send_command(PlayerCommand::JumpTo(idx));
        }
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
        use ksni::menu::*;
        let status = self.status.lock().unwrap().clone();

        let mut items: Vec<MenuItem<Self>> = vec![
            StandardItem {
                label: play_pause_label(&status).into(),
                icon_name: if status.active && !status.paused {
                    "media-playback-pause".into()
                } else {
                    "media-playback-start".into()
                },
                activate: Box::new(|tray: &mut Self| tray.toggle_play_pause()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next".into(),
                icon_name: "media-skip-forward".into(),
                activate: Box::new(|tray: &mut Self| tray.next()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Previous".into(),
                icon_name: "media-skip-backward".into(),
                activate: Box::new(|tray: &mut Self| tray.previous()),
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
                    let _ = tray.shutdown_tx.try_send(());
                }),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

/// Spawns the stay-alive tray (#156 T7 / #168 T-phase-2).
///
/// `status`/`cmd_tx` must come from the in-process `Player` (see
/// `PlayerProxy::local_cmd_tx`), never from a `RemotePlayer` -- the tray
/// must stay on the stay-alive side of the architecture and must not become
/// a ctrl-socket client. `shutdown_tx` keeps the existing Phase 1 real-quit
/// behavior (equivalent to `mbv -q` / graceful shutdown).
pub fn spawn(
    shutdown_tx: SyncSender<()>,
    status: Arc<Mutex<PlayerStatus>>,
    cmd_tx: Arc<Mutex<Option<Sender<PlayerCommand>>>>,
) -> Option<Box<dyn Send>> {
    MbvTray {
        shutdown_tx,
        status,
        cmd_tx,
    }
    .spawn()
    .map(|tray| Box::new(tray) as Box<dyn Send>)
    .map_err(|e| {
        log::warn!(target: "tray", "not available: {e}");
    })
    .ok()
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
    }

    #[test]
    fn now_playing_rows_hidden_when_idle() {
        assert!(!is_playing_with_title(&status(false, false, "A Song")));
    }

    #[test]
    fn now_playing_rows_hidden_when_paused() {
        assert!(!is_playing_with_title(&status(true, true, "A Song")));
    }

    #[test]
    fn now_playing_rows_hidden_when_no_title() {
        assert!(!is_playing_with_title(&status(true, false, "")));
    }

    #[test]
    fn play_pause_label_reflects_playing_state() {
        assert_eq!(play_pause_label(&status(true, false, "A Song")), "Pause");
        assert_eq!(play_pause_label(&status(true, true, "A Song")), "Play");
        assert_eq!(play_pause_label(&status(false, false, "")), "Play");
    }

    /// Builds a tray wired to a fresh command channel, so tests can assert
    /// on what `toggle_play_pause`/`next`/`previous` actually send without a
    /// real mpv thread. Mirrors `PlayerProxy::spy_on_commands`
    /// (crates/mbv-core/src/player.rs).
    fn spy_tray(st: PlayerStatus) -> (MbvTray, std::sync::mpsc::Receiver<PlayerCommand>) {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, _shutdown_rx) = std::sync::mpsc::sync_channel(1);
        let tray = MbvTray {
            shutdown_tx,
            status: Arc::new(Mutex::new(st)),
            cmd_tx: Arc::new(Mutex::new(Some(cmd_tx))),
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
        assert!(matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
    }

    #[test]
    fn toggle_play_pause_sends_toggle_pause_while_paused() {
        let (tray, rx) = spy_tray(status(true, true, "A Song"));
        tray.toggle_play_pause();
        assert!(matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
    }

    #[test]
    fn next_sends_jump_to_when_room() {
        let mut st = status(true, false, "A Song");
        st.current_idx = 0;
        st.queue_len = 2;
        let (tray, rx) = spy_tray(st);
        tray.next();
        assert!(matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))));
    }

    #[test]
    fn next_is_a_clean_noop_at_end_of_queue() {
        let mut st = status(true, false, "A Song");
        st.current_idx = 1;
        st.queue_len = 2;
        let (tray, rx) = spy_tray(st);
        tray.next();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn previous_sends_jump_to_when_available() {
        let mut st = status(true, false, "A Song");
        st.current_idx = 1;
        st.queue_len = 2;
        let (tray, rx) = spy_tray(st);
        tray.previous();
        assert!(matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(0))));
    }

    #[test]
    fn previous_is_a_clean_noop_at_start_of_queue() {
        let mut st = status(true, false, "A Song");
        st.current_idx = 0;
        st.queue_len = 2;
        let (tray, rx) = spy_tray(st);
        tray.previous();
        assert!(rx.try_recv().is_err());
    }
}
