use super::*;
// Characterization coverage stays beside the moved TV component.
use crate::app::components::TvWorkspaceComponent;
use crate::app::layout::LayoutMain;
use crate::app::render::test_helpers::buffer_to_string;
use crate::app::render::HomeImagePaint;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, SeriesDetail, TabSelection};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Terminal;
use std::collections::HashMap;
use tuirealm::component::Component;

/// Paints the wide TV workspace exactly as the live shell does: draw the
/// legacy `App` base frame (which now only publishes the `tv_wide_*`
/// hand-off geometry, task 5.3d.18d) then render the mounted
/// `TvWorkspaceComponent` over the same area so it owns the picture.
/// Returns the buffer and the component so tests can read both the App
/// pre-pass layout (`AppLayout`) and the component-owned geometry
/// (`tv_wide_episode_rows`/`tv_wide_season_tabs`).
fn render_tv_workspace(app: &mut App, layout: &mut LayoutMain) -> (String, TvWorkspaceComponent) {
    let backend = TestBackend::new(100, 40);
    let mut term = Terminal::new(backend).unwrap();
    let area = Rect::new(0, 0, 100, 40);
    let mut component = TvWorkspaceComponent::new();
    component.set_content(
        app.wide_tv_render_ctx(0, true, None)
            .with_image_state(false, false),
    );
    term.draw(|f| {
        app.render_library(f, area, true, layout, None);
        component.view(f, area);
    })
    .unwrap();
    (buffer_to_string(&term), component)
}

fn tv_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    let mut library = make_item("Shows", "CollectionFolder");
    library.id = "library".into();
    library.collection_type = "tvshows".into();
    library.is_folder = true;

    let mut series = make_item("The Series", "Series");
    series.id = "series".into();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    season.index_number = 1;
    let mut episode = make_item("Pilot", "Episode");
    episode.id = "episode".into();
    episode.index_number = 1;
    episode.runtime_ticks = 3600 * mbv_core::api::TICKS_PER_SECOND;

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "library".into(),
            title: "Shows".into(),
            items: vec![series],
            total_count: 1,
            resting: crate::app::types_browse::BrowseResting::new(0, 0),
            item_types: Some("Series".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        library_total: Some(1),
        ..LibraryTab::new(library)
    });
    let mut episodes = HashMap::new();
    episodes.insert("season-1".into(), vec![episode]);
    app.series_detail_cache.insert(
        "series".into(),
        SeriesDetail {
            seasons: vec![season],
            episodes,
        },
    );
    app
}

#[test]
fn wide_tv_requests_selected_series_primary_image_with_budget_and_placeholder() {
    let app = tv_app();
    let mut component = TvWorkspaceComponent::new();
    component.set_content(app.wide_tv_render_ctx(0, true, None));
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| component.view(f, f.area())).unwrap();
    match component.take_image_paint() {
        Some(HomeImagePaint::Series {
            area,
            item,
            show_placeholder,
        }) => {
            assert_eq!(item.id, "series");
            assert_eq!((area.width, area.height), (18, 12));
            assert!(show_placeholder);
        }
        _ => panic!("expected selected Series image request"),
    }
}

#[test]
fn wide_tv_series_overview_wraps_around_portrait_in_real_painter() {
    let mut app = tv_app();
    app.libs[0].nav_stack[0].items[0].overview =
        "one two three four five six seven eight nine ten eleven twelve thirteen".into();
    let mut layout = LayoutMain::default();
    let (output, _) = render_tv_workspace(&mut app, &mut layout);
    assert!(
        output.contains("one two three"),
        "overview should be painted: {output:?}"
    );
    assert!(
        output.contains("four five six"),
        "overview should wrap beside the portrait: {output:?}"
    );
    assert!(!output.contains("one two three four five six seven eight nine"));
}

