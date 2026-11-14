use super::*;
use crate::app::queue_card_artwork::test_source;
use crate::app::state::app_struct::LaunchRestore;

/// The GUI setting (add-gui-library-panel design D4): the GUI arranges its own
/// panels from its measured size and always paints the Queue panel, so neither
/// the TUI's terminal-width Mini check nor its hidden empty queue column
/// applies, and the stored Panel focus and mode stay as the core set them.
/// The TUI's own rule stays
/// (`empty_displayed_queue_derives_library_only_while_stored_mode_stays_both`).
#[test]
fn the_gui_setting_disables_the_tui_panel_rules() {
    let mut app = make_app_stub();
    app.enable_gui_client();
    // A narrow terminal plus an empty queue arms both of the TUI's rules.
    app.terminal_width = 60;
    app.panel_focus = PanelFocus::Queue;
    app.panel_mode = PanelMode::Both;

    assert!(app.displayed_queue().slots().is_empty());
    assert!(!app.is_mini_view());
    assert!(!app.queue_column_hidden_empty());
    assert_eq!(app.effective_panel_focus(), PanelFocus::Queue);
    assert_eq!(app.effective_panel_mode(), PanelMode::Both);
}

/// The GUI setting drops the TUI launch restore (add-gui-library-panel design
/// D9): the GUI keeps its own launch location, so a saved TUI launch state
/// never moves the GUI's tab or Panel focus.
#[test]
fn the_gui_setting_starts_with_the_launch_restore_done() {
    let _guard = crate::config::TestStateDirGuard::new();
    mbv_config::save_tui_launch_state(&mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::Home,
        panel_focus: mbv_config::LaunchPanelFocus::Queue,
        selector: None,
        item: None,
    })
    .unwrap();
    let mut app = make_app_stub();
    assert!(matches!(app.launch_restore, LaunchRestore::Pending(_)));

    app.enable_gui_client();

    assert_eq!(app.launch_restore, LaunchRestore::Done);
}

/// The card key the stub's playing row resolves to: the landscape key a Movie
/// item fetches under.
const CARD_KEY: &str = "id:QB";

/// The persisted visual-slot hide is a TUI setting, and only the TUI acts on
/// it (live run of 2026-11-14: the GUI's window showed no now-playing art).
/// The GUI Client always paints the canvas's still slot — it has no
/// `hide_visual_slot` control to undo a hide — so a `prefs.json` that hid the
/// slot for the TUI must not take the GUI's art away, while the TUI keeps the
/// hide it saved.
#[test]
fn the_tui_visual_slot_hide_never_hides_the_gui_clients_art() {
    let art = image::DynamicImage::new_rgb8(4, 4);
    let mut gui = stub_app_with_playing_queue(1);
    gui.enable_card_images_for_test();
    gui.seed_card_image_for_test(CARD_KEY.to_owned(), art.clone());
    gui.visual_slot_hidden = true;
    gui.enable_gui_client();
    let mut tui = stub_app_with_playing_queue(1);
    tui.enable_card_images_for_test();
    tui.seed_card_image_for_test(CARD_KEY.to_owned(), art.clone());
    tui.visual_slot_hidden = true;

    assert_eq!(
        gui.queue_card_artwork(),
        QueueCardArtwork::Image {
            key: CARD_KEY.to_owned(),
            source: test_source(&art),
        },
        "the GUI paints the playing card's art"
    );
    assert_eq!(
        tui.queue_card_artwork(),
        QueueCardArtwork::Hidden,
        "the TUI keeps the hide it saved"
    );
}
