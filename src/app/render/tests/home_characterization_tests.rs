use super::test_helpers::{buffer_to_string, make_movie_app, render_home_shell_with};
use super::*;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::ComponentId;
use crate::app::tests::make_app_stub;
use crate::app::{palette, PanelFocus, TabSelection};

fn home_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::Home;
    app.mini_view_focus = PanelFocus::Library;
    app
}

/// The Continue Watching item the characterization seeds into Model-owned
/// `home_content` (task 5.3d).
fn emby_cw_item() -> mbv_core::api::EmbyItem {
    let movie_app = make_movie_app();
    movie_app.libs[0].nav_stack[0].items[0].clone()
}

/// The mounted `LibraryPanel` (the Home owner's host since task 5.11).
fn panel(model: &crate::app::shell::Model) -> &LibraryPanel {
    model
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
}

/// The startup frame shows Continue's empty-state placeholder while
/// `home_content.loading` is still set, without a Selector row or pill bar.
#[test]
fn startup_frame_paints_loading_affordances_not_blank_panes() {
    let mut app = home_app();
    app.terminal_width = 100;
    app.terminal_height = 30;
    let mut model = crate::app::shell::Model::new(app);
    // The precondition `Model::run` sets before its first `terminal.draw`
    // (`src/app/shell_run.rs`): the Home destination is still loading.
    model.home_content.loading = true;
    model.push_home_content();
    model.sync_mounted_surfaces();

    let backend = ratatui::backend::TestBackend::new(100, 30);
    let mut term = ratatui::Terminal::new(backend).unwrap();
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    let output = buffer_to_string(&term);

    assert!(
        output.split_whitespace().next().is_some(),
        "startup frame must not be an empty buffer"
    );
    assert!(
        output.contains('⌂'),
        "startup frame must paint the Continue tab house glyph: {output:?}"
    );
    assert!(
        panel(&model).test_selector_hits().regions().is_empty(),
        "Continue has no Selector-row pill targets"
    );
    assert!(
        output.contains("Loading"),
        "startup Home pane must paint its loading affordance, not a blank pane: {output:?}"
    );
}

/// The mounted Library panel places no Selector-row geometry or pill targets
/// for Continue at either breakpoint.
#[test]
fn home_pill_row_and_targets_are_characterized_end_to_end() {
    for (width, height) in [(60, 20), (200, 30)] {
        let cw_item = emby_cw_item();
        let (model, _) = render_home_shell_with(home_app(), width, height, |m| {
            m.home_content.continue_items = vec![cw_item];
        });

        let panel = panel(&model);
        assert!(
            panel.test_selector_hits().regions().is_empty(),
            "Continue publishes no Selector-row pill targets at {width}x{height}"
        );
        let (selector_bar, list_panel) = panel
            .test_narrow_geometry()
            .map(|geometry| (geometry.selector_bar, geometry.list_panel))
            .or_else(|| {
                panel
                    .test_wide_geometry()
                    .map(|geometry| (geometry.selector_bar, geometry.list_panel))
            })
            .expect("the Library panel paints a Home skeleton");
        assert_eq!(
            selector_bar.height, 0,
            "Continue places no Selector-row band at {width}x{height}"
        );
        assert_eq!(
            list_panel.y, selector_bar.y,
            "Continue's list starts in the former Selector band at {width}x{height}"
        );
    }
}

/// migrate-home-feeds 4.6 regression, rewritten to the panel output (task
/// 5.11): the browser list stripes like every other library (restored
/// 2026-09-20), so the selected first row paints the bar and the striped row
/// below it carries the library column's fill. Unfocused rows keep the pane
/// fill, the stripe, and paint no bar.
#[test]
fn wide_home_stripes_its_browser_list_with_the_selected_bar() {
    let bgs = |focused: bool| {
        let mut app = home_app();
        if !focused {
            app.panel_focus = PanelFocus::Queue;
        }
        let cw_item = emby_cw_item();
        let mut second = cw_item.clone();
        second.id = "cw-second".into();
        second.name = "Second Continue".into();
        let mut third = cw_item.clone();
        third.id = "cw-third".into();
        third.name = "Third Continue".into();
        let (model, terminal) = render_home_shell_with(app, 160, 40, |m| {
            m.home_content.continue_items = vec![cw_item, second, third];
        });
        let (_, selected) = panel(&model)
            .menu_geometry()
            .expect("wide Home publishes a selected-row rect");
        let selected = selected.expect("wide Home publishes a selected row");
        let buffer = terminal.backend().buffer();
        (
            buffer[(selected.x, selected.y)].style().bg,
            // Grouped under the Continue watching heading, members alternate from
            // the striped secondary fill: member 0 is the selected row (bar),
            // member 1 is plain, member 2 carries the stripe again.
            buffer[(selected.x, selected.y + 2)].style().bg,
        )
    };

    // The browser list stripes: the selected first member paints the bar and
    // the next striped member carries the library column's fill.
    let (selected, plain) = bgs(true);
    assert_eq!(
        selected,
        Some(palette::SELECTED_ROW_BG),
        "the selected first row paints the bar"
    );
    assert_eq!(
        plain,
        Some(palette::surface_colors(palette::Surface::LibraryColumn, true).fill),
        "the next striped member below the selected one carries the stripe"
    );

    let (selected, plain) = bgs(false);
    assert_eq!(
        selected,
        Some(palette::surface_colors(palette::Surface::LibraryColumn, false).fill),
        "an unfocused list paints no bar; the first striped member keeps its stripe"
    );
    assert_eq!(
        plain,
        Some(palette::surface_colors(palette::Surface::LibraryColumn, false).fill),
        "the unfocused striped member below carries the stripe"
    );
}
