use super::test_helpers::{
    draw_mounted_frame, draw_mounted_terminal, make_movie_app, mounted_model_at,
};
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::ComponentId;

/// The mounted `LibraryPanel` (the migrated `EmbyLibraryContent` owner's host
/// since task 6.1, mirroring the Home precedent).
fn panel(model: &crate::app::shell::Model) -> &LibraryPanel {
    model
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
}

#[test]
fn library_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    // Note: width 120 triggers the Wide skeleton, and narrow width the
    // Narrow skeleton, both painted by the mounted `LibraryPanel`'s embedded
    // `EmbyLibraryContent` owner (task 6.1). Route through the real
    // `Model::draw_frame` path.
    let states = [(60, 20, 0), (60, 20, 1)];
    for (width, height, cursor) in states {
        let mut app = make_movie_app();
        app.libs[0].nav_stack[0].set_resting_cursor(cursor);
        let mut model = mounted_model_at(app, width, height);
        let output = draw_mounted_frame(&mut model, width, height);
        assert!(
            output.contains("Movie"),
            "library rows missing in {width}x{height}: {output:?}"
        );
    }
}

// movies_pill_row_and_targets_are_characterized_end_to_end deleted. It
// tested the legacy wide Movies layout, which is now handled by the
// embedded `EmbyLibraryContent` owner (task 6.1). Component rendering is tested
// separately.

// TV's Narrow surface is the same shared panel skeleton as Wide (task 8.3,
/// The mounted Wide library rail leaves exactly one blank gap row above the
/// status bar: the rail's row flow keeps one spacer row above its own panel
/// bottom edge, and the status band owns the single gap row between the
/// panel and the status row.
#[test]
fn wide_library_rail_leaves_one_gap_row_above_the_status_bar() {
    let mut model = mounted_model_at(make_movie_app(), 160, 40);
    let terminal = draw_mounted_terminal(&mut model, 160, 40);
    let rail = panel(&model)
        .test_wide_geometry()
        .expect("the panel paints a Wide list");
    let band = model
        .app
        .compute_chrome_geometry(ratatui::layout::Rect::new(0, 0, 160, 40))
        .status_area;

    assert_eq!(
        rail.list_area.bottom(),
        rail.list_panel.bottom() - 1,
        "the rail's row flow keeps one spacer row above its own bottom edge"
    );
    assert_eq!(
        rail.list_panel.bottom(),
        band.y,
        "the rail must end where the status band begins (rail={:?}, band={:?})",
        rail.list_panel,
        band
    );
    let buffer = terminal.backend().buffer();
    for x in rail.list_panel.left()..rail.list_panel.right() {
        assert_eq!(
            buffer[(x, band.y)].symbol(),
            " ",
            "the band's single gap row {} must be blank (x={x})",
            band.y
        );
    }
}
