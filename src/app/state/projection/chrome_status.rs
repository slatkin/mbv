//! Status-bar content that builds no terminal value (task 2.2, design D3).
//!
//! The mounted `StatusBarPanel` Interactive Component
//! (`crates/mbv-components/src/status_bar_panel.rs`) owns the status row's pill hit
//! regions, overflow drop-order and click/scroll resolution. The `impl App`
//! methods here are content production only: they read app state and produce
//! the presented facts the shell's painters turn into spans.
//!
//! The span builders themselves moved to `shell::chrome_status`: they build
//! terminal values, so they belong to the TUI edge. The playback target's
//! labels and the in-app toast gate stay here, because they build no terminal
//! value and every Client reads them.
//!
//! The Local/Peer queue-link pills are queue concern and paint in the
//! `QueueColumn` footer (`render_queue_status`), never here.

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::toasts::LiveToast;
use crate::app::{App, QueueLink, RemoteSlotState};
use mbv_ui_model::toast::{StatusToast, ToastCard};
use std::time::Instant;

impl App {
    /// Whether a toast of `severity` renders in-app at all (spec
    /// `toast-notification-semantics`): a Neutral toast always does; a
    /// Success, Warning, or Error toast does only when system notifications
    /// are disabled or the desktop notification failed (`notif_failed`),
    /// because a delivered desktop notification hides the in-app toast.
    /// Restores the in-app toast surface dropped during the `TuiRealm`
    /// migration (commit `e9f3ab957`).
    fn renders_in_app(&self, severity: ToastSeverity) -> bool {
        matches!(severity, ToastSeverity::Neutral)
            || !self.system_notifications
            || self.notif_failed
    }

    /// The live toasts that render in-app, oldest first: the one list both the
    /// status row and the card stack read.
    fn in_app_toasts(&self) -> impl Iterator<Item = &LiveToast> {
        self.toasts
            .live(Instant::now())
            .filter(|toast| self.renders_in_app(toast.severity))
    }

    /// The live toast's presentation for the status row, or `None` when no
    /// toast should render in-app (spec `toast-notification-semantics`): the
    /// newest live toast that renders in-app. The presentation is neutral
    /// (extract-client-core row 4.6): the Severity's colour is the painters'
    /// choice, and the presented facts — the text and the severity — ride the
    /// Status bar's Surface state.
    pub(in crate::app) fn toast_presentation(&self) -> Option<StatusToast> {
        self.in_app_toasts().last().map(presented_toast)
    }

    /// Every live toast that renders in-app, oldest first (design D3): one
    /// card per toast, for the GUI Client's floating toast stack. The id is
    /// the card's stable key.
    #[must_use]
    pub fn toast_cards(&self) -> Vec<ToastCard> {
        self.in_app_toasts()
            .map(|toast| ToastCard {
                id: toast.id,
                toast: presented_toast(toast),
            })
            .collect()
    }

    /// The label naming the remote receiver that is the owner's playback
    /// output, per the last `CtrlEvent::PlaybackOutput` (design D12/D13).
    /// Feeds the queue header so it names the output consistently; the
    /// owner's report carries the discovered or device name. An Emby Session
    /// output names the Session device. A disconnected output keeps the
    /// device name with a " (disconnected)" suffix so the header presents
    /// it as disconnected (spec `cast-session-control` "Connection drops
    /// while attached", `owner-playback-output` "Device disappears"; bug:
    /// a disconnected output looked like local playback).
    fn remote_output_label(&self) -> Option<String> {
        match &self.playback_output {
            Some(
                mbv_ctrl::OutputState::Cast {
                    connection, name, ..
                }
                | mbv_ctrl::OutputState::EmbySession {
                    connection, name, ..
                },
            ) => match connection {
                mbv_ctrl::OutputConnection::Disconnected => Some(format!("{name} (disconnected)")),
                mbv_ctrl::OutputConnection::Connecting | mbv_ctrl::OutputConnection::Connected => {
                    Some(name.clone())
                }
            },
            Some(mbv_ctrl::OutputState::Local) | None => None,
        }
    }

    /// Whether the playback output reads as connected for the queue header:
    /// a remote output (Cast receiver or Emby Session) is attached — live
    /// or disconnected, which must still present as remote — or the remote
    /// slot names a direct route.
    fn remote_connected(&self, remote_state: RemoteSlotState) -> bool {
        self.remote_output_label().is_some()
            || matches!(remote_state, RemoteSlotState::DirectRemote)
    }

