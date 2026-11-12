//! The Settings screen's core contracts (`add-gui-settings-screen` rows
//! 3.2, 3.3): the open actions open, close, and switch the screen through the
//! core thread's drain (design D2), a start with nothing configured opens
//! Services, and the global keys keep working behind the screen while the
//! queue keys stay behind it (the spec's *Global keys keep working behind the
//! screen*).
//!
//! The `settings` command arms and the frame they answer (rows 5.1 to 5.6 and
//! 9.2) live in `settings/commands.rs`, which resolves under
//! `tests/core_thread/settings/` (one binary per crate; no `mod.rs`).

use mbv::ClientTickHost;
use mbv::{stub_app_with_playing_queue, stub_app_with_queue};
use mbv_gui::bridge::layout::{QueueTargetId, RegionTarget};
use mbv_gui::core_thread::CoreInput;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};
use rstest::rstest;

use crate::{
    TEST_URL, TestCore, layout_report, measured_layout, press_at, region_at, test_core,
    test_core_with,
};

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

/// The top bar's Settings control (design D11 *Mouse-centric*, the spec's *A
/// Settings control in the top bar opens the screen*): the `open` command
/// opens a closed screen at Services, as the `settings_open` action does, and
/// a second `open` on the showing screen changes nothing — the shown section
/// stays.
#[test]
fn the_open_command_opens_at_services_and_leaves_an_open_screen_alone() {
    let mut test = test_core_with(stub_app_with_queue(2));

    test.tx
        .send(CoreInput::Settings(mbv::GuiSettingsCommand::Open))
        .unwrap();
    assert!(test.core.step());

    assert_eq!(
        test.core.gui_settings_section_for_test(),
        Some(mbv::GuiSettingsSection::Services),
        "the closed screen opened at Services"
    );
    assert!(
        test.next_frame().settings.is_some(),
        "the frame carries the open screen"
    );

    test.tx
        .send(CoreInput::Settings(
            mbv::GuiSettingsCommand::SelectSection {
                section: mbv::GuiSettingsSection::Queue,
            },
        ))
        .unwrap();
    assert!(test.core.step());
    assert_eq!(
        test.core.gui_settings_section_for_test(),
        Some(mbv::GuiSettingsSection::Queue)
    );

    test.tx
        .send(CoreInput::Settings(mbv::GuiSettingsCommand::Open))
        .unwrap();
    assert!(test.core.step());

    assert_eq!(
        test.core.gui_settings_section_for_test(),
        Some(mbv::GuiSettingsSection::Queue),
        "the open screen stayed where it was"
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

/// A press at a former queue-row point runs no queue action while the screen
/// is open (the spec's *A press does not act behind the screen*, design D11
/// *Pointer contract*): the Queue panel reported its rows, then unmounted and
/// retracted its regions, so the hit map holds nothing at that point and the
/// press resolves to no core surface. The screen stays open and the queue
/// cursor stays where it was.
#[test]
fn a_press_at_a_retracted_queue_row_runs_no_queue_action() {
    let mut test = test_core(3);
    // The mounted Queue panel reported its rows: the point is slot 2's row.
    test.tx
        .send(measured_layout(
            3,
            vec![region_at(
                0.0,
                0.0,
                RegionTarget::Queue(QueueTargetId::Slot(2)),
            )],
        ))
        .unwrap();
    assert!(test.core.step());
    test.next_frame();

    test.tx
        .send(press(Key::Function(2), KeyModifiers::NONE))
        .unwrap();
    assert!(test.core.step());
    assert!(test.next_frame().settings.is_some(), "the screen opened");

    // The screen unmounted the panels, so the front end reported no regions.
    test.tx.send(layout_report()).unwrap();
    assert!(test.core.step());
    let before = test.next_frame().queue.cursor;

    test.tx.send(press_at(50.0, 10.0)).unwrap();
    assert!(test.core.step());

    let frame = test.next_frame();
    assert_eq!(
        frame.queue.cursor, before,
        "the press at the former row point ran no queue action"
    );
    assert!(frame.settings.is_some(), "the screen stays open");
    assert_eq!(
        test.core.gui_settings_section_for_test(),
        Some(mbv::GuiSettingsSection::Services),
        "the Settings focus stays on the shown section"
    );
}

mod commands;
