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
    app.set_gui_client_capabilities_for_test();
    app.set_panel_focus_for_test(PanelFocus::Queue);
    app.set_panel_mode_for_test(PanelMode::Both);
    // A narrow terminal plus an empty queue arms both of the TUI's rules; the
    // terminal size lives on the shell `Model`.
    let mut model = Model::new(app);
    crate::tui::test_support::set_terminal_width(&mut model, 60);
    let app = &model.app;

    assert!(app.displayed_queue().slots().is_empty());
    assert!(!app.is_mini_view());
    assert!(!app.queue_column_hidden_empty());
    assert_eq!(app.effective_panel_focus(), PanelFocus::Queue);
    assert_eq!(app.effective_panel_mode(), PanelMode::Both);
}

/// The GUI setting drops the TUI launch restore (add-gui-library-panel design
/// D9): the GUI keeps its own launch location, so a saved TUI launch state
/// never moves the GUI's tab or Panel focus. The GUI calls
/// `skip_launch_restore` for that; the capabilities a GUI test sets decide
/// nothing here (design D6a).
#[test]
fn the_gui_setting_starts_with_the_launch_restore_done() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    mbv_config::save_tui_launch_state(&mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::Home,
        panel_focus: mbv_config::LaunchPanelFocus::Queue,
        selector: None,
        item: None,
    })
    .unwrap();
    let mut app = make_app_stub();
    assert!(matches!(
        app.launch_restore_for_test(),
        LaunchRestore::Pending(_)
    ));

    app.skip_launch_restore();

    assert_eq!(app.launch_restore_for_test(), LaunchRestore::Done);
}

/// The card key a playing Movie row resolves to: the landscape key the card
/// fetch issues.
const CARD_KEY: &str = "id:QB";

/// The `App` a Client's test drives with one playing Movie row: the stub queue
/// view holds the item, the player plays its first slot, and card images are
/// on. Built from the `cfg(test)` stubs alone — the plain `-p mbv` test target
/// does not enable the `test` feature, so the exported stubs are not there.
fn playing_movie_app() -> App {
    let mut app = make_app_stub();
    app.seed_local_queue_emby_items_for_test(vec![make_item("Item 0", "Movie")], 0);
    app.seed_playing_status_for_test(0, 1);
    app.enable_card_images_for_test();
    app
}

/// The persisted visual-slot hide is the TUI's (the `hide_visual_slot` chord
/// and the slot's context menu write it, and it survives a relaunch). The GUI
/// window content paints no control for it, so acting on it would leave every
/// now-playing surface painting none of the item's art with no way back (live
/// run of 2026-11-14: the GUI's window showed no now-playing art). The GUI
/// therefore paints the art whatever the stored hide says, and the TUI keeps
/// the hide it saved.
#[test]
fn the_tui_visual_slot_hide_never_hides_the_gui_clients_art() {
    let art = image::DynamicImage::new_rgb8(4, 4);
    let mut gui = playing_movie_app();
    gui.seed_card_image_for_test(CARD_KEY.to_owned(), art.clone());
    gui.seed_visual_slot_hidden_for_test(true);
    gui.set_gui_client_capabilities_for_test();
    let mut tui = playing_movie_app();
    tui.seed_card_image_for_test(CARD_KEY.to_owned(), art.clone());
    tui.seed_visual_slot_hidden_for_test(true);

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

/// The GUI Client never changes or persists the TUI's visual-slot controls: its
/// window content paints no `hide_visual_slot` control and no visualizer, so a
/// routed chord (`h`, `v`) must leave the stored hide and the visualizer alone.
/// The GUI reads prefs a TUI run wrote, so a write here would silently rewrite
/// the TUI's setting; enabling the visualizer would blank the art with nothing
/// painted in its place.
#[test]
fn the_gui_client_leaves_the_tuis_visual_slot_controls_alone() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut tui = playing_movie_app();
    tui.set_visual_slot_hidden_for_test(true);
    let mut gui = playing_movie_app();
    gui.seed_visual_slot_hidden_for_test(true);
    gui.set_gui_client_capabilities_for_test();

    gui.set_visual_slot_hidden_for_test(false);
    gui.set_visualizer_choice_for_test(true);

    assert!(
        gui.visual_slot_hidden_for_test(),
        "the GUI never changes the stored hide"
    );
    assert!(
        !gui.visualizer_owns_the_slot_for_test(),
        "the GUI enables no visualizer"
    );
    assert_eq!(
        App::load_prefs()["visual_slot_hidden"].as_bool(),
        Some(true),
        "the TUI's saved hide is still what the prefs say"
    );
}
