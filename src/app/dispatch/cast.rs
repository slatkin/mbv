// The F3 panel's Cast half: selecting a cast target routes through the
// Player owner (`select_panel_target`, design D13) and cast receiver
// discovery feeds the panel's merged target list. The TUI holds no receiver
// connection: the Owner is the only Player owner, and its Cast output is
// the one place a receiver is attached.

use crate::app::App;
use crate::app::state::panel_targets::PanelTarget;
use crate::app::state::types::cast::CastEvent;

/// How long a single cast discovery browse is allowed to run when the F3
/// panel opens (8.1). Runs concurrently with the `/Sessions` fetch on its
/// own background thread, so this bounds only the cast half of the panel's
/// content, never Emby rows rendering.
const CAST_DISCOVERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

impl App {
    /// Selecting a target from the F3 panel (design D13): the previously
    /// connected target is severed first (attachment slots are mutually
    /// exclusive), then an Emby target either keeps Direct remote control or
    /// becomes the owner's Emby Session output (`activate_session_target`,
    /// design D12); a cast target sends `CtrlCmd::SelectPlaybackOutput(Cast)`
    /// to the owner whose `playback-output` capability gated the row (D3). The
    /// owner performs the hand-off itself — no `Stop` here, or the active slot
    /// the new receiver must continue would be cleared (spec scenario "A
    /// different cast target is selected while one is attached"). The Client
    /// holds no receiver connection: the owner connects and broadcasts the
    /// resulting `CtrlEvent::PlaybackOutput`.
    pub(in crate::app) fn select_panel_target(&mut self, target: PanelTarget) {
        match target {
            PanelTarget::Emby(session) => self.activate_session_target(&session),
            PanelTarget::Cast(receiver) => {
                self.sever_connections_without_output_change();
                // Same panel-close-on-connect parity as the Emby path
                // (`connect_to_session` dismisses the Sessions sidebar).
                self.request_sidebar_dismiss(mbv_ui_model::overlay::SidebarId::Sessions);
                self.select_playback_output(mbv_ctrl::OutputSelection::Cast {
                    receiver_id: receiver.id,
                    name: receiver.friendly_name,
                    host: receiver.host,
                    port: receiver.port,
                });
            }
        }
    }

    /// Browses for cast receivers on a background thread and reports the
    /// result back as `CastEvent::DiscoveryCompleted` (8.1). Runs
    /// concurrently with `spawn_sessions_load`'s `/Sessions` fetch -- callers
    /// invoke both, never wait on this one before showing Emby rows.
    pub(in crate::app) fn spawn_cast_discovery(&mut self) {
        let tx = self.channels.cast_tx.clone();
        std::thread::spawn(move || {
            let receivers = mbv_cast::discovery::browse_cast_receivers(CAST_DISCOVERY_TIMEOUT);
            let _ = tx.send(CastEvent::DiscoveryCompleted(receivers));
        });
    }

    pub(in crate::app) fn handle_cast_event(&mut self, event: CastEvent) {
        match event {
            CastEvent::DiscoveryCompleted(receivers) => {
                self.cast_receivers = receivers;
                self.rebuild_panel_targets();
            }
        }
    }

    pub(in crate::app) fn drain_cast_events(&mut self) -> bool {
        let mut produced = false;
        while let Ok(event) = self.channels.cast_rx.try_recv() {
            produced = true;
            self.handle_cast_event(event);
        }
        produced
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{QueueViewTestExt, live_owner_channel, make_app_stub};
    use mbv_queue::{FeedEntry, QueueItem};

    fn feed_item(guid: &str, url: Option<&str>) -> QueueItem {
        QueueItem::Feed(FeedEntry {
            guid: guid.to_string(),
            title: format!("Episode {guid}"),
            enclosure_url: url.map(str::to_string),
            link: None,
            mime_type: Some("audio/mpeg".to_string()),
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: Some("feed".to_string()),
            position_ticks: 0,
            played: false,
        })
    }

    /// cast-owner-output row 8.1, design D13, spec "Selecting a cast target
    /// makes it the owner's output": selecting a cast row sends
    /// `CtrlCmd::SelectPlaybackOutput(Cast)` to the owner (no Stop — the
    /// owner performs the hand-off), attaches no local transport, dismisses
    /// the F3 panel like the Emby path, and leaves the queue intact.
    #[test]
    fn selecting_a_cast_target_from_the_panel_selects_the_owner_output_and_leaves_the_queue_intact()
    {
        let mut app = make_app_stub();
        let cmd_rx = live_owner_channel(&mut app);
        app.local_view
            .adopt_queue_items(vec![feed_item("a", Some("https://feed/a.mp3"))], 0);
        let before: Vec<String> = app
            .local_view
            .slots()
            .iter()
            .map(|s| s.item.id().to_string())
            .collect();

        let receiver = mbv_cast::discovery::CastReceiver {
            id: "device-1".to_string(),
            friendly_name: "Living Room".to_string(),
            host: "192.168.0.5".to_string(),
            port: 8009,
        };
        app.select_panel_target(PanelTarget::Cast(receiver));

        assert!(matches!(
            cmd_rx.try_recv().unwrap(),
            mbv_ctrl::CtrlCmd::SelectPlaybackOutput(mbv_ctrl::OutputSelection::Cast {
                ref receiver_id,
                ..
            }) if receiver_id == "device-1"
        ));
        assert!(
            cmd_rx.try_recv().is_err(),
            "no command precedes the selection: the owner performs the hand-off"
        );
        // Emby-path parity: selecting a target dismisses the F3 Sessions
        // panel (`connect_to_session` requests the same dismissal).
        match app.pending_overlay {
            Some(mbv_ui_model::overlay::OverlayRequest::DismissSidebar(
                mbv_ui_model::overlay::SidebarId::Sessions,
            )) => {}
            other => panic!("expected Sessions-sidebar dismissal, got {other:?}"),
        }
        let after: Vec<String> = app
            .local_view
            .slots()
            .iter()
            .map(|s| s.item.id().to_string())
            .collect();
        assert_eq!(before, after);
    }
}
