use crate::ctrl::{CtrlCmd, WireCommand};
use crate::playback_queue::FeedEntry;
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