    /// The connected Player-owner endpoint's host label (PR #973 review):
    /// names the attachment the Client actually holds — never an assumed
    /// or configured host. `Local` carries no peer name; the caller's
    /// device-name fallback covers it.
    fn connected_player_endpoint_label(&self) -> Option<String> {
        self.player_endpoint
            .as_ref()
            .and_then(|endpoint| match endpoint {
                mbv_remote_player::DaemonEndpoint::Tcp(addr) => Some(addr.ip().to_string()),
                mbv_remote_player::DaemonEndpoint::Unix(path) => path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string),
                mbv_remote_player::DaemonEndpoint::Local => None,
            })
    }

    /// The playback target's label, gap-prefixed. An attached remote output
    /// names it first; otherwise the remote slot names it, and this machine's
    /// device name is the fallback. `or_else` leaves the slot unread while a
    /// receiver is attached. The queue-header caller trims the gap.
    fn remote_label(&self, remote_state: RemoteSlotState) -> String {
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let target = self.remote_output_label().or_else(|| match remote_state {
            RemoteSlotState::LocalDaemon => None,
            RemoteSlotState::DirectRemote => self
                .active_route
                .as_ref()
                .map(|name| format!("route:{name}"))
                .or_else(|| self.direct_remote_label.clone())
                .or_else(|| self.connected_player_endpoint_label()),
        });
        match target {
            Some(target) => format!("{gap}{target}"),
            None => format!("{gap}{}", mbv_emby::device_name()),
        }
    }

    /// The playback target's host label plus whether it names a remote target
    /// (a cast attachment, an Emby Session output, or a direct-remote
    /// route/label): the output's name, the direct-remote route/label, or
    /// this machine's device name when playback is local -- computed together
    /// from one `remote_slot_state()` resolution for callers (the queue header
    /// sync) that need both every tick. Callers style the returned label and
    /// the header row shows it verbatim.
    pub fn playback_host_label_and_remote(&self) -> (String, bool) {
        let remote_state = self.remote_slot_state();
        let is_remote = self.remote_connected(remote_state);
        let label = self.remote_label(remote_state);
        (label.trim_start().to_string(), is_remote)
    }

    /// The shown queue's Player owner's name (design D7, `queue-playback-panel`
    /// delta): the connected peer by device name while the shown queue is the
    /// peer link, else this machine's device name (the local Owner). The peer
    /// resolution mirrors `remote_label`: route name,
    /// session device name, connected endpoint host.
    pub(in crate::app) fn queue_owner_label(&self) -> String {
        if self.playing_queue_link() == QueueLink::Peer {
            self.peer_owner_label()
        } else {
            mbv_emby::device_name()
        }
    }

    /// The connected peer Player owner's name, as the Queue chooser's peer
    /// entry paints it (gui-named-queues design D8): the route name, the
    /// session device name, or the connected endpoint's host, with this
    /// machine's device name only as the last fallback. The resolution mirrors
    /// `remote_label`, and it does not depend on the shown entry, so the
    /// chooser names the peer while a local queue is shown.
    pub fn peer_owner_label(&self) -> String {
        self.active_route
            .as_ref()
            .map(|name| format!("route:{name}"))
            .or_else(|| self.direct_remote_label.clone())
            .or_else(|| self.connected_player_endpoint_label())
            .unwrap_or_else(mbv_emby::device_name)
    }
}

