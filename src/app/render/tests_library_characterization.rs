use super::test_helpers::{
    draw_mounted_frame, make_movie_app, mounted_browser_layout, mounted_browser_scroll,
    mounted_model_at, set_browser_cursor_for_test,
};
use super::*;
use crate::app::tests::make_item;
use crate::app::TabSelection;

#[test]
fn library_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    // Note: width 120 triggers wide Movies layout, which is now handled by
    // BrowserComponent (5.3d.17a). Narrow Movies is likewise painted by the
    // mounted `BrowserComponent` now (task 3.8), so route through the real
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

// Note: movies_pill_row_and_targets_are_characterized_end_to_end deleted.
// It tested the legacy wide Movies layout, which is now handled by
// BrowserComponent (5.3d.17a). Component rendering is tested separately.

/// `unify-surface-colour` section 1: the right column's gutter (the chrome
/// backdrop outside the padded library content) follows the library panel's
/// focus — `SURFACE_FOCUSED` while the library holds focus, `SURFACE_BACKDROP`
/// once the queue takes it. The left/queue arm is unaffected.
#[test]
fn wide_library_column_gutter_follows_panel_focus() {
    let cases = [
        (
            crate::app::PanelMode::LibraryOnly,
            crate::app::PanelFocus::Library,
            crate::app::palette::SURFACE_FOCUSED,
            "wide LibraryOnly focused",
        ),
        (
            crate::app::PanelMode::Both,
            crate::app::PanelFocus::Library,
            crate::app::palette::SURFACE_FOCUSED,
            "wide Both focused",
        ),
        (
            crate::app::PanelMode::Both,
            crate::app::PanelFocus::Queue,
            crate::app::palette::SURFACE_BACKDROP,
            "wide Both queue-focused",
        ),
    ];
    for (panel_mode, panel_focus, expected, label) in cases {
        let mut app = make_movie_app();
        app.panel_mode = panel_mode;
        app.panel_focus = panel_focus;
        app.terminal_width = 120;
        let chrome = app.compute_chrome_geometry(ratatui::layout::Rect::new(0, 0, 120, 30));
        let terminal = super::test_helpers::render_app_to_terminal(&mut app, 120, 30);
        let buffer = terminal.backend().buffer();
        // First content row of the right column, on the unpadded gutter column
        // the backdrop (not the library content) owns.
        let gutter = buffer[(chrome.right_area.x, chrome.right_area.y + 1)].bg;
        assert_eq!(gutter, expected, "{label}: column gutter surface");
    }
}

#[test]
fn movies_plain_replacement_characterization_covers_bottom_scroll_fallback_and_targets() {
    let mut app = make_movie_app();
    app.libs[0].nav_stack[0].items[1].overview = "The selected movie overview.".into();
    app.libs[0].nav_stack[0].set_resting_cursor(1);
    app.libs[0].nav_stack[0].set_resting_scroll(1);
    let mut model = mounted_model_at(app, 70, 30);
    set_browser_cursor_for_test(&mut model, 1);
    let output = draw_mounted_frame(&mut model, 70, 30);
    let layout = mounted_browser_layout(&model);

    assert!(
        output.contains("Second Movie"),
        "selected movie is missing:\n{output}"
    );
    assert!(
        layout.hero_area.height > 0,
        "complete selected replacement should fit: hero={:?}\n{output}",
        layout.hero_area
    );
    let selected_rect = layout
        .selected_item_rect
        .expect("selected movie keeps a parent-owned row target");
    assert_eq!(selected_rect.x, layout.hero_area.x);
    assert_eq!(selected_rect.y, layout.hero_area.y);
    assert_eq!(selected_rect.width, layout.hero_area.width);
    assert!(selected_rect.height > 0);
    let hero_lines = output
        .lines()
        .skip(layout.hero_area.y as usize)
        .take(layout.hero_area.height as usize)
        .collect::<String>();
    assert!(
        !hero_lines.contains('▎'),
        "ordinary selection marker leaked into the hero"
    );
    let control_scroll = mounted_browser_scroll(&model);
    assert!(
        control_scroll > 0,
        "mounted control must retain replacement scroll"
    );
    let _ = draw_mounted_frame(&mut model, 70, 30);
    assert_eq!(
        mounted_browser_scroll(&model),
        control_scroll,
        "mounted control scroll persists across redraws"
    );

    let mut cannot_fit = make_movie_app();
    cannot_fit.libs[0].nav_stack[0].items[1].overview = "The selected movie overview.".into();
    cannot_fit.libs[0].nav_stack[0].set_resting_cursor(1);
    let mut fallback_model = mounted_model_at(cannot_fit, 70, 12);
    set_browser_cursor_for_test(&mut fallback_model, 1);
    let fallback = draw_mounted_frame(&mut fallback_model, 70, 12);
    let fallback_layout = mounted_browser_layout(&fallback_model);
    assert!(
        fallback.contains("Second Movie"),
        "ordinary fallback loses the row:\n{fallback}"
    );
    assert_eq!(fallback_layout.hero_area.height, 0);
    assert!(
        fallback_layout.selected_item_rect.is_some(),
        "ordinary fallback retains the selected row geometry"
    );
}

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

