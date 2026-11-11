//! The Settings screen's core contracts (`add-gui-settings-screen` rows
//! 3.2, 3.3): the open actions open, close, and switch the screen through the
//! core thread's drain (design D2), a start with nothing configured opens
//! Services, and the global keys keep working behind the screen while the
//! queue keys stay behind it (the spec's *Global keys keep working behind the
//! screen*).

use mbv::ClientTickHost;
use mbv::{stub_app_with_playing_queue, stub_app_with_queue};
use mbv_gui::core_thread::CoreInput;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};
use rstest::rstest;

use crate::{TEST_URL, TestCore, test_core_with};

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

// ── Rows 5.1 to 5.6: the command arms ────────────────────────────────────

/// Send one Settings command and run one tick: the Client's production path
/// (`CoreInput::Settings`), which the core thread's input phase applies.
fn send_settings(test: &mut TestCore, command: mbv::GuiSettingsCommand) {
    test.tx.send(CoreInput::Settings(command)).unwrap();
    assert!(test.core.step(), "the core thread never quits mid-command");
}

/// Send one key press and run one tick.
fn send_key(test: &mut TestCore, key: Key, modifiers: KeyModifiers) {
    test.tx.send(press(key, modifiers)).unwrap();
    assert!(test.core.step(), "the core thread never quits mid-command");
}

/// Open the Settings screen through F2 (the `settings_open` action) and then
/// show `section`, so a command addresses an open screen at that section.
fn open_settings_at(test: &mut TestCore, section: mbv::GuiSettingsSection) {
    send_key(test, Key::Function(2), KeyModifiers::NONE);
    assert!(test.core.gui_settings_section_for_test().is_some());
    send_settings(test, mbv::GuiSettingsCommand::SelectSection { section });
    assert_eq!(test.core.gui_settings_section_for_test(), Some(section));
}

/// The newest frame the core emitted.
fn newest_frame(test: &TestCore) -> mbv_gui::bridge::frame::GuiFrame {
    test.latest_frame().expect("the core emitted a frame")
}

/// The Settings content the newest frame carries.
fn settings_content(
    frame: &mbv_gui::bridge::frame::GuiFrame,
) -> &mbv_gui::bridge::frame::settings::GuiSettingsContent {
    &frame
        .settings
        .as_ref()
        .expect("the frame carries the Settings screen")
        .content
}

/// One Keys row of the frame by action id: its effective chords.
fn key_row_chords(frame: &mbv_gui::bridge::frame::GuiFrame, action: &str) -> Vec<String> {
    let mbv_gui::bridge::frame::settings::GuiSettingsContent::Keys { rows, .. } =
        settings_content(frame)
    else {
        panic!("the Keys section carries Keys content");
    };
    rows.iter()
        .find(|row| row.id == action)
        .map(|row| row.chords.clone())
        .unwrap_or_default()
}

/// An `activate` of a toggle runs the shared activation: the configuration
/// changes and the deferred save is armed (the spec's *Toggling by key*).
#[test]
fn an_activate_of_a_toggle_changes_the_config_and_arms_the_save() {
    use mbv_ui_model::settings::SettingKey;

    let _guard = mbv_config::TestStateDirGuard::new();
    let mut test = test_core_with(stub_app_with_queue(2));
    open_settings_at(&mut test, mbv::GuiSettingsSection::Queue);
    let before = test
        .core
        .app_mut()
        .gui_settings_config_for_test()
        .always_play_next;

    send_settings(
        &mut test,
        mbv::GuiSettingsCommand::Activate {
            setting: SettingKey::AlwaysPlayNext,
        },
    );

    let app = test.core.app_mut();
    assert_eq!(
        app.gui_settings_config_for_test().always_play_next,
        !before,
        "the toggle flipped the stored flag"
    );
    assert!(
        app.gui_settings_save_armed_for_test(),
        "the accepted activation armed the deferred save"
    );
}

/// An `activate` of a setting whose scope is not `AllClients` is refused: the
/// TUI-only mouse support row changes neither the configuration nor the save
/// (design D4's client scope, design D5's refusal).
#[test]
fn an_activate_of_mouse_support_changes_nothing() {
    use mbv_ui_model::settings::SettingKey;

    let _guard = mbv_config::TestStateDirGuard::new();
    let mut test = test_core_with(stub_app_with_queue(2));
    open_settings_at(&mut test, mbv::GuiSettingsSection::Services);
    let before = test
        .core
        .app_mut()
        .gui_settings_config_for_test()
        .mouse_support;

    send_settings(
        &mut test,
        mbv::GuiSettingsCommand::Activate {
            setting: SettingKey::MouseSupport,
        },
    );

    let app = test.core.app_mut();
    assert_eq!(
        app.gui_settings_config_for_test().mouse_support,
        before,
        "a TUI-only setting is refused"
    );
    assert!(
        !app.gui_settings_save_armed_for_test(),
        "a refused activation arms no save"
    );
}

