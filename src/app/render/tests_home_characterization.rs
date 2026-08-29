use super::test_helpers::{
    buffer_to_string, make_movie_app, render_app_to_terminal, render_home_shell_with,
};
use super::*;
use crate::app::components::{ComponentId, HomeComponent};
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

/// Task 5.3d, Home legacy underpaint removal — regression: the legacy base
/// frame (`App::render`) no longer paints any Home content before the
/// mounted component view runs. It still reserves the full Home destination
/// area (`home_area`) as the placement handoff, but paints no Home rows,
/// pills, or hero there. Home content is Model-owned now (task 5.3d), so
/// the legacy frame never even holds a copy to (not) paint.
#[test]
fn legacy_base_frame_does_not_paint_home_content_before_the_component() {
    let mut app = home_app();
    app.terminal_width = 60;
    app.terminal_height = 20;
    let terminal = render_app_to_terminal(&mut app, 60, 20);
    assert!(
        app.layout.main.home_area.height > 0,
        "legacy frame must still reserve home_area: {:?}",
        app.layout.main.home_area
    );
    let output = buffer_to_string(&terminal);
    assert!(
        !output.contains("Focused Movie"),
        "legacy frame must not paint Home rows/hero before the component: {output:?}"
    );
}

/// Task 5.3d, Home legacy underpaint removal: this characterization now
/// renders through the mounted `HomeComponent` (via the shell-equivalent
/// `render_home_shell` helper) instead of the legacy `App`-only frame, which
/// no longer paints Home content at all. The behavioral assertion — each
/// width/focused state still paints the selected movie's hero/list — is
/// unchanged.
#[test]
fn home_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    let states = [
        (120, 40, true),
        (120, 40, false),
        (60, 40, true),
        (60, 12, true),
    ];
    for (width, height, focused) in states {
        let mut app = home_app();
        if !focused {
            app.panel_focus = PanelFocus::Queue;
        }
        let cw_item = emby_cw_item();
        let (_model, terminal) = render_home_shell_with(app, width, height, |m| {
            m.home_content.continue_items = vec![cw_item];
        });
        let output = buffer_to_string(&terminal);
        assert!(
            output.contains("Focused Movie"),
            "home hero/list missing in {width}x{height}: {output:?}"
        );
    }
}

/// `remove-migrated-surface-underpaint` D3 + the "Startup content" risk
/// bullet: task 2.4 routes the two startup `terminal.draw` sites in
/// `Model::run` (`src/app/shell_run.rs`) through `Model::draw_frame`, so the
/// first frame now paints the full base frame *and* the mounted component
/// views — not the old chrome-only flash. This characterizes that the startup
/// Home frame shows the mounted `HomeComponent`'s loading affordances (its
/// painted pill bar and empty-state placeholder while home_content.loading is
/// still set and no content has arrived) rather than blank panes.
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

    let backend = ratatui::backend::TestBackend::new(100, 30);
    let mut term = ratatui::Terminal::new(backend).unwrap();
    term.draw(|f| model.draw_frame(f, false, false)).unwrap();
    let output = buffer_to_string(&term);

    assert!(
        output.split_whitespace().next().is_some(),
        "startup frame must not be an empty buffer"
    );
    assert!(
        output.contains("Continue"),
        "startup frame must paint the mounted HomeComponent's pill bar, not \
         just legacy chrome: {output:?}"
    );
    assert!(
        output.contains("(empty)"),
        "startup Home pane must paint its empty-state placeholder, not a \
         blank pane: {output:?}"
    );
}

/// Task 5.3d, Home legacy underpaint removal: the pill targets are now
/// characterized from the single painter — the mounted `HomeComponent`'s
/// own `pill_targets` — rather than `LayoutMain.selector_tabs`, which the
/// legacy frame no longer populates for Home. The assertions are preserved:
/// one Continue-Watching pill (id 0), the targets share one painted row, the
/// selected pill is highlighted, and exactly one pill bar row is painted.
#[test]
fn home_pill_row_and_targets_are_characterized_end_to_end() {
    let cw_item = emby_cw_item();
    let (model, terminal) = render_home_shell_with(home_app(), 60, 20, |m| {
        m.home_content.continue_items = vec![cw_item];
    });

    let home = model
        .application
        .get_component(&ComponentId::Home)
        .expect("Home component mounted")
        .as_any()
        .downcast_ref::<HomeComponent>()
        .expect("Home component type");
    let targets = home.test_pill_targets();
    assert_eq!(
        targets.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        vec![0],
        "Home pill targets"
    );
    let first = targets.first().expect("Home should publish pill targets").0;
    assert!(
        targets
            .iter()
            .all(|(rect, _)| rect.y == first.y && rect.height == 1),
        "pill hitboxes must occupy one shared row: {targets:?}"
    );

    let buffer = terminal.backend().buffer();
    let selected = targets
        .iter()
        .find(|(_, id)| *id == 0)
        .expect("selected pill id should have a hitbox")
        .0;
    assert_eq!(
        buffer[(selected.x + 1, selected.y)].style().bg,
        Some(palette::PILL_SELECTED_BG),
        "selected pill appearance"
    );
    let row_text = (0..buffer.area().width)
        .map(|x| buffer[(x, first.y)].symbol())
        .collect::<String>();
    assert!(
        row_text.contains("⌘"),
        "pill row missing glyph: {row_text:?}"
    );
    assert!(
        row_text.contains("Continue"),
        "pill row missing label: {row_text:?}"
    );
    let pill_rows = (0..buffer.area().height)
        .filter(|y| buffer[(first.x, *y)].symbol() == "◢")
        .collect::<Vec<_>>();
    assert_eq!(
        pill_rows,
        vec![first.y],
        "Home must paint exactly one pill bar row"
    );
}
