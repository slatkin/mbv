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

/// Task 5.3d + 5.11: the startup frame shows the Home owner's loading
/// affordances (its Selector row's pill bar and the empty-state placeholder
/// while home_content.loading is still set and no content has arrived)
/// rather than blank panes. The owner lives inside the mounted `LibraryPanel`
/// (task 5.11), painted through `draw_frame` exactly as `Model::run` does.
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
        output.contains("Continue"),
        "startup frame must paint the Home owner's Selector row pill bar, not \
         just legacy chrome: {output:?}"
    );
    assert!(
        output.contains("(empty)"),
        "startup Home pane must paint its empty-state placeholder, not a \
         blank pane: {output:?}"
    );
}

/// Task 5.3d + 5.11: the Selector row's hit targets are characterized from
/// the single painter — the mounted `LibraryPanel`'s own retained
/// `SkeletonHits.selector` — rather than the deleted `HomeComponent`'s
/// `pill_targets`. The assertions are preserved: one Continue-Watching pill
/// (id 0), the targets share one painted row, the selected pill is
/// highlighted, and exactly one pill bar row is painted.
#[test]
fn home_pill_row_and_targets_are_characterized_end_to_end() {
    let cw_item = emby_cw_item();
    let (model, terminal) = render_home_shell_with(home_app(), 60, 20, |m| {
        m.home_content.continue_items = vec![cw_item];
    });

    let targets = panel(&model).test_selector_hits().regions().to_vec();
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
        row_text.contains("Continue"),
        "pill row missing label: {row_text:?}"
    );
    assert_eq!(
        buffer[(first.x, first.y)].symbol(),
        "◢",
        "the selector painter owns the pill-bar start glyph"
    );
}

/// migrate-home-feeds 4.6 regression, rewritten to the panel output (task
/// 5.11): after the Wide panel skeleton paint the focused selected row keeps
/// its MainContentBox zebra stripe under the gutter accent. Unfocused rows
/// keep the unfocused stripe value.
#[test]
fn wide_home_selected_row_keeps_its_stripe_under_the_accent() {
    let bgs = |focused: bool| {
        let mut app = home_app();
        if !focused {
            app.panel_focus = PanelFocus::Queue;
        }
        let cw_item = emby_cw_item();
        let (model, terminal) = render_home_shell_with(app, 160, 40, |m| {
            m.home_content.continue_items = vec![cw_item];
        });
        let (_, selected) = panel(&model)
            .menu_geometry()
            .expect("wide Home publishes a selected-row rect");
        let selected = selected.expect("wide Home publishes a selected row");
        let buffer = terminal.backend().buffer();
        (
            buffer[(selected.x, selected.y)].style().bg,
            buffer[(selected.x, selected.y + 1)].style().bg,
        )
    };

    let (selected, _body) = bgs(true);
    // D1: the Browser-pane Wide arm stripes with the MainContentBox pair.
    assert_eq!(
        selected,
        Some(palette::surface_colors(palette::Surface::MainContentBox, true).fill),
        "the selected striped row keeps the MainContentBox fill under the gutter accent"
    );

    let (selected, _body) = bgs(false);
    assert_eq!(
        selected,
        Some(palette::surface_colors(palette::Surface::MainContentBox, false).fill),
        "the unfocused striped row keeps the MainContentBox fill"
    );
}

/// migrate-home-feeds 4.6 regression, rewritten to the panel output (task
/// 5.11): the Narrow inline hero's shell carries the focused surface. Narrow
/// Home is only reachable while its mini-view half holds focus, so there is
/// no unfocused narrow case to characterize — the panel just has to carry the
/// focused surface.
#[test]
fn narrow_home_hero_shell_carries_the_focus_surface() {
    let app = home_app();
    let (model, terminal) = render_home_shell_with(app, 60, 40, |m| {
        m.home_content.continue_items = vec![emby_cw_item()];
    });
    let hero = panel(&model)
        .test_narrow_geometry()
        .and_then(|geo| geo.inline_hero)
        .expect("narrow Home paints an inline hero");
    let expected = palette::resolve_surface_focus(true);
    let matches = (hero.left()..hero.right())
        .flat_map(|x| (hero.top()..hero.bottom()).map(move |y| (x, y)))
        .filter(|&(x, y)| terminal.backend().buffer()[(x, y)].style().bg == Some(expected))
        .count();
    assert!(matches > 0, "hero shell missing the focus surface");
}

/// migrate-home-feeds 4.6 regression, rewritten to the panel output (task
/// 5.11): a focused narrow inline hero reads as a recessed card — the
/// hero-shell background differs from the pane backdrop showing behind
/// non-selected rows. The pane is never flooded with the focus surface.
#[test]
fn narrow_home_inline_hero_contrasts_with_pane_backdrop() {
    let app = home_app();
    let (model, terminal) = render_home_shell_with(app, 60, 40, |m| {
        m.home_content.continue_items = vec![emby_cw_item()];
    });
    let geometry = panel(&model)
        .test_narrow_geometry()
        .expect("narrow Home paints the Narrow skeleton");
    let hero = geometry
        .inline_hero
        .expect("narrow Home paints an inline hero");
    let buffer = terminal.backend().buffer();

    let hero_bg = buffer[(hero.x + 1, hero.y + 1)].style().bg;
    assert_eq!(hero_bg, Some(palette::resolve_surface_focus(true)));

    // A row cell above the hero: pane backdrop from `chrome.rs`, never flooded.
    let backdrop_bg = buffer[(geometry.list_area.x, hero.y.saturating_sub(1))]
        .style()
        .bg;
    assert_eq!(backdrop_bg, Some(palette::SURFACE_BACKDROP));
    assert_ne!(hero_bg, backdrop_bg, "hero must read as a recessed card");
}