/// An `embySetup` command fills the `App`'s setup form with the carried
/// fields and submits it: a setup generation starts for the carried server
/// URL (design D5).
#[test]
fn an_emby_setup_command_starts_a_generation_with_the_carried_server_url() {
    let _guard = mbv_config::TestStateDirGuard::new();
    let mut test = test_core_with(stub_app_with_queue(2));
    open_settings_at(&mut test, mbv::GuiSettingsSection::Services);

    send_settings(
        &mut test,
        mbv::GuiSettingsCommand::EmbySetup {
            server_url: TEST_URL.to_string(),
            username: "user".to_string(),
            password: "secret".to_string(),
        },
    );

    let app = test.core.app_mut();
    assert_eq!(
        app.gui_setup_server_url_for_test().as_deref(),
        Some(TEST_URL),
        "the form carries the command's server URL"
    );
    assert!(
        app.gui_setup_busy_for_test(),
        "the validation is in progress"
    );
    assert!(
        app.gui_setup_generation_started_for_test(),
        "the submit started a setup generation"
    );
}

/// A `serviceAction` of `ManageFeeds` shows the Feeds section and raises no
/// TUI overlay request (design D5's exception): routing it would raise
/// `OpenFeedsManage`, which the GUI presents nothing for.
#[test]
fn a_service_action_of_manage_feeds_shows_the_feeds_section() {
    use mbv_ui_model::settings::ServiceActionIntent;

    let _guard = mbv_config::TestStateDirGuard::new();
    let mut test = test_core_with(stub_app_with_queue(2));
    open_settings_at(&mut test, mbv::GuiSettingsSection::Services);

    send_settings(
        &mut test,
        mbv::GuiSettingsCommand::ServiceAction {
            action: ServiceActionIntent::ManageFeeds,
        },
    );

    assert_eq!(
        test.core.gui_settings_section_for_test(),
        Some(mbv::GuiSettingsSection::Feeds),
        "the screen shows the Feeds section"
    );
    assert!(
        test.core.app_mut().pending_overlay_is_none_for_test(),
        "no TUI overlay request is left for the GUI to drop"
    );
}

/// A `serviceAction` of `RemoveEmby` raises the TUI's removal confirmation
/// (design D5): the Services section offers every action the TUI offers, so
/// the Emby row reaches the same confirm the TUI's `d` key does.
#[test]
fn a_remove_emby_action_raises_the_removal_confirm() {
    use mbv_ui_model::settings::ServiceActionIntent;

    let _guard = mbv_config::TestStateDirGuard::new();
    let mut test = test_core_with(stub_app_with_queue(2));
    test.core
        .app_mut()
        .set_gui_emby_service_state_for_test(mbv_core::service_runtime::ServiceState::Ready);
    open_settings_at(&mut test, mbv::GuiSettingsSection::Services);

    send_settings(
        &mut test,
        mbv::GuiSettingsCommand::ServiceAction {
            action: ServiceActionIntent::RemoveEmby,
        },
    );

    match newest_frame(&test).overlay {
        Some(mbv_gui::bridge::frame::overlay::GuiOverlayFrame::Confirm { message, .. }) => assert!(
            message.contains("Remove Emby"),
            "the confirmation asks about Emby: {message}"
        ),
        overlay => panic!("the removal confirmation was not raised: {overlay:?}"),
    }
}

/// A `feedEdit` for an unknown URL changes nothing: the subscriptions are
/// addressed by URL, and an unmatched one has no row to edit (design D5).
#[test]
fn a_feed_edit_for_an_unknown_url_changes_nothing() {
    let _guard = mbv_config::TestStateDirGuard::new();
    let mut app = stub_app_with_queue(2);
    app.seed_gui_feed_subscriptions_for_test(&[("A", "https://a")]);
    let mut test = test_core_with(app);
    open_settings_at(&mut test, mbv::GuiSettingsSection::Feeds);

    send_settings(
        &mut test,
        mbv::GuiSettingsCommand::FeedEdit {
            url: "https://unknown".to_string(),
            name: "Renamed".to_string(),
            kind: mbv_queue::FeedKind::Video,
        },
    );

    let feeds = test.core.app_mut().gui_settings_feeds().subscriptions;
    assert_eq!(feeds.len(), 1, "no subscription was added");
    assert_eq!(feeds[0].name, "A", "the known subscription is untouched");
    assert_eq!(feeds[0].url, "https://a");
}

