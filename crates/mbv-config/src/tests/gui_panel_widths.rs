use crate::TestTempDir;
use crate::gui_panel_widths::{GuiPanelWidths, load_gui_panel_widths_at, save_gui_panel_widths_at};

// Focused filesystem tests for the GUI's dragged panel widths
// (`gui_panel_widths.rs`). Hermetic by construction: every test writes
// through the explicit-path `_at` variants into fresh `TestTempDir` scratch
// directories (uuid-qualified, removed on drop), so no test touches the real
// state dir or a sibling test's files.

/// The widths a Music session dragged on both dividers leaves.
fn both_dragged() -> GuiPanelWidths {
    GuiPanelWidths {
        queue: Some(500),
        list: Some(420),
    }
}

#[test]
fn gui_panel_widths_round_trips_a_dragged_width() {
    let scratch = TestTempDir::new();
    let path = scratch.join("gui-panel-widths.toml");
    let widths = both_dragged();

    save_gui_panel_widths_at(&path, &widths);

    assert_eq!(load_gui_panel_widths_at(&path), widths);
}

#[test]
fn gui_panel_widths_missing_file_loads_the_defaults() {
    let scratch = TestTempDir::new();
    let path = scratch.join("gui-panel-widths.toml");

    assert_eq!(load_gui_panel_widths_at(&path), GuiPanelWidths::default());
}

/// D14: an unreadable (here: unparseable) file never blocks the window —
/// both columns read as `None` and the defaults apply.
#[test]
fn gui_panel_widths_unreadable_file_loads_the_defaults() {
    let scratch = TestTempDir::new();
    let path = scratch.join("gui-panel-widths.toml");
    std::fs::write(&path, "{not toml").unwrap();

    assert_eq!(load_gui_panel_widths_at(&path), GuiPanelWidths::default());
}
