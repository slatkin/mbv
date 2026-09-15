use crate::ctrl::{CtrlCmd, CtrlCompatibility};
use crate::playback_queue::{AudiobookshelfQueueItem, QueueItem, QueueSlotId};
use crate::playback_execution_sequence::ExecSlot;
use crate::remote_player::RemotePlayer;

fn proxy_audiobookshelf_item() -> QueueItem {
    QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
        library_item_id: "show-1".into(),
        episode_id: "episode-1".into(),
        title: "Episode 1".into(),
        show_title: None,
        author: None,
        description: None,
        duration_ticks: Some(100),
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    })
}

fn capability_abs_disabled() -> CtrlCompatibility {
    let mut compat = CtrlCompatibility::current();
    compat.supports_abs_queue = false;
    compat.supports_abs_book_queue = false;
    compat
}

#[test]
fn capable_ctrl_owner_admits_audiobookshelf_and_forwards_commands() {
    let (remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(vec![], 0);
    let proxy = PlayerProxy::remote(remote, false);

    let paired = |item: QueueItem| ExecSlot {
        slot_id: QueueSlotId::from_raw(900),
        item,
    };
    assert!(proxy.can_admit_audiobookshelf());
    assert!(proxy.submit_queue_slots(vec![paired(proxy_audiobookshelf_item())], 0, None, false, 100));
    assert!(proxy.queue_append(vec![paired(proxy_audiobookshelf_item())]));
    assert!(matches!(
        cmd_rx.recv().unwrap(),
        CtrlCmd::UnifiedQueueReplace { .. }
    ));
    assert!(matches!(
        cmd_rx.recv().unwrap(),
        CtrlCmd::UnifiedQueueAppend { .. }
    ));
}

#[test]
fn owner_audio_only_reflects_remote_capability_and_local_player_is_capable() {
    let local = PlayerProxy::stub(std::sync::Arc::new(std::sync::Mutex::new(
        crate::player::PlayerStatus::default(),
    )));
    assert!(!local.owner_is_audio_only());

    let (mut remote, _event_rx, _cmd_rx) = RemotePlayer::stub_with_command_rx(vec![], 0);
    assert!(!PlayerProxy::remote(remote.clone(), false).owner_is_audio_only());
    remote.ctrl_compatibility.supports_audio_only = true;
    assert!(PlayerProxy::remote(remote, false).owner_is_audio_only());
}

#[test]
fn incapable_peer_rejects_audiobookshelf_without_command_or_queue_mutation() {
    let (mut remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(vec![], 0);
    remote.ctrl_compatibility = capability_abs_disabled();
    let proxy = PlayerProxy::remote(remote, false);

    let paired = |item: QueueItem| ExecSlot {
        slot_id: QueueSlotId::from_raw(900),
        item,
    };
    assert!(!proxy.can_admit_audiobookshelf());
    assert!(!proxy.submit_queue_slots(
        vec![paired(proxy_audiobookshelf_item())],
        0,
        None,
        false,
        100
    ));
    assert!(!proxy.queue_append(vec![paired(proxy_audiobookshelf_item())]));
    assert!(cmd_rx.try_recv().is_err());
}
