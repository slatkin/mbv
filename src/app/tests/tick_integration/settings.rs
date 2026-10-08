use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

use crate::app::PanelFocus;
use crate::app::dispatch::action::Command;
use crate::app::input::router::RouterOutcome;
use crate::app::tests::make_app_stub;
use crate::app::tests::tick_integration::harness::TickHarness;
use mbv_ui_msg::{ComponentId, Msg, OverlayId, UserEvent};

fn key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

/// Task 3.3 (add-mouse-support-option): the Display section's `MouseSupport`
/// row routes Enter through the shell sync pass to `handle_settings_activate`,
/// flipping the config value and arming the live capture flip for the run
/// loop (item ordinal 7: Services, 4 Playback, `ImageProtocol`,
/// `SystemNotifications`).
#[test]
fn settings_mouse_support_row_toggle_flips_config_and_arms_capture() {
    let mut harness = TickHarness::new(make_app_stub());

    harness.inject(key(Key::Function(2)));
    let outcome = harness.step();
    assert!(matches!(
        outcome.router,
        RouterOutcome::Command(Command::ToggleSettings)
    ));
    harness
        .model_mut()
        .dispatch_router_command(&Command::ToggleSettings);
    {
        let (mut music_resize, mut tv_resize) = (false, false);
        for message in outcome.messages {
            harness
                .model_mut()
                .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
        }
    }
    harness.model_mut().sync_mounted_surfaces();
    let settings_id = ComponentId::Overlay(OverlayId::Settings);
    assert!(harness.model().application.mounted(&settings_id));
    assert_eq!(harness.model().application.focus(), Some(&settings_id));

    // Locate the MouseSupport row ordinal instead of hardcoding Down presses:
    // the flat row order is the shell's visible SETTING_SECTIONS order.
    // The production cursor map is the single source of truth for that order.
    let row_count: usize = mbv_ui_model::settings::SETTING_SECTIONS
        .iter()
        .map(|(_, keys)| keys.len())
        .sum();
    let mouse_support_downs = (0..row_count)
        .find(|&cursor| {
            mbv_ui_model::settings::settings_cursor_to_key(cursor)
                == mbv_ui_model::settings::SettingKey::MouseSupport
        })
        .expect("MouseSupport row exists in the visible settings rows");
    for _ in 0..mouse_support_downs {
        harness.inject(key(Key::Down));
        let outcome = harness.step();
        let (mut music_resize, mut tv_resize) = (false, false);
        for message in outcome.messages {
            harness
                .model_mut()
                .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
        }
    }

    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }

    assert!(!harness.model().app.config.lock().unwrap().mouse_support);
    assert_eq!(harness.model().app.mouse_capture_pending, Some(false));
}

/// #745 (row 5.2): the Actions `Reset UI State` row routes Enter through the
/// existing activate intent to the shell reset coordinator instead of the App
/// configuration-edit/save path, and the coordinator dismisses Settings.
#[test]
fn settings_reset_ui_state_row_runs_the_coordinator_and_dismisses_settings() {
    let mut harness = TickHarness::new(make_app_stub());
    let feed = |harness: &mut TickHarness, messages: Vec<Msg>| {
        let (mut music_resize, mut tv_resize) = (false, false);
        for message in messages {
            harness
                .model_mut()
                .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
        }
    };

    harness.inject(key(Key::Function(2)));
    let outcome = harness.step();
    assert!(matches!(
        outcome.router,
        RouterOutcome::Command(Command::ToggleSettings)
    ));
    harness
        .model_mut()
        .dispatch_router_command(&Command::ToggleSettings);
    feed(&mut harness, outcome.messages);
    harness.model_mut().sync_mounted_surfaces();
    let settings_id = ComponentId::Overlay(OverlayId::Settings);
    assert!(harness.model().application.mounted(&settings_id));

    // A non-default focus the coordinator must restore, so delivery is
    // observable independently of dismissal.
    harness.model_mut().app.panel_focus = PanelFocus::Queue;

    // Locate the ResetUiState row via the production cursor map, the single
    // source of truth for the flat row order.
    let row_count: usize = mbv_ui_model::settings::SETTING_SECTIONS
        .iter()
        .map(|(_, keys)| keys.len())
        .sum();
    let reset_downs = (0..row_count)
        .find(|&cursor| {
            mbv_ui_model::settings::settings_cursor_to_key(cursor)
                == mbv_ui_model::settings::SettingKey::ResetUiState
        })
        .expect("ResetUiState row exists in the visible settings rows");
    for _ in 0..reset_downs {
        harness.inject(key(Key::Down));
        let outcome = harness.step();
        feed(&mut harness, outcome.messages);
    }

    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    feed(&mut harness, outcome.messages);

    assert!(
        !harness.model().application.mounted(&settings_id),
        "Reset UI State must dismiss Settings"
    );
    assert_eq!(
        harness.model().app.panel_focus,
        PanelFocus::Library,
        "the reset coordinator must restore the default focus"
    );
    assert!(
        harness.model().app.settings_save_at.is_none(),
        "the reset must bypass the configuration-edit/save path"
    );
}
