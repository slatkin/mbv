use super::test_helpers::{assert_surface_pills, buffer_to_string, make_movie_app};
use super::*;
use crate::app::tests::make_app_stub;
use crate::app::{PanelFocus, TabSelection};

fn home_app() -> App {
    let mut app = make_app_stub();
    let movie_app = make_movie_app();
    app.home.continue_items = vec![movie_app.libs[0].nav_stack[0].items[0].clone()];
    app.tab = TabSelection::Home;
    app.mini_view_focus = PanelFocus::Library;
    app
}

#[test]
fn home_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    let states = [
        (120, 40, true),
        (120, 40, false),
        (60, 40, true),
        (60, 8, true),
    ];
    for (width, height, focused) in states {
        let mut app = home_app();
        if !focused {
            app.panel_focus = PanelFocus::Queue;
        }
        let (terminal, _) = super::test_helpers::render_view_to_terminal(&mut app, width, height);
        let output = buffer_to_string(&terminal);
        assert!(
            output.contains("Focused Movie"),
            "home hero/list missing in {width}x{height}: {output:?}"
        );
    }
}

#[test]
fn home_pill_row_and_targets_are_characterized_end_to_end() {
    let mut app = home_app();
    let (terminal, layout) = super::test_helpers::render_view_to_terminal(&mut app, 60, 20);

    assert_surface_pills(
        &terminal,
        &layout,
        Rect::new(0, 0, 60, 20),
        1,
        palette::SURFACE_BACKDROP,
        &[0],
        &["⌘", "Continue"],
        0,
    );
}
