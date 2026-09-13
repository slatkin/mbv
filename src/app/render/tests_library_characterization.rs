use super::test_helpers::{
    draw_mounted_frame, make_movie_app, mounted_model_at, mounted_tv_layout, mounted_tv_scroll,
    set_tv_cursor_for_test,
};
use super::*;
use crate::app::components::browser_content::BrowserContent as BrowserOwner;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::ComponentId;
use crate::app::tests::make_item;
use crate::app::TabSelection;
use tuirealm::component::Component;

/// The mounted `LibraryPanel` (the migrated `BrowserContent` owner's host
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

/// Seed the migrated Movies/HomeVideos/Generic owner's authoritative
/// selection directly (mirrors `set_browser_cursor_for_test`'s old
/// `BrowserComponent` contract, now against the embedded owner).
fn set_movie_cursor_for_test(model: &mut crate::app::shell::Model, cursor: usize) {
    model.sync_mounted_surfaces();
    let (_, key, _) = model
        .active_migrated_browser_owner()
        .expect("a migrated browser owner is active");
    model
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        .and_then(|panel| panel.owner_mut(&key))
        .and_then(|owner| owner.as_any_mut().downcast_mut::<BrowserOwner>())
        .expect("browser owner installed")
        .set_cursor_for_test(cursor);
}

fn movie_owner_scroll(model: &crate::app::shell::Model) -> usize {
    let (_, key, _) = model
        .active_migrated_browser_owner()
        .expect("a migrated browser owner is active");
    panel(model)
        .owner(&key)
        .and_then(|owner| owner.as_any().downcast_ref::<BrowserOwner>())
        .expect("browser owner installed")
        .scroll()
}

#[test]
fn library_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    // Note: width 120 triggers the Wide skeleton, and narrow width the
    // Narrow skeleton, both painted by the mounted `LibraryPanel`'s embedded
    // `BrowserContent` owner (task 6.1). Route through the real
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
// embedded `BrowserContent` owner (task 6.1). Component rendering is tested
// separately.

#[test]
fn movies_plain_replacement_characterization_covers_bottom_scroll_fallback_and_targets() {
    let mut app = make_movie_app();
    app.libs[0].nav_stack[0].items[1].overview = "The selected movie overview.".into();
    app.libs[0].nav_stack[0].set_resting_cursor(1);
    app.libs[0].nav_stack[0].set_resting_scroll(1);
    let mut model = mounted_model_at(app, 70, 30);
    set_movie_cursor_for_test(&mut model, 1);
    let output = draw_mounted_frame(&mut model, 70, 30);
    let layout = panel(&model)
        .test_narrow_geometry()
        .expect("the panel painted a Narrow skeleton");

    assert!(
        output.contains("Second Movie"),
        "selected movie is missing:\n{output}"
    );
    let hero_area = layout.inline_hero.unwrap_or_default();
    assert!(
        hero_area.height > 0,
        "complete selected replacement should fit: hero={hero_area:?}\n{output}"
    );
    let selected_rect = layout
        .selected
        .expect("selected movie keeps a parent-owned row target");
    assert_eq!(selected_rect.x, hero_area.x);
    assert_eq!(selected_rect.y, hero_area.y);
    assert_eq!(selected_rect.width, hero_area.width);
    assert!(selected_rect.height > 0);
    let hero_lines = output
        .lines()
        .skip(hero_area.y as usize)
        .take(hero_area.height as usize)
        .collect::<String>();
    assert!(
        !hero_lines.contains('▎'),
        "ordinary selection marker leaked into the hero"
    );
    let control_scroll = movie_owner_scroll(&model);
    assert!(
        control_scroll > 0,
        "mounted owner must retain replacement scroll"
    );
    let _ = draw_mounted_frame(&mut model, 70, 30);
    assert_eq!(
        movie_owner_scroll(&model),
        control_scroll,
        "mounted owner scroll persists across redraws"
    );

    let mut cannot_fit = make_movie_app();
    cannot_fit.libs[0].nav_stack[0].items[1].overview = "The selected movie overview.".into();
    cannot_fit.libs[0].nav_stack[0].set_resting_cursor(1);
    let mut fallback_model = mounted_model_at(cannot_fit, 70, 12);
    set_movie_cursor_for_test(&mut fallback_model, 1);
    let fallback = draw_mounted_frame(&mut fallback_model, 70, 12);
    let fallback_layout = panel(&fallback_model)
        .test_narrow_geometry()
        .expect("the panel painted a Narrow skeleton");
    assert!(
        fallback.contains("Second Movie"),
        "ordinary fallback loses the row:\n{fallback}"
    );
    assert!(fallback_layout.inline_hero.is_none());
    assert!(
        fallback_layout.selected.is_some(),
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

    let mut boundary_model = mounted_model_at(tv_letter_grouped_app(1), 70, 24);
    set_tv_cursor_for_test(&mut boundary_model, 54);
    let boundary_output = draw_mounted_frame(&mut boundary_model, 70, 24);
    let boundary_layout = mounted_tv_layout(&boundary_model);
    assert!(
        boundary_output.contains("Series"),
        "header fit boundary hides selected row: hero={:?}\n{boundary_output}",
        boundary_layout.inline_hero_area
    );
    assert_eq!(
        boundary_layout.inline_hero_area.height, 0,
        "cannot-fit grouped detail restores ordinary rows"
    );
    assert!(
        boundary_layout.selected_item_rect.is_some(),
        "cannot-fit grouped detail retains the selected row target"
    );
}

#[test]
fn wide_movies_list_panel_leaves_exactly_one_row_above_the_status_bar() {
    use crate::app::components::browser_content::BrowserOwnerPush;
    use crate::app::components::component_id::BrowserKind;
    use crate::app::components::library_panel::owner::LibraryKey;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let items: Vec<_> = (0..40)
        .map(|i| {
            let mut item = make_item(&format!("Movie {i:02}"), "Movie");
            item.id = format!("movie-{i}");
            item
        })
        .collect();
    let total_count = items.len();
    let mut owner = BrowserOwner::new(BrowserKind::Movies);
    owner.set_content(BrowserOwnerPush {
        items,
        total_count,
        library_total: None,
        letter_filter: None,
        loading: false,
        group_pills: false,
        home_video: false,
        show_letter_pills: false,
        feed_groups: Vec::new(),
        feed_group_cursor: 0,
    });
    owner.apply_position(0, 40);
    let mut panel = LibraryPanel::new();
    panel.insert_owner(LibraryKey::Home, Box::new(owner));
    panel.set_active(Some(LibraryKey::Home));
    tuirealm::component::Component::attr(
        &mut panel,
        tuirealm::props::Attribute::Focus,
        tuirealm::props::AttrValue::Flag(true),
    );

    let area = ratatui::layout::Rect::new(0, 0, 120, 40);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, area))
        .unwrap();
    let geometry = panel
        .test_wide_geometry()
        .expect("the panel painted a Wide skeleton");
    assert!(
        geometry.list_panel.height > 0,
        "wide movies rail must paint"
    );
    super::test_helpers::assert_list_pane_reserves_one_row_above_status(
        terminal.backend().buffer(),
        geometry.list_panel,
        area.bottom(),
    );
}
