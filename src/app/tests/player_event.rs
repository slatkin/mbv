use crate::app::tests::*;
use mbv_core::player::PlayerCommand;

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
        Ok(PlayerCommand::SeekAbsolute(secs)) if secs.to_bits() == 30.0_f64.to_bits()
    ));
    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::SkipIntroDismiss)));
    assert!(
        app.status.is_empty(),
        "auto-skip must not leave a TUI prompt"
    );
}
