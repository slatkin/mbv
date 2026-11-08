use super::*;
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