#[test]
fn wide_tv_series_placeholder_paints_the_full_portrait_budget() {
    let mut app = tv_app();
    let item = app.libs[0].nav_stack[0].items[0].clone();
    let mut terminal = Terminal::new(TestBackend::new(30, 20)).unwrap();
    terminal
        .draw(|f| {
            app.paint_home_image(
                f,
                Some(HomeImagePaint::Series {
                    area: Rect::new(2, 2, 18, 12),
                    item: Box::new(item),
                    show_placeholder: true,
                }),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    for y in 2..14 {
        for x in 2..20 {
            assert_eq!(
                buffer[(x, y)].bg,
                palette::BORDER_UNFOCUSED,
                "unpainted portrait cell at {x},{y}"
            );
        }
    }
}

#[test]
fn wide_tv_persists_series_workspace_and_separate_targets() {
    let mut app = tv_app();
    let mut layout = crate::app::layout::LayoutMain::default();
    let (output, component) = render_tv_workspace(&mut app, &mut layout);

    assert!(layout.is_wide_tv_active());
    assert!(!component.test_layout().tv_wide_episode_rows.is_empty());
    assert!(
        output.contains("Series:"),
        "season tabs are missing: {output}"
    );
    assert!(output.contains("The Series"));
    assert!(output.contains("Pilot"));
    assert!(output.contains("1h"));
}

/// `remove-migrated-surface-underpaint` 3.3 (D4): at the wide hero-on-left
/// breakpoint the mounted `TvWorkspaceComponent` owns the picture.
/// `render_library` publishes the `tv_wide_*` geometry hand-off and
/// `render_list` then returns (`src/app/render/components/list.rs:113`)
/// without painting the series hero, season tabs, or episode table.
/// Mirrors the Home precedent
/// `legacy_base_frame_does_not_paint_home_content_before_the_component`.
#[test]
fn wide_tv_legacy_base_frame_publishes_geometry_but_paints_no_workspace() {
    let mut app = tv_app();
    let mut layout = LayoutMain::default();
    let area = Rect::new(0, 0, 100, 30);
    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    term.draw(|f| {
        app.render_library(f, area, true, &mut layout, None);
    })
    .unwrap();

    assert!(layout.is_wide_tv_active());
    assert!(
        layout.tv_wide_right_area.width > 0 && layout.tv_wide_right_area.height > 0,
        "wide TV geometry hand-off must still be reserved: {:?}",
        layout.tv_wide_right_area
    );
    let output = buffer_to_string(&term);
    assert!(
        !output.contains("Pilot") && !output.contains("The Series"),
        "legacy base frame must not paint the TV workspace at the wide breakpoint: {output:?}"
    );
}

#[test]
fn wide_series_render_keeps_loading_treatment_during_season_fan_out() {
    let mut app = tv_app();
    app.series_detail_cache
        .get_mut("series")
        .unwrap()
        .episodes
        .clear();
    app.series_detail_loading.insert("series".into());
    app.series_season_loading
        .insert(("series".into(), "season-1".into()));

    let (output, _component) = render_tv_workspace(&mut app, &mut LayoutMain::default());

    assert!(output.contains("Loading"), "{output}");
}

#[test]
fn wide_series_with_no_seasons_keeps_the_child_region_blank() {
    let mut app = tv_app();
    app.series_detail_cache
        .get_mut("series")
        .unwrap()
        .seasons
        .clear();
    let mut layout = LayoutMain::default();

    let (output, component) = render_tv_workspace(&mut app, &mut layout);

    assert!(output.contains("The Series"), "{output}");
    assert!(!output.contains("No items available"), "{output}");
    assert!(!output.contains("Empty"), "{output}");
    assert!(component.test_layout().tv_wide_season_tabs.is_empty());
    assert!(component.test_layout().tv_wide_episode_rows.is_empty());
}

#[test]
fn wide_tv_selected_series_follows_inline_search_cursor() {
    let mut second = make_item("Search Series", "Series");
    second.id = "search-series".into();
    let mut component = crate::app::components::InlineSearchComponent::new();
    component.set_content(
        crate::app::components::SearchPool::Items(vec![second.clone()]),
        false,
        true,
    );
    use tuirealm::component::AppComponent;
    for key in "search".chars() {
        component.on(&tuirealm::event::Event::Keyboard(
            tuirealm::event::KeyEvent {
                code: tuirealm::event::Key::Char(key),
                modifiers: tuirealm::event::KeyModifiers::NONE,
            },
        ));
    }
    assert_eq!(component.selected_item().unwrap().id, second.id);
}

#[test]
fn wide_tv_focused_series_browser_uses_focused_surface() {
    fn render(focused: bool) -> (ratatui::buffer::Buffer, LayoutMain) {
        let mut app = tv_app();
        let area = Rect::new(0, 0, 100, 30);
        let mut layout = LayoutMain::default();
        let mut component = TvWorkspaceComponent::new();
        component.set_content(app.wide_tv_render_ctx(0, focused, None));
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal
            .draw(|f| {
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
                    area,
                );
                app.render_library(f, area, focused, &mut layout, None);
                component.view(f, area);
            })
            .unwrap();
        (terminal.backend().buffer().clone(), layout)
    }

    let (library_buffer, library_layout) = render(true);
    assert_eq!(
        library_buffer[(
            library_layout.tv_wide_list_area.x,
            library_layout.tv_wide_list_area.y + 1
        )]
            .bg,
        palette::SURFACE_BACKDROP
    );
    assert_eq!(
        library_buffer[(
            library_layout.tv_wide_list_area.x.saturating_sub(1),
            library_layout.tv_wide_list_area.y.saturating_sub(1)
        )]
            .bg,
        palette::resolve_surface_focus(false)
    );

    let (queue_buffer, queue_layout) = render(false);
    assert_ne!(
        queue_buffer[(
            queue_layout.tv_wide_list_area.x,
            queue_layout.tv_wide_list_area.y + 1
        )]
            .bg,
        palette::SURFACE_BACKDROP
    );
    assert_eq!(
        queue_buffer[(
            queue_layout.tv_wide_list_area.x.saturating_sub(1),
            queue_layout.tv_wide_list_area.y.saturating_sub(1)
        )]
            .bg,
        palette::resolve_surface_focus(true)
    );
}
