use crate::app::tests::*;
use mbv_core::player::{PlayerCommand, PlayerEvent};

#[test]
fn intro_started_auto_skips_when_client_prefers_it() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.always_skip_intro = true;
    let rx = app.player.spy_on_commands();

    app.handle_player_event(PlayerEvent::IntroStarted {
        intro_end_ticks: 300_000_000, // 30s
    });

    assert!(matches!(
        rx.try_recv(),
        Ok(PlayerCommand::SeekAbsolute(secs)) if secs == 30.0
    ));
    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::SkipIntroDismiss)));
    assert!(
        app.skip_intro_end_ticks.is_none(),
        "auto-skip must not also arm the manual prompt"
    );
}

#[test]
fn intro_started_keeps_manual_prompt_when_client_prefers_it() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.always_skip_intro = false;
    let rx = app.player.spy_on_commands();

    app.handle_player_event(PlayerEvent::IntroStarted {
        intro_end_ticks: 300_000_000,
    });

    assert!(rx.try_recv().is_err(), "manual mode must not auto-seek");
    assert_eq!(app.skip_intro_end_ticks, Some(300_000_000));
    assert_eq!(app.status, "Skip intro? (Y/n)");
}

#[test]
fn feed_consumed_removes_matching_entry_from_client_feed_tail() {
    // The client mirrors the daemon's own feed-tail drain (§5.4): on
    // `FeedConsumed`, the entry with the matching guid must be removed from
    // `PlayerTab.feed_items` and no others.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let make_entry = |guid: &str| mbv_core::playback_queue::FeedEntry {
        guid: guid.to_string(),
        title: "Episode".to_string(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
    };
    app.player_tab.feed_items = vec![make_entry("keep"), make_entry("consumed")];

    app.handle_player_event(PlayerEvent::FeedConsumed {
        guid: "consumed".to_string(),
    });

    assert_eq!(
        app.player_tab
            .feed_items
            .iter()
            .map(|e| e.guid.as_str())
            .collect::<Vec<_>>(),
        ["keep"]
    );
}
