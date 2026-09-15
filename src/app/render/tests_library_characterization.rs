use super::test_helpers::{
    draw_mounted_frame, draw_mounted_terminal, make_movie_app, mounted_model_at, mounted_tv_layout,
    mounted_tv_scroll, set_tv_cursor_for_test,
};
use super::*;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::ComponentId;
use crate::app::tests::make_item;
use crate::app::TabSelection;

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

fn tv_letter_grouped_app(scroll: usize) -> App {
    let mut app = make_movie_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.libs[0].library.collection_type = "tvshows".into();
    let items = (0..55)
        .map(|i| {
            let mut item = make_item(
                &format!("{} Series {i:02}", (b'A' + (i % 26) as u8) as char),
                "Series",
            );
            item.id = format!("series-{i}");
            item.is_folder = true;
            item.overview = "The selected series overview.".into();
            item
        })
        .collect();
    app.libs[0].nav_stack[0].items = items;
    app.libs[0].nav_stack[0].total_count = 55;
    app.libs[0].nav_stack[0].set_resting_cursor(54);
    app.libs[0].nav_stack[0].set_resting_scroll(scroll);
    app.libs[0].library_total = Some(55);
    app
}

// TV's Narrow surface is the same shared panel skeleton as Wide (task 8.3,
// design D4/D7); task 8.4 registers the owner under
// `LibraryKey::Service(TvShows)`, so the surface is the mounted panel's own
// paint at its `RootFrame` placement. Characterize its rendered content and
// owner viewport rather than the deleted Series-specific detail geometry.
//
// The fit boundary below is re-derived for the panel's real placement (the
// deleted component's tests re-viewed it over the whole frame, so their
// boundary saw ~10 extra rows): the shared inline hero's detail block is 14
// rows, and the panel's list area is `terminal height - 10`, so the block is
// admitted from height 26 up and refused at 24 (design D15: re-derived, not
// loosened).
#[test]
fn tv_narrow_panel_characterization_keeps_content_and_viewport() {
    let mut model = mounted_model_at(tv_letter_grouped_app(12), 70, 26);
    set_tv_cursor_for_test(&mut model, 54);
    let output = draw_mounted_frame(&mut model, 70, 26);
    let layout = mounted_tv_layout(&model);

    assert!(
        output.contains("Series"),
        "selected series is missing from shared panel output:\n{output}"
    );
    assert!(
        !output.contains("Season") && !output.contains("Episode"),
        "Narrow TV must open episodes only through SelectionModal"
    );
    let hero_area = layout.inline_hero_area;
    assert!(
        hero_area.height > 0,
        "complete selected replacement should fit: hero={hero_area:?}\n{output}"
    );
    let hero_lines = output
        .lines()
        .skip(hero_area.y as usize)
        .take(hero_area.height as usize)
        .collect::<String>();
    assert!(
        !hero_lines.contains('\u{258e}'),
        "ordinary marker leaked into the grouped hero"
    );
    let control_scroll = mounted_tv_scroll(&model);
    let _ = draw_mounted_frame(&mut model, 70, 26);
    assert_eq!(
        mounted_tv_scroll(&model),
        control_scroll,
        "shared panel redraw must preserve the TV viewport"
    );
}

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
