// Direct `handle_ctrl` coverage for the unified-queue mutation commands
// (Append / MoveSlot / RemoveSlot branches / Clear). These arms mutate the
// daemon's canonical queue and mirror the change to the player; the tests
// assert both the resulting canonical order and the forwarded PlayerCommand.

use super::*;

fn queue_op_client(token: &str) -> Arc<Mutex<crate::api::EmbyClient>> {
    let mut client = crate::api::EmbyClient::new(Config::default());
    client.token = token.to_string();
    Arc::new(Mutex::new(client))
}

fn run_queue_cmd(
    cmd: CtrlCmd,
    client_id: u64,
    reply_tx: &mpsc::Sender<CtrlOutbound>,
    client: &Arc<Mutex<crate::api::EmbyClient>>,
    player: &Player,
    owner: &mut DaemonPlayerOwner,
    registry: &Arc<Mutex<CtrlClients>>,
) {
    let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
    handle_ctrl_for_role(
        cmd,
        CtrlContext {
            reply_tx,
            client_id,
            client,
            player,
            audio_only: false,
            owner,
            shared_queue: &shared_queue_state(),
            ctrl_clients: registry,
            has_audiobookshelf: false,
            merged_tx: &merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Local,
        },
    );
}

/// Same as [`run_queue_cmd`], but with an explicit shared queue snapshot so a
/// test can seed and then assert on `observed_active_slot`.
fn run_queue_cmd_with_shared(
    cmd: CtrlCmd,
    client_id: u64,
    reply_tx: &mpsc::Sender<CtrlOutbound>,
    client: &Arc<Mutex<crate::api::EmbyClient>>,
    player: &Player,
    owner: &mut DaemonPlayerOwner,
    shared_queue: &SharedQueueState,
    registry: &Arc<Mutex<CtrlClients>>,
) {
    let (merged_tx, _merged_rx) = mpsc::channel::<DaemonEvent>();
    handle_ctrl_for_role(
        cmd,
        CtrlContext {
            reply_tx,
            client_id,
            client,
            player,
            audio_only: false,
            owner,
            shared_queue,
            ctrl_clients: registry,
            has_audiobookshelf: false,
            merged_tx: &merged_tx,
            stay_alive: false,
            role: crate::daemon::DaemonRole::Local,
        },
    );
}

pub fn owner_with(items: Vec<QueueItem>, active: usize) -> DaemonPlayerOwner {
    DaemonPlayerOwner {
        core: PlayerOwnerState::new(
            PlaybackQueue::from_queue_items(items, Some(active)),
            QueueSource::Unknown,
        ),
        ..Default::default()
    }
}

// Queue mutations preserve canonical order and mirror changes to the player.
mod queue_mutations;
// Queue state persists and is taken over across owner restarts.
mod queue_persistence;
// Idle queue loads validate peers and commit only after playback stops.
mod queue_idle_load;
// Queue replacements and source updates publish lineage-consistent snapshots.
mod queue_source_updates;
