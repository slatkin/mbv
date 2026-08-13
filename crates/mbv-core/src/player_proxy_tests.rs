use crate::ctrl::{CtrlCmd, WireCommand};
use crate::playback_queue::{AudiobookshelfQueueItem, FeedEntry, QueueItem};
use crate::remote_player::RemotePlayer;

fn feed_entry(guid: &str) -> FeedEntry {
    FeedEntry {
        guid: guid.to_string(),
        title: format!("Feed {guid}"),
        enclosure_url: Some(format!("https://example.com/{guid}.mp3")),
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

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

// `stub_with_command_rx` hardcodes `CtrlCompatibility::current()` (both
// `unified-queue` and `feed-playback` on). Override to the legacy
// feed-capable-but-not-unified combination this fallback exists for.
fn stub_feed_capable_non_unified() -> (RemotePlayer, std::sync::mpsc::Receiver<CtrlCmd>) {
    let (mut remote, _event_rx, cmd_rx) = RemotePlayer::stub_with_command_rx(vec![], 0);
    remote.ctrl_compatibility.supports_unified_queue = false;
    remote.ctrl_compatibility.supports_feed_playback = true;
    (remote, cmd_rx)
}

#[test]
fn submit_queue_falls_back_to_legacy_play_feed_for_non_unified_peer() {
    let (remote, cmd_rx) = stub_feed_capable_non_unified();
    let proxy = PlayerProxy::remote(remote, false);

    let sent = proxy.submit_queue(vec![QueueItem::Feed(feed_entry("f1"))], 0, None, false, 100);

    assert!(sent);
    let cmd = cmd_rx.try_recv().expect("expected a command to be sent");
    match cmd {
        CtrlCmd::PlayerCmd(WireCommand::LoadFeed { entry }) => {
            assert_eq!(entry.guid, "f1");
        }
        _ => panic!("expected PlayerCmd(LoadFeed)"),
    }
}

#[test]
fn ctrl_owner_rejects_audiobookshelf_without_command_or_queue_mutation() {
    let (remote, cmd_rx) = stub_feed_capable_non_unified();
    let proxy = PlayerProxy::remote(remote, false);

    assert!(!proxy.can_admit_audiobookshelf());
    assert!(!proxy.submit_queue(vec![proxy_audiobookshelf_item()], 0, None, false, 100));
    assert!(!proxy.queue_append(vec![proxy_audiobookshelf_item()]));
    assert!(cmd_rx.try_recv().is_err());
}
