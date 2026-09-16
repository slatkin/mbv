use super::test_helpers::{
    buffer_to_string, draw_mounted_frame, make_music_group_app, mounted_model_at,
};
use super::*;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

/// Narrow grouped Music is painted by the mounted `MusicWorkspaceComponent`
/// now (task 3.8), so route the narrow characterization renders through the
/// real `Model::draw_frame` shell path.
fn render_narrow_music(app: App, width: u16, height: u16) -> String {
    let mut model = mounted_model_at(app, width, height);
    draw_mounted_frame(&mut model, width, height)
}

fn render_music_legacy(app: &mut App, width: u16, height: u16, _focused: bool) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = Rect::default();
    terminal
        .draw(|f| {
            app.reserve_library_area(f, Rect::new(0, 0, width, height), &mut layout, None);
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn music_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    // Wide grouped Music: the legacy base frame is now geometry-only — the
    // mounted `MusicWorkspaceComponent` is the sole painter (#613), so
    // `render_library` paints no grouped-album rows at the wide breakpoint.
    for (width, height, focused) in [(120, 30, true), (120, 30, false)] {
        let mut app = make_music_group_app();
        app.libs[0].nav_stack[1].set_resting_cursor(0);
        let output = render_music_legacy(&mut app, width, height, focused);
        assert!(
            !output.contains("First Album"),
            "wide legacy frame must not paint music rows in {width}x{height}: {output:?}"
        );
    }

    // Narrow grouped Music is painted by the mounted `MusicWorkspaceComponent`
    // now (task 3.8), reached through the full `Model::draw_frame` path.
    for (width, height) in [(60, 30), (60, 20)] {
        let mut app = make_music_group_app();
        app.libs[0].nav_stack[1].set_resting_cursor(0);
        let output = render_narrow_music(app, width, height);
        assert!(
            output.contains("First Album"),
            "narrow music row missing in {width}x{height}: {output:?}"
        );
    }
}

#[test]
fn narrow_grouped_music_shows_group_pill_bar() {
    // Task 3.6a: the narrow branch reserves a group pill row above the album
    // rows, mirroring narrow TV and the narrow browser. Before the fix the
    // narrow branch rendered album rows straight into the full area and never
    // published `selector_tabs`.
    let app = make_music_group_app();
    let mut model = mounted_model_at(app, 60, 30);
    let output = draw_mounted_frame(&mut model, 60, 30);
    let selector_tabs = model
        .application
        .get_component(&crate::app::components::ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .expect("Library panel mounted")
        .test_selector_hits()
        .regions()
        .to_vec();
    assert!(
        !selector_tabs.is_empty(),
        "narrow grouped Music must publish group selector pills:\n{output}"
    );
    assert!(
        output.contains("Beta"),
        "group pill labels must paint:\n{output}"
    );
}
