use crate::playback_queue::{AudiobookshelfQueueItem, QueueItem};
use crate::remote_player::RemotePlayer;

fn proxy_audiobookshelf_item() -> QueueItem {
    QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
        library_item_id: "show-1".into(),
        episode_id: "episode-1".into(),
        title: "Episode 1".into(),
        show_title: None,
        author: None,
        duration_ticks: Some(100),
        position_ticks: 0,
        played: false,
        pub_date_secs: None,
        is_finished: false,
        cover_path: None,
    })
}

#[test]
fn ctrl_owner_rejects_audiobookshelf_without_command_or_queue_mutation() {
    let (remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(vec![], 0);
    let proxy = PlayerProxy::remote(remote, false);

    assert!(!proxy.can_admit_audiobookshelf());
    assert!(!proxy.submit_queue(vec![proxy_audiobookshelf_item()], 0, None, false, 100));
    assert!(!proxy.queue_append(vec![proxy_audiobookshelf_item()]));
    assert!(cmd_rx.try_recv().is_err());
}