/// A drained successful add result appends the subscription the worker
/// fetched (row 5.3, design D7): the Client's tick host calls the `App` drain,
/// so the saved list grows by one.
#[test]
fn an_injected_feed_add_result_appends_the_subscription() {
    let _guard = mbv_config::TestStateDirGuard::new();
    let mut app = stub_app_with_queue(2);
    app.inject_gui_feed_add_result_for_test("Added", "https://added", mbv_queue::FeedKind::Video);
    let mut test = test_core_with(app);
    open_settings_at(&mut test, mbv::GuiSettingsSection::Feeds);

    test.tx.send(CoreInput::Wake).unwrap();
    assert!(test.core.step());

    let feeds = test.core.app_mut().gui_settings_feeds().subscriptions;
    assert_eq!(feeds.len(), 1, "the applied add appended one subscription");
    assert_eq!(feeds[0].name, "Added");
    assert_eq!(feeds[0].url, "https://added");
}

/// A capture to a free chord is accepted at once: the frame's row carries it,
/// `[keys]` holds it, and the next press of that chord fires the action (the
/// spec's *A rebind fires at once*).
#[test]
fn a_capture_to_a_free_chord_fires_the_action_on_the_next_key() {
    let _guard = mbv_config::TestStateDirGuard::new();
    let mut test = test_core_with(stub_app_with_playing_queue(2));
    open_settings_at(&mut test, mbv::GuiSettingsSection::Keys);

    send_settings(
        &mut test,
        mbv::GuiSettingsCommand::KeysCapture {
            target: "toggle_play_pause".to_string(),
            key: KeyEvent::new(Key::Char('j'), KeyModifiers::CONTROL),
        },
    );
    let frame = newest_frame(&test);
    assert_eq!(
        key_row_chords(&frame, "toggle_play_pause"),
        vec!["Ctrl+j".to_string()],
        "the row shows the accepted chord"
    );
    assert_eq!(
        mbv::load_config()
            .expect("the configuration the save wrote loads")
            .keybinds
            .router_override("toggle_play_pause")
            .map(|chord| chord.to_string()),
        Some("Ctrl+j".to_string()),
        "`[keys]` holds the edit at once"
    );

    send_key(&mut test, Key::Char('j'), KeyModifiers::CONTROL);

    let frame = newest_frame(&test);
    assert_eq!(
        frame.status.toast.as_ref().map(|toast| toast.text.as_str()),
        Some("Pause requested"),
        "the captured chord fired its action"
    );
}

/// A capture of another action's default chord is refused, the refusal names
/// that action, and the bindings are unchanged (the spec's *A collision names
/// the other action*).
#[test]
fn a_capture_to_another_actions_default_is_refused_naming_that_action() {
    let _guard = mbv_config::TestStateDirGuard::new();
    let mut test = test_core_with(stub_app_with_playing_queue(2));
    open_settings_at(&mut test, mbv::GuiSettingsSection::Keys);

    send_settings(
        &mut test,
        mbv::GuiSettingsCommand::KeysCapture {
            target: "toggle_mute".to_string(),
            // `stop`'s declared default chord.
            key: KeyEvent::new(Key::Char('S'), KeyModifiers::SHIFT),
        },
    );

    let frame = newest_frame(&test);
    assert_eq!(
        key_row_chords(&frame, "toggle_mute"),
        vec!["m".to_string()],
        "the refused edit left the binding as it was"
    );
    let mbv_gui::bridge::frame::settings::GuiSettingsContent::Keys { refusal, .. } =
        settings_content(&frame)
    else {
        panic!("the Keys section carries Keys content");
    };
    let refusal = refusal.as_ref().expect("the refusal rides the frame");
    assert_eq!(
        refusal.action, "toggle_mute",
        "the refusal names the edited action"
    );
    assert!(
        refusal.text.contains("stop"),
        "the refusal names the action that owns the chord: {}",
        refusal.text
    );
}

/// The Keys section lists no action without a GUI effect (design D4): the
/// TUI-only `help_open` has no row, so the screen offers no chord the GUI
/// would silently ignore.
#[test]
fn the_keys_frame_lists_no_tui_only_action() {
    let mut test = test_core_with(stub_app_with_queue(2));
    open_settings_at(&mut test, mbv::GuiSettingsSection::Keys);

    let frame = newest_frame(&test);
    let mbv_gui::bridge::frame::settings::GuiSettingsContent::Keys { rows, .. } =
        settings_content(&frame)
    else {
        panic!("the Keys section carries Keys content");
    };
    assert!(
        rows.iter().all(|row| row.id != "help_open"),
        "no TUI-only action is listed"
    );
}

