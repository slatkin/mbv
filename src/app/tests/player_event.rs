use crate::app::tests::*;
use mbv_core::player::{PlayerCommand, PlayerEvent};

#[test]
fn intro_started_auto_skips_when_client_prefers_it() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().always_skip_intro = true;
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
        app.status.is_empty(),
        "auto-skip must not leave a TUI prompt"
    );
}

#[test]
fn intro_started_does_not_show_tui_prompt_when_client_does_not_auto_skip() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.config.lock().unwrap().always_skip_intro = false;
    let rx = app.player.spy_on_commands();

    app.handle_player_event(PlayerEvent::IntroStarted {
        intro_end_ticks: 300_000_000,
    });

    assert!(rx.try_recv().is_err(), "manual mode must not auto-seek");
    assert!(
        app.status.is_empty(),
        "manual mode must not show a TUI prompt"
    );
}
