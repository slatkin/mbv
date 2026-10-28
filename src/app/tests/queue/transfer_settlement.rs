use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::queue_transfer::{
    PendingQueueTransfer, QueueTransferDirection, QueueTransferFollowUp,
};
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::tests::{make_audio_items, make_local_daemon_app_stub_with_cmd_rx};
use mbv_ctrl::player::PlayerStatus;
use mbv_ctrl::{CtrlCmd, UnifiedQueueSlot, UnifiedQueueStateData};
use mbv_queue::{QueueItem, QueueLineage, QueueSlotId, QueueSource};
use mbv_remote_player::{DaemonEndpoint, RemotePlayer};

/// Design D2 failure (queue-handoff-and-take row 7.1): when the target's
/// adopted snapshot no longer holds the start slot, admission dropped it and
/// the target started a different item — the Client stops the target,
/// reports the failure, and never stops the source.
#[test]
fn a_target_snapshot_without_the_start_slot_stops_the_target_and_never_the_source() {
    let (mut app, local_cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_audio_items(2));
    let (peer, peer_rx, peer_cmd_rx) = RemotePlayer::stub_with_command_rx(make_audio_items(2), 0);
    app.switch_to_direct_remote(
        &mbv_emby::test_support::make_session("peer-owner", "mbv"),
        peer,
        peer_rx,
        &DaemonEndpoint::Tcp("127.0.0.1:1".parse().unwrap()),
    );
    // Discard whatever the attach path sent, so the assertions below see only
    // the settlement's sends.
    while local_cmd_rx.try_recv().is_ok() {}
    while peer_cmd_rx.try_recv().is_ok() {}

    // The post-replacement snapshot the target adopted: the submitted start
    // slot 1 is gone (admission dropped it) and the target started slot 2.
    let items = make_audio_items(2);
    let snapshot = UnifiedQueueStateData {
        status: PlayerStatus {
            active: true,
            paused: false,
            ..PlayerStatus::default()
        },
        active_slot: Some(2),
        slots: items
            .into_iter()
            .enumerate()
            .map(|(index, item)| UnifiedQueueSlot {
                slot_id: (index + 2) as u64,
                item: QueueItem::Emby(Box::new(item)),
            })
            .collect(),
        revision: 1,
        source: QueueSource::Album,
        lineage: QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    app.handle_unified_queue_updated(&snapshot);

    app.pending_queue_transfer = Some(PendingQueueTransfer {
        direction: QueueTransferDirection::HandOff,
        target: mbv_ui_model::panel_targets::SessionTargetKey::Emby("peer-owner".to_string()),
        start_slot: QueueSlotId::from_raw(1),
        op: QueueTransferFollowUp::StayConnected,
        source_link: None,
    });

    let mut harness = TickHarness::new(app);
    harness.step();

    let app = &harness.model().app;
    assert!(app.pending_queue_transfer.is_none());
    // `PlayerProxy::stop` asks the owner through its standard playback-intent
    // stop (the same send the transport and consume paths use).
    assert!(matches!(
        peer_cmd_rx.try_recv(),
        Ok(CtrlCmd::PlaybackIntent(intent)) if intent.action == mbv_ctrl::PlaybackIntentAction::Stop
    ));
    while let Ok(cmd) = local_cmd_rx.try_recv() {
        assert!(
            !matches!(
                cmd,
                CtrlCmd::PlaybackIntent(intent) if intent.action == mbv_ctrl::PlaybackIntentAction::Stop
            ),
            "the source Owner must never be stopped by a failed transfer"
        );
    }
    assert_eq!(app.status_severity, ToastSeverity::Error);
    assert!(app.status.starts_with("Hand off failed"));
}
