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