#[test]
fn tv_letter_grouped_replacement_characterization_covers_header_fit_and_marker_suppression() {
    let mut model = mounted_model_at(tv_letter_grouped_app(12), 70, 20);
    set_browser_cursor_for_test(&mut model, 54);
    let output = draw_mounted_frame(&mut model, 70, 20);
    let layout = mounted_browser_layout(&model);

    assert!(
        output.contains("Series"),
        "selected series is missing:\n{output}"
    );
    assert!(
        layout.hero_area.height > 0,
        "grouped complete replacement should fit"
    );
    let control_scroll = mounted_browser_scroll(&model);
    assert!(
        control_scroll > 0,
        "mounted grouped control must retain scroll"
    );
    let hero_lines = output
        .lines()
        .skip(layout.hero_area.y as usize)
        .take(layout.hero_area.height as usize)
        .collect::<String>();
    assert!(
        !hero_lines.contains('▎'),
        "ordinary marker leaked into the grouped hero"
    );
    let _ = draw_mounted_frame(&mut model, 70, 20);
    assert_eq!(
        mounted_browser_scroll(&model),
        control_scroll,
        "mounted grouped control scroll persists across redraws"
    );

    let mut boundary_model = mounted_model_at(tv_letter_grouped_app(1), 70, 14);
    set_browser_cursor_for_test(&mut boundary_model, 54);
    let boundary_output = draw_mounted_frame(&mut boundary_model, 70, 14);
    let boundary_layout = mounted_browser_layout(&boundary_model);
    assert!(
        boundary_output.contains("Series"),
        "header fit boundary hides selected row: hero={:?}\n{boundary_output}",
        boundary_layout.hero_area
    );
    assert_eq!(
        boundary_layout.hero_area.height, 0,
        "cannot-fit grouped detail restores ordinary rows"
    );
    assert!(boundary_layout.selected_item_rect.is_some());
}

/// migrate-home-feeds 4.6 regression: after the full wide-Movies arrangement
/// paint, the focused selected row's background is the surface *containing*
/// the list panel (`SURFACE_BACKDROP`), and the rail-framing helper — which
/// now runs before the row flow — must not overpaint that bar. Unfocused,
/// the row must match the panel body (no bar).
#[test]
fn wide_movies_selected_row_punches_through_to_the_library_backdrop() {
    use crate::app::components::browser::{BrowserComponent, BrowserContent};
    use crate::app::components::component_id::BrowserKind;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::component::Component;

    fn selected_and_body_bg(focused: bool) -> (ratatui::style::Color, ratatui::style::Color) {
        let items = (0..10)
            .map(|i| {
                let mut item = make_item(&format!("Movie {i:02}"), "Movie");
                item.id = format!("movie-{i}");
                item
            })
            .collect();
        let mut browser = BrowserComponent::new_for_kind(BrowserKind::Movies);
        browser.set_content(BrowserContent::from_items(items));
        browser.set_focused(focused);
        browser.apply_position(0, 40);
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| browser.view(frame, frame.area()))
            .unwrap();
        let layout = browser.test_layout();
        let buffer = terminal.backend().buffer();
        let row_for = |target: usize| layout.left_area.y + target as u16;
        (
            buffer[(layout.left_area.x, row_for(0))].bg,
            buffer[(layout.left_area.x, row_for(1))].bg,
        )
    }

    let (selected, body) = selected_and_body_bg(true);
    assert_eq!(selected, crate::app::palette::SURFACE_BACKDROP);
    assert_eq!(body, crate::app::palette::resolve_surface_focus(true));
    assert_ne!(selected, body);

    let (selected, body) = selected_and_body_bg(false);
    assert_eq!(selected, body, "unfocused rail shows no selection bar");
}

/// migrate-home-feeds 5.1 (§5 geometry test): the shared Wide hero
/// primitive owns the one-row status-bar reserve, so wide Movies' framed list
/// panel paints its `▁` bottom border two rows above the destination area's
/// bottom, leaving exactly one blank row before the status bar. Asserted
/// against the painted buffer so a one-row vertical shift is caught.
#[test]
fn wide_movies_list_panel_leaves_exactly_one_row_above_the_status_bar() {
    use crate::app::components::browser::{BrowserComponent, BrowserContent};
    use crate::app::components::component_id::BrowserKind;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::component::Component;

    let items = (0..40)
        .map(|i| {
            let mut item = make_item(&format!("Movie {i:02}"), "Movie");
            item.id = format!("movie-{i}");
            item
        })
        .collect();
    let mut browser = BrowserComponent::new_for_kind(BrowserKind::Movies);
    browser.set_content(BrowserContent::from_items(items));
    browser.set_focused(true);
    browser.apply_position(0, 40);

    let area = ratatui::layout::Rect::new(0, 0, 120, 40);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|frame| browser.view(frame, area)).unwrap();
    let right = browser.test_layout().movies_wide_right_area;
    assert!(right.height > 0, "wide movies right pane must paint");
    super::test_helpers::assert_list_pane_reserves_one_row_above_status(
        terminal.backend().buffer(),
        right,
        area.bottom(),
    );
}