/// A `selectTarget` command selects through the chooser's path and leaves the
/// screen open (the spec's *Selecting a cast receiver*).
#[test]
fn a_select_target_leaves_the_screen_open() {
    let _guard = mbv_config::TestStateDirGuard::new();
    let mut test = test_core_with(stub_app_with_playing_queue(2));
    open_settings_at(&mut test, mbv::GuiSettingsSection::Sessions);

    send_settings(
        &mut test,
        mbv::GuiSettingsCommand::SelectTarget {
            target: mbv_ui_model::panel_targets::ChooserTargetKey::Local,
        },
    );

    assert_eq!(
        test.core.gui_settings_section_for_test(),
        Some(mbv::GuiSettingsSection::Sessions),
        "the screen stays open"
    );
}

/// A `playlistAction` of play replaces the empty queue with the playlist's
/// items, and the drained `DismissSidebar(Playlists)` closes the screen so the
/// replaced queue shows (row 5.5, design D2).
#[test]
fn a_playlist_play_on_an_empty_queue_closes_the_screen() {
    let _guard = mbv_config::TestStateDirGuard::new();
    let (app, cmd_rx) = mbv::stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    open_settings_at(&mut test, mbv::GuiSettingsSection::Playlists);
    let http = mbv_net::mock_http::MockHttp::new();
    http.respond_when(
        "/Playlists/pl/Items",
        200,
        r#"{"Items":[{"Id":"a","Name":"One","Type":"Audio","MediaType":"Audio"}],"TotalRecordCount":1}"#,
    );
    let app = test.core.app_mut();
    app.set_gui_emby_client_for_test(crate::test_emby_client(&http));
    app.seed_gui_playlists_for_test(&[("pl", "Road Trip")]);

    send_settings(
        &mut test,
        mbv::GuiSettingsCommand::PlaylistAction {
            id: "pl".to_string(),
            action: mbv_ui_msg::MusicTreeAction::Play,
        },
    );

    let replaces: Vec<(usize, mbv_queue::QueueSource)> = cmd_rx
        .try_iter()
        .filter_map(|command| match command {
            mbv_ctrl::CtrlCmd::UnifiedQueueReplace { items, source, .. } => {
                Some((items.len(), source))
            }
            _ => None,
        })
        .collect();
    assert_eq!(replaces.len(), 1, "one queue replacement was sent");
    assert_eq!(replaces[0].0, 1, "the playlist's one playable item");
    assert!(
        matches!(&replaces[0].1, mbv_queue::QueueSource::Playlist { id: Some(id), .. } if id == "pl"),
        "the replacement names the playlist: {:?}",
        replaces[0].1
    );
    assert_eq!(
        test.core.gui_settings_section_for_test(),
        None,
        "the drained dismissal closed the screen"
    );
}

/// The reset returns the GUI to Home with no kept collection, expands the
/// panel, leaves the queue's slots alone, rises the reset counter, and deletes
/// the saved launch location (design D10, the spec's *Reset returns to
/// Home*).
#[test]
fn a_reset_returns_to_home_and_clears_the_launch_location() {
    let _guard = mbv_config::TestStateDirGuard::new();
    let mut app = stub_app_with_queue(2);
    app.seed_gui_music_library_tab_for_test();
    app.seed_gui_library_groups_for_test(0, "grp-soul");
    mbv_config::save_gui_launch_state(&mbv_config::first_launch())
        .expect("the saved launch location is written");
    let epoch_before = app.gui_ui_reset_epoch();
    let mut test = test_core_with(app);
    open_settings_at(&mut test, mbv::GuiSettingsSection::Actions);
    send_key(&mut test, Key::Char('z'), KeyModifiers::NONE);
    send_key(&mut test, Key::Char('q'), KeyModifiers::NONE);
    let toggles = test.core.app_mut().panel_toggles_projection();
    assert!(
        toggles.collapsed && toggles.queue_folded,
        "the toggles were on before the reset"
    );

    send_settings(&mut test, mbv::GuiSettingsCommand::ResetUiState);

    let frame = newest_frame(&test);
    assert!(frame.settings.is_none(), "the reset closed the screen");
    assert_eq!(frame.queue.rows.len(), 2, "the queue's slots are unchanged");
    assert_eq!(
        frame.ui_reset_epoch,
        epoch_before + 1,
        "the reset counter rose"
    );
    let app = test.core.app_mut();
    assert_eq!(
        app.tab_for_test(),
        mbv_ui_model::tab_selection::TabSelection::Home,
        "the Library shows Home"
    );
    assert!(
        app.gui_library_groups_for_test(0).is_none(),
        "no kept collection survives the reset"
    );
    let toggles = app.panel_toggles_projection();
    assert!(
        !toggles.collapsed && !toggles.queue_folded,
        "the panel toggles are off"
    );
    assert!(
        !mbv_config::gui_launch_state_path().exists(),
        "the saved launch location is gone"
    );
}
