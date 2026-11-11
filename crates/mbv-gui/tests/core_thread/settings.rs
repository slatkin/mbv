//! The Settings screen's core contracts (`add-gui-settings-screen` rows
//! 3.2, 3.3): the open actions open, close, and switch the screen through the
//! core thread's drain (design D2), a start with nothing configured opens
//! Services, and the global keys keep working behind the screen while the
//! queue keys stay behind it (the spec's *Global keys keep working behind the
//! screen*).

use mbv::{stub_app_with_playing_queue, stub_app_with_queue};
use mbv_gui::core_thread::CoreInput;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};
use rstest::rstest;

use crate::test_core_with;

/// A key press with no modifier held.
fn press(key: Key, modifiers: KeyModifiers) -> CoreInput {
    CoreInput::Key(KeyEvent::new(key, modifiers))
}

/// The open action each function key carries: F2 opens the Settings screen's
/// F2 group, F3 and F4 their own sections (the spec's open-action
/// requirement).
#[rstest]
#[case::f2(Key::Function(2), mbv::GuiSettingsSection::Services)]
#[case::f3(Key::Function(3), mbv::GuiSettingsSection::Sessions)]
#[case::f4(Key::Function(4), mbv::GuiSettingsSection::Playlists)]
fn a_function_key_opens_the_screen_at_its_section(
    #[case] key: Key,
    #[case] section: mbv::GuiSettingsSection,
) {
    let mut test = test_core_with(stub_app_with_queue(2));

    test.tx.send(press(key, KeyModifiers::NONE)).unwrap();
    assert!(test.core.step());

    assert_eq!(test.core.gui_settings_section_for_test(), Some(section));
}

/// A rebound `settings_open` chord opens the screen (the spec's *A rebound
/// action opens the screen*): the drain acts on the routed intent, never on
/// the default chord.
#[test]
fn a_rebound_settings_open_chord_opens_the_screen() {
    let mut app = stub_app_with_queue(2);
    app.rebind_chord_for_test("settings_open", "K");
    let mut test = test_core_with(app);

    test.tx
        .send(press(Key::Char('K'), KeyModifiers::NONE))
        .unwrap();
    assert!(test.core.step());

    assert_eq!(
        test.core.gui_settings_section_for_test(),
        Some(mbv::GuiSettingsSection::Services)
    );
}

/// A first-start configuration with no Remote Service and no feed
/// subscriptions opens the screen at Services (the spec's *The screen opens
/// at Services when nothing is set up*): the App-side open rides
/// `pending_overlay`, and the drain is the only presenter the GUI has.
#[test]
fn a_first_start_opens_the_screen_at_services() {
    let mut app = stub_app_with_queue(2);
    app.seed_first_start_settings_for_test();
    let mut test = test_core_with(app);

    assert!(test.core.step());

    assert_eq!(
        test.core.gui_settings_section_for_test(),
        Some(mbv::GuiSettingsSection::Services)
    );
}

/// Space toggles playback behind the open screen (the spec's *Global keys
/// keep working behind the screen*): the Settings id holds the core focus, so
/// the key routes with no leaf, and the transport's deferred candidate fires.
/// The App-side effect announces itself through the shared Status bar
/// projection the frame carries (design D9's status line).
#[test]
fn space_toggles_playback_behind_the_screen() {
    let mut test = test_core_with(stub_app_with_playing_queue(2));
    test.tx
        .send(press(Key::Function(2), KeyModifiers::NONE))
        .unwrap();
    assert!(test.core.step());
    assert!(test.core.gui_settings_section_for_test().is_some());

    test.tx
        .send(press(Key::Char(' '), KeyModifiers::NONE))
        .unwrap();
    assert!(test.core.step());

    let frame = test.latest_frame().expect("the core emitted a frame");
    assert_eq!(
        frame.status.toast.as_ref().map(|toast| toast.text.as_str()),
        Some("Pause requested"),
        "the transport toggle fired behind the screen"
    );
    assert_eq!(
        test.core.gui_settings_section_for_test(),
        Some(mbv::GuiSettingsSection::Services),
        "the screen stays open"
    );
}

/// The clear-queue chord opens no prompt behind the open screen (the spec's
/// *Global keys keep working behind the screen*, the queue-key outcome): the
/// queue-scoped gate reads the screen's keyboard hold and stays closed
/// (`add-gui-settings-screen` design D2). The chord is a rebound
/// `clear_queue_prompt_c`: the GUI-only key context claims the default `c`
/// for the subtitle cycle, so only a rebound chord reaches the queue layer.
#[test]
fn the_clear_queue_chord_opens_no_prompt_behind_the_screen() {
    let mut app = stub_app_with_queue(2);
    app.rebind_chord_for_test("clear_queue_prompt_c", "Shift+C");
    let mut test = test_core_with(app);
    test.tx
        .send(press(Key::Function(2), KeyModifiers::NONE))
        .unwrap();
    assert!(test.core.step());
    assert!(test.core.gui_settings_section_for_test().is_some());

    test.tx
        .send(press(Key::Char('C'), KeyModifiers::SHIFT))
        .unwrap();
    assert!(test.core.step());

    let frame = test.latest_frame().expect("the core emitted a frame");
    assert!(frame.overlay.is_none(), "no clear-queue prompt opened");
    assert_eq!(
        test.core.gui_settings_section_for_test(),
        Some(mbv::GuiSettingsSection::Services),
        "the screen stays open"
    );
}
