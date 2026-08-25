use super::*;
// Characterization coverage stays beside the moved TV component.
use crate::app::layout::LayoutMain;
use crate::app::render::test_helpers::render_library_to_string_sized;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, SeriesDetail, TabSelection};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Terminal;
use std::collections::HashMap;

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
            cursor: 0,
            scroll: 0,
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
fn wide_tv_persists_series_workspace_and_separate_targets() {
    let mut app = tv_app();
    let mut layout = crate::app::layout::LayoutMain::default();
    let output = render_library_to_string_sized(&mut app, &mut layout, 100, 30);

    assert!(layout.is_wide_tv_active());
    assert!(!layout.tv_wide_episode_rows.is_empty());
    assert!(!layout.tv_wide_season_tabs.is_empty());
    assert_eq!(app.current_library_columns(0), 1);
    assert!(output.contains("The Series"));
    assert!(output.contains("Pilot"));
    assert!(output.contains("1h"));
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

    let output = render_library_to_string_sized(&mut app, &mut LayoutMain::default(), 100, 30);

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

    let output = render_library_to_string_sized(&mut app, &mut layout, 100, 30);

    assert!(output.contains("The Series"), "{output}");
    assert!(!output.contains("No items available"), "{output}");
    assert!(!output.contains("Empty"), "{output}");
    assert!(layout.tv_wide_season_tabs.is_empty());
    assert!(layout.tv_wide_episode_rows.is_empty());
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
    let mut app = tv_app();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut layout = crate::app::layout::LayoutMain::default();
    terminal
        .draw(|f| {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
                Rect::new(0, 0, 100, 30),
            );
            app.render_library(f, Rect::new(0, 0, 100, 30), true, &mut layout);
        })
        .unwrap();

    let pos = (
        layout.tv_wide_right_area.x + 2,
        layout.tv_wide_right_area.y + 3,
    );
    assert_eq!(
        terminal.backend().buffer()[(pos.0, pos.1)].bg,
        palette::SURFACE_FOCUSED
    );
    assert_eq!(
        terminal.backend().buffer()[(layout.tv_wide_right_area.x, pos.1)].bg,
        palette::SURFACE_FOCUSED
    );
}
