//! Playlist-save routing to the Local queue's owner link (queue-owner-process
//! Unit 4 correction; spec local-daemon-thin-client, "Every local Client shows
//! the owner's accepted queue" — a Save As source change "SHALL be reflected
//! by the owner and all attached Clients", so the source update and its
//! lineage fence must address the owner link, not a routed peer).

use crate::app::state::queue_owner::QueueOrigin;
use crate::app::tests::*;
use mbv_queue::{QueueLineage, QueueSource};

/// A Local-daemon stub whose home link is suspended under a routed peer:
/// the home link carries lineage 7, the routed peer stays at the default
/// lineage 0. Both advertise the owner-queue-load capability so a save's
/// source update would succeed on either link — only the targeting under
/// test decides which one receives it.
fn app_with_suspended_home() -> (
    App,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    let mut app = make_local_daemon_app_stub(make_items(1));
    let (home, home_rx, home_cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_owner_queue_load_with_command_rx(make_items(1), 0);
    home.unified_queue.lock().unwrap().as_mut().unwrap().lineage = QueueLineage(7);
    app.player = mbv_player::PlayerProxy::remote(home, false);
    app.player_rx = home_rx;

    let (route, route_rx, route_cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_owner_queue_load_with_command_rx(make_items(1), 0);
    app.switch_to_library_route(
        "music",
        route,
        route_rx,
        &crate::app::tests::route_state::stub_endpoint(),
    );
    while route_cmd_rx.try_recv().is_ok() {}

    assert!(app.suspended_local.is_some());
    (app, home_cmd_rx, route_cmd_rx)
}

#[test]
fn queue_origin_reads_the_local_owner_link_lineage_not_the_routed_peer() {
    let (app, _home_cmd_rx, _route_cmd_rx) = app_with_suspended_home();

    let origin = app
        .queue_origin()
        .expect("the suspended home owner has delivered a queue snapshot");

    assert_eq!(origin.lineage, QueueLineage(7));
}

#[test]
fn saved_playlist_source_update_targets_the_owner_link() {
    let (mut app, home_cmd_rx, route_cmd_rx) = app_with_suspended_home();
    let origin = QueueOrigin {
        epoch: app.queue_epoch,
        lineage: QueueLineage(7),
    };

    assert!(app.apply_saved_playlist_source(QueueSource::Album, origin));

    let cmd = home_cmd_rx
        .try_recv()
        .expect("the owner link must receive the source update");
    match cmd {
        mbv_ctrl::CtrlCmd::UnifiedQueueSourceUpdate {
            source, lineage, ..
        } => {
            assert_eq!(source, QueueSource::Album);
            assert_eq!(lineage, QueueLineage(7));
        }
        other => panic!("unexpected command on the owner link: {other:?}"),
    }
    assert!(
        route_cmd_rx.try_recv().is_err(),
        "the routed peer must not receive the source update"
    );
    assert_eq!(
        app.pending_owner_source_update,
        Some((QueueSource::Album, QueueLineage(7)))
    );
}

fn home_unified(source: QueueSource, lineage: QueueLineage) -> mbv_ctrl::UnifiedQueueStateData {
    let mut unified = emby_unified_state(&[], 0);
    unified.source = source;
    unified.lineage = lineage;
    unified
}

#[test]
fn adopt_home_snapshot_reconciles_a_matching_pending_owner_source_update() {
    let mut app = make_app_stub();
    app.pending_owner_source_update = Some((QueueSource::Album, QueueLineage(7)));
    app.queue_dirty = true;

    app.adopt_home_snapshot(&home_unified(QueueSource::Album, QueueLineage(7)));

    assert!(app.pending_owner_source_update.is_none());
    assert!(!app.queue_dirty);
}

#[test]
fn adopt_home_snapshot_clears_a_pending_owner_source_update_on_new_lineage() {
    let mut app = make_app_stub();
    app.pending_owner_source_update = Some((QueueSource::Album, QueueLineage(9)));
    app.queue_dirty = true;

    app.adopt_home_snapshot(&home_unified(QueueSource::Album, QueueLineage(7)));

    assert!(app.pending_owner_source_update.is_none());
    // The pending update was superseded by a different owner lineage, so the
    // queue stays dirty: its save never confirmed against this snapshot.
    assert!(app.queue_dirty);
}