/// The presented facts of one live toast: its text and its severity. The id
/// and the expiry are the Client core's, never the painter's.
fn presented_toast(toast: &LiveToast) -> StatusToast {
    StatusToast {
        text: toast.text.clone(),
        severity: toast.severity,
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::app::dispatch::notify::ToastSeverity;
    use crate::app::tests::{
        make_app_stub, make_audio_items, make_local_daemon_app_stub_with_cmd_rx,
    };

    /// The queue owner label names the connected peer's endpoint when its
    /// device name is empty — the connected endpoint, never a configured or
    /// assumed one (PR #973 review; issue #972 drops the configured-endpoint
    /// config key, the label contract stays).
    #[test]
    fn an_empty_named_peer_labels_the_connected_endpoint_not_the_configured_one() {
        let (mut app, _cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_audio_items(2));
        let mut sess = mbv_emby::test_support::make_session("", "mbv");
        sess.id = "peer-1".to_string();
        let (peer, peer_rx, _peer_cmd_rx) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(make_audio_items(1), 0);
        app.switch_to_direct_remote(
            &sess,
            peer,
            peer_rx,
            &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:47788".parse().unwrap()),
        );

        assert_eq!(app.queue_owner_label(), "127.0.0.1");
    }

    /// The owner's cast output names the queue header's host, matching
    /// the connected icon cast-session-control "Playing while attached"
    /// pairs with it: the header previously showed the connected icon but
    /// labelled the host with this machine's device name.
    #[test]
    fn attached_cast_receiver_names_the_queue_header_host() {
        let mut app = make_app_stub();
        app.handle_player_event(mbv_ctrl::player::PlayerEvent::PlaybackOutput(
            mbv_ctrl::OutputState::Cast {
                receiver_id: "device-1".to_string(),
                name: "Living Room".to_string(),
                connection: mbv_ctrl::OutputConnection::Connected,
            },
        ));

        let (host, is_remote) = app.playback_host_label_and_remote();

        assert_eq!(host, "Living Room");
        assert!(is_remote);
    }

    /// A disconnected Cast output keeps naming the header's host with a
    /// " (disconnected)" suffix and still counts as remote, so the header
    /// presents the output as disconnected instead of looking like local
    /// playback (bug: a disconnected output looked like local playback;
    /// spec `cast-session-control` "Connection drops while attached",
    /// `owner-playback-output` "Device disappears" — every Client
    /// presents the output as disconnected).
    #[test]
    fn a_disconnected_cast_output_names_the_queue_header_as_disconnected() {
        let mut app = make_app_stub();
        app.handle_player_event(mbv_ctrl::player::PlayerEvent::PlaybackOutput(
            mbv_ctrl::OutputState::Cast {
                receiver_id: "device-1".to_string(),
                name: "Living Room".to_string(),
                connection: mbv_ctrl::OutputConnection::Disconnected,
            },
        ));

        let (host, is_remote) = app.playback_host_label_and_remote();

        assert_eq!(host, "Living Room (disconnected)");
        assert!(is_remote);
    }

    /// A live toast projects its message with the severity color on the
    /// status-bar surface; a delivered desktop notification hides the in-app
    /// row for non-neutral severities ("when the desktop notification
    /// succeeds, the in-app toast row is hidden"); an expired toast stops
    /// projecting (a toast expires on its severity-derived duration and
    /// claims no key). One outcome per case, stub setup in the body (spec
    /// `toast-notification-semantics`). (Bug regression context: the in-app
    /// toast render path was dropped during the `TuiRealm` migration, commit
    /// `e9f3ab957`.)
    #[rstest::rstest]
    #[case::live_toast_projects(ToastSeverity::Neutral, false, false, ToastExpiry::Live, true)]
    #[case::delivered_desktop_notification_hides_the_row(
        ToastSeverity::Error,
        true,
        false,
        ToastExpiry::Live,
        false
    )]
    #[case::expired_toast_stops_projecting(
        ToastSeverity::Neutral,
        false,
        false,
        ToastExpiry::Expired,
        false
    )]
    fn toast_projection_matches_the_spec(
        #[case] severity: ToastSeverity,
        #[case] system_notifications: bool,
        #[case] notif_failed: bool,
        #[case] expiry: ToastExpiry,
        #[case] projected: bool,
    ) {
        let mut app = make_app_stub();
        app.system_notifications = system_notifications;
        app.notif_failed = notif_failed;
        push_toast(&mut app, "Scanning 'Movies'...", severity, expiry);

        assert_eq!(
            app.toast_presentation()
                .is_some_and(|toast| toast.text.contains("Scanning")),
            projected,
        );
    }

    /// The GUI's card list drops the toasts that render out of app: one live
    /// Neutral toast then one delivered Error toast leaves only the Neutral
    /// card (spec `toast-notification-semantics`).
    #[test]
    fn a_delivered_error_toast_adds_no_card_beside_a_neutral_one() {
        let mut app = make_app_stub();
        app.system_notifications = true;
        app.notif_failed = false;
        push_toast(
            &mut app,
            "Scanning 'Movies'...",
            ToastSeverity::Neutral,
            ToastExpiry::Live,
        );
        push_toast(
            &mut app,
            "Couldn't load home",
            ToastSeverity::Error,
            ToastExpiry::Live,
        );

        let cards = app.toast_cards();

        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].toast.text, "Scanning 'Movies'...");
    }

    /// Case fixture: whether the toast under test is still live when the
    /// projection reads the clock, or already expired.
    #[derive(Clone, Copy)]
    enum ToastExpiry {
        Live,
        Expired,
    }

    /// Pushes one toast through the `ToastList` so it is live, or already
    /// expired, when the projection reads the clock: an expired toast arrived
    /// long enough ago that its own duration has run out.
    fn push_toast(app: &mut App, text: &str, severity: ToastSeverity, expiry: ToastExpiry) {
        let now = std::time::Instant::now();
        let arrived = match expiry {
            ToastExpiry::Live => now,
            ToastExpiry::Expired => now
                .checked_sub(severity.ttl() + std::time::Duration::from_secs(1))
                .expect("the process has run long enough to place a toast in the past"),
        };
        app.toasts.push(text.to_string(), severity, arrived);
    }
}
