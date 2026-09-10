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
use tuirealm::component::{AppComponent, Component};

fn surface_fill(surface: palette::Surface, focused: bool) -> ratatui::style::Color {
    palette::surface_colors_for_column_focus(surface, focused).fill
}

/// Paints the wide TV workspace exactly as the live shell does: draw the
/// legacy `App` base frame (which now only publishes the `tv_wide_*`
/// hand-off geometry, task 5.3d.18d) then render the mounted
/// `TvWorkspaceComponent` over the same area so it owns the picture.
/// Returns the buffer and the component so tests can read both the App
/// pre-pass layout (`AppLayout`) and the component-owned geometry
/// (`tv_wide_episode_list_area`/`tv_wide_season_tabs`).
fn render_tv_workspace(app: &mut App, layout: &mut LayoutMain) -> (String, TvWorkspaceComponent) {
    let backend = TestBackend::new(100, 40);
    let mut term = Terminal::new(backend).unwrap();
    let area = Rect::new(0, 0, 100, 40);
    let mut component = TvWorkspaceComponent::new();
    component.set_content(
        app.wide_tv_render_ctx(0, None)
            .with_image_state(false, false),
    );
    component.set_focused(true);
    term.draw(|f| {
        app.render_library(f, area, layout, None);
        component.view(f, area);
    })
    .unwrap();
    let image_paint = component.take_image_paint();
    assert!(
        image_paint.is_none(),
        "images-off TV hero must not request image painting"
    );
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
fn is_right_panel_wide_reflects_terminal_size_paint_free() {
    let mut app = make_app_stub();
    app.terminal_width = 150;
    app.terminal_height = 24;
    assert!(app.is_right_panel_wide());

    app.terminal_width = 60;
    app.terminal_height = 24;
    assert!(!app.is_right_panel_wide());
}

#[test]
fn wide_tv_images_off_collapses_artwork_and_uses_full_text_width() {
    let mut app = tv_app();
    let (output, component) = render_tv_workspace(&mut app, &mut LayoutMain::default());
    let layout = component.test_layout();
    assert!(layout.tv_wide_left_area.width > 0);
    assert!(layout.tv_wide_right_area.width > layout.tv_wide_left_area.width / 2);
    assert!(output.contains("The Series"));
    assert!(
        output.contains("Pilot"),
        "TV text must remain visible: {output}"
    );
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
                    image_types: &["Primary"],
                }),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    for y in 2..14 {
        for x in 2..20 {
            assert_eq!(
                buffer[(x, y)].bg,
                surface_fill(palette::Surface::ArtworkLoadingPlaceholder, false),
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

    assert!(layout.tv_wide_right_area.width > 0 && layout.tv_wide_right_area.height > 0);
    assert!(component.test_layout().tv_wide_episode_list_area.height > 0);
    assert!(
        output.contains("Series:"),
        "season tabs are missing: {output}"
    );
    assert!(output.contains("The Series"));
    assert!(output.contains("Pilot"));
    assert!(output.contains("1:00:00"));
}

/// TV season selection must wrap at both ends, and the rendered pill window
/// must follow the wrapped selection.
#[test]
fn wide_tv_season_pills_wrap_and_scroll_with_selection() {
    let mut app = tv_app();
    let seasons: Vec<_> = (0..8)
        .map(|index| {
            let mut season = make_item(&format!("Season {index}"), "Season");
            season.id = format!("season-{index}");
            season
        })
        .collect();
    let episodes = seasons
        .iter()
        .map(|season| {
            let mut episode = make_item("Episode", "Episode");
            episode.id = format!("episode-{}", season.id);
            (season.id.clone(), vec![episode])
        })
        .collect();
    app.series_detail_cache
        .insert("series".into(), SeriesDetail { seasons, episodes });

    let mut component = TvWorkspaceComponent::new();
    component.set_content(
        app.wide_tv_render_ctx(0, None)
            .with_image_state(false, false),
    );
    component.set_focused(true);
    component.on(&tuirealm::event::Event::Keyboard(
        tuirealm::event::KeyEvent {
            code: tuirealm::event::Key::Right,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        },
    ));
    for _ in 0..7 {
        component.on(&tuirealm::event::Event::Keyboard(
            tuirealm::event::KeyEvent {
                code: tuirealm::event::Key::Char(']'),
                modifiers: tuirealm::event::KeyModifiers::NONE,
            },
        ));
        component.set_content(
            app.wide_tv_render_ctx(0, None)
                .with_image_state(false, false),
        );
    }
    // The next right move wraps from the last season to the first.
    component.on(&tuirealm::event::Event::Keyboard(
        tuirealm::event::KeyEvent {
            code: tuirealm::event::Key::Char(']'),
            modifiers: tuirealm::event::KeyModifiers::NONE,
        },
    ));
    component.set_content(
        app.wide_tv_render_ctx(0, None)
            .with_image_state(false, false),
    );

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    assert!(terminal
        .draw(|frame| component.view(frame, frame.area()))
        .is_ok());
    let first_tabs = component.test_layout().tv_wide_season_tabs.clone();
    assert!(
        first_tabs.iter().any(|(_, id)| *id == 0),
        "wrapped first season must be visible: {first_tabs:?}"
    );
    assert!(
        first_tabs.len() < 8,
        "the season row must actually overflow"
    );

    // Moving left from the first season wraps to the last and scrolls the
    // visible window to the end.
    component.on(&tuirealm::event::Event::Keyboard(
        tuirealm::event::KeyEvent {
            code: tuirealm::event::Key::Char('['),
            modifiers: tuirealm::event::KeyModifiers::NONE,
        },
    ));
    component.set_content(
        app.wide_tv_render_ctx(0, None)
            .with_image_state(false, false),
    );
    assert!(terminal
        .draw(|frame| component.view(frame, frame.area()))
        .is_ok());
    let last_tabs = &component.test_layout().tv_wide_season_tabs;
    assert!(
        last_tabs.iter().any(|(_, id)| *id == 7),
        "wrapped last season must be visible: {last_tabs:?}"
    );
}

/// `remove-migrated-surface-underpaint` 3.3 (D4): at the wide Wide hero
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
        app.render_library(f, area, &mut layout, None);
    })
    .unwrap();

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
    assert_eq!(component.test_layout().tv_wide_episode_list_area.height, 0);
}

#[test]
fn wide_tv_episode_list_uses_shared_focus_surfaces_when_focused() {
    // A second episode (task 4.2d) so there is an unselected row. The
    // canonical episode `WideMediaList` uses the shared focused surface for
    // its selected row; the enclosing detail panel keeps the soft accent.
    let mut app = tv_app();
    let mut second_episode = make_item("Episode Two", "Episode");
    second_episode.id = "episode-2".into();
    app.series_detail_cache
        .get_mut("series")
        .unwrap()
        .episodes
        .get_mut("season-1")
        .unwrap()
        .push(second_episode);
    let mut component = TvWorkspaceComponent::new();
    component.set_content(
        app.wide_tv_render_ctx(0, None)
            .with_image_state(false, false),
    );
    component.set_focused(true);
    component.on(&tuirealm::event::Event::Keyboard(
        tuirealm::event::KeyEvent {
            code: tuirealm::event::Key::Right,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        },
    ));
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| component.view(f, f.area())).unwrap();

    let episode_list_area = component.test_layout().tv_wide_episode_list_area;
    assert_eq!(
        terminal.backend().buffer()[(episode_list_area.x, episode_list_area.y)].bg,
        surface_fill(palette::Surface::SelectedRowOnLibraryPane, true),
        "selected episode row uses the shared focused surface"
    );
    let unselected_row_y = episode_list_area.y.saturating_add(1);
    assert_eq!(
        terminal.backend().buffer()[(
            episode_list_area.x.saturating_sub(PANE_PAD_X),
            unselected_row_y
        )]
            .bg,
        surface_fill(palette::Surface::MainContentBox, true)
    );
}

/// `unify-surface-colour` 4.6: TV's overview box is the pane's soft content
/// body while the library column holds focus and keeps today's `#2d353b`
/// inset while it rests — matching the episode selection box beside it
/// instead of staying a fixed dark box that never lit up.
#[test]
fn wide_tv_overview_box_follows_the_pane_focus_like_the_episode_box() {
    let mut app = tv_app();
    // An overview gets its own inset box below the title/metadata rows.
    app.libs[0].nav_stack[0].items[0].overview =
        "A series overview long enough to reserve an inset box of its own.".into();

    let paint = |focused: bool| -> (ratatui::buffer::Buffer, Rect, Rect) {
        let mut component = TvWorkspaceComponent::new();
        component.set_content(
            app.wide_tv_render_ctx(0, None)
                .with_image_state(false, false),
        );
        component.set_focused(focused);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| component.view(f, f.area())).unwrap();
        let hero = component.test_layout().tv_wide_left_area;
        let episode_box = component.test_layout().tv_wide_episode_list_area;
        (terminal.backend().buffer().clone(), hero, episode_box)
    };

    // Focused: the overview box lights up to the episode box's own fill.
    let (focused_buffer, hero, episode_box) = paint(true);
    let episode_fill =
        focused_buffer[(episode_box.x.saturating_sub(PANE_PAD_X), episode_box.y + 1)].bg;
    let overview_focused = (hero.x..hero.right())
        .flat_map(|x| (hero.y..episode_box.y).map(move |y| (x, y)))
        .find(|&(x, y)| focused_buffer[(x, y)].bg == episode_fill)
        .expect("the focused overview box lights up with the pane");
    assert_eq!(
        focused_buffer[overview_focused].bg, episode_fill,
        "TV's overview box matches the episode selection box while the pane holds focus"
    );

    // Resting: the same box returns to today's `#2d353b` inset.
    let (resting_buffer, hero, episode_box) = paint(false);
    let overview_resting = (hero.x..hero.right())
        .flat_map(|x| (hero.y..episode_box.y).map(move |y| (x, y)))
        .find(|&(x, y)| {
            resting_buffer[(x, y)].bg == surface_fill(palette::Surface::MainContentBox, false)
        })
        .expect("the resting overview box keeps its backdrop inset");
    assert_eq!(
        resting_buffer[overview_resting].bg,
        surface_fill(palette::Surface::MainContentBox, false),
        "the resting overview box keeps today's #2d353b inset"
    );
}

/// `unify-surface-colour` 3.2: every panel fill in the wide TV workspace
/// follows the library column's focus, never the episode cursor. Moving the
/// cursor from the series rail to the episode list changes no fill, while
/// the selected-row highlight keeps following the cursor (the part that must
/// not move).
#[test]
fn wide_tv_panel_fills_follow_the_library_column_not_the_episode_cursor() {
    let mut app = tv_app();
    let mut second_episode = make_item("Episode Two", "Episode");
    second_episode.id = "episode-2".into();
    app.series_detail_cache
        .get_mut("series")
        .unwrap()
        .episodes
        .get_mut("season-1")
        .unwrap()
        .push(second_episode);
    let mut component = TvWorkspaceComponent::new();
    component.set_content(
        app.wide_tv_render_ctx(0, None)
            .with_image_state(false, false),
    );
    component.set_focused(true);

    // Cursor on the series rail.
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| component.view(f, f.area())).unwrap();
    let rail = component.test_layout().tv_wide_list_area;
    let episode_box = component.test_layout().tv_wide_episode_list_area;
    let episode_fill_x = episode_box.x.saturating_sub(PANE_PAD_X);
    let rail_body = terminal.backend().buffer()[(rail.x, rail.y + 2)].bg;
    let episode_fill = terminal.backend().buffer()[(episode_fill_x, episode_box.y + 1)].bg;
    assert_eq!(
        rail_body,
        surface_fill(palette::Surface::LibraryPanel, true),
        "rail body follows the library column's focus"
    );
    assert_eq!(
        episode_fill,
        surface_fill(palette::Surface::MainContentBox, true),
        "episode box follows the library column's focus"
    );
    // The rail holds the cursor, so its selected row is the punch-through
    // surface against the focused body.
    assert_eq!(
        terminal.backend().buffer()[(rail.x, rail.y + 1)].bg,
        surface_fill(palette::Surface::SelectedRow, false),
        "rail selected-row highlight while the rail holds the cursor"
    );

    // Move the cursor into the episode list.
    component.on(&tuirealm::event::Event::Keyboard(
        tuirealm::event::KeyEvent {
            code: tuirealm::event::Key::Right,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        },
    ));
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| component.view(f, f.area())).unwrap();
    let rail = component.test_layout().tv_wide_list_area;
    let episode_box = component.test_layout().tv_wide_episode_list_area;
    assert_eq!(
        terminal.backend().buffer()[(rail.x, rail.y + 2)].bg,
        rail_body,
        "rail body fill is unchanged when the cursor moves into the episode list"
    );
    assert_eq!(
        terminal.backend().buffer()[(episode_fill_x, episode_box.y + 1)].bg,
        episode_fill,
        "episode box fill is unchanged when the cursor moves into it"
    );
    // The highlight moved with the cursor: the episode list now marks its
    // selected row, and the rail's selected row is indistinguishable from the
    // body.
    assert_ne!(
        terminal.backend().buffer()[(episode_box.x, episode_box.y)].bg,
        episode_fill,
        "episode selected-row highlight only while the episode pane holds the cursor"
    );
    assert_eq!(
        terminal.backend().buffer()[(rail.x, rail.y + 1)].bg,
        rail_body,
        "rail selected-row highlight is gone while the episode pane holds the cursor"
    );

    // Queue column holds panel focus: both panels rest.
    component.set_focused(false);
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| component.view(f, f.area())).unwrap();
    let rail = component.test_layout().tv_wide_list_area;
    let episode_box = component.test_layout().tv_wide_episode_list_area;
    assert_eq!(
        terminal.backend().buffer()[(rail.x, rail.y + 2)].bg,
        surface_fill(palette::Surface::LibraryPanel, false),
        "rail body rests when the queue column holds focus"
    );
    assert_eq!(
        terminal.backend().buffer()[(episode_fill_x, episode_box.y + 1)].bg,
        surface_fill(palette::Surface::MainContentBox, false),
        "episode box rests when the queue column holds focus"
    );
}

/// `unify-surface-colour` 4.5: the wide-hero rail panel body has exactly one
/// filler. The six screens' duplicate `list_panel` fills were removed, so the
/// shared `wide_hero_browser_border` writes the whole panel — including the
/// flush edge outside the padded row flow — with the `LibraryPanel` surface.
/// A buffer test cannot distinguish one agreeing filler from two, so this
/// pins the surface across the full panel that the shared painter owns.
#[test]
fn wide_tv_rail_panel_body_is_filled_by_the_shared_border_painter() {
    let app = tv_app();
    let mut component = TvWorkspaceComponent::new();
    component.set_content(
        app.wide_tv_render_ctx(0, None)
            .with_image_state(false, false),
    );
    component.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| component.view(f, f.area())).unwrap();

    let padded = component.test_layout().tv_wide_list_area;
    let panel_x = padded.x.saturating_sub(PANE_PAD_X);
    let panel_width = padded.width + PANE_PAD_X * 2;
    let panel_fill =
        palette::surface_colors_for_column_focus(palette::Surface::LibraryPanel, true).fill;
    // A non-selected body row (the rail's second content row): every cell
    // across the full panel width carries the shared painter's surface.
    for x in panel_x..panel_x + panel_width {
        assert_eq!(
            terminal.backend().buffer()[(x, padded.y + 2)].bg,
            panel_fill,
            "rail panel body cell {x} is filled by the shared LibraryPanel painter"
        );
    }
}

/// migrate-home-feeds 5.1 (§5 geometry test): the shared Wide hero
/// primitive owns the one-row status-bar reserve, so wide TV's framed series
/// rail paints its `▁` bottom border two rows above `tv_wide_area`'s bottom,
/// leaving exactly one blank row before the status bar. Asserted against the
/// painted buffer so a one-row vertical shift is caught.
#[test]
fn wide_tv_series_rail_leaves_exactly_one_row_above_the_status_bar() {
    let app = tv_app();
    let mut component = TvWorkspaceComponent::new();
    component.set_content(
        app.wide_tv_render_ctx(0, None)
            .with_image_state(false, false),
    );
    component.set_focused(true);
    let area = Rect::new(0, 0, 100, 30);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|f| component.view(f, area)).unwrap();
    let right = component.test_layout().tv_wide_right_area;
    assert!(right.height > 0, "wide TV right rail must paint");
    crate::app::render::test_helpers::assert_list_pane_reserves_one_row_above_status(
        terminal.backend().buffer(),
        right,
        area.bottom(),
    );
}

/// Library wide view: the rail's panel fill follows the *column* focus, not
/// which sub-panel holds the cursor. When the episode (left) pane takes the
/// cursor the right series rail keeps the focused surface, because the
/// library column still holds panel focus (the "selection moves inside a
/// focused pane" scenario).
#[test]
fn wide_tv_rail_keeps_the_focused_surface_when_the_episode_pane_takes_the_cursor() {
    let app = tv_app();
    let mut component = TvWorkspaceComponent::new();
    component.set_content(
        app.wide_tv_render_ctx(0, None)
            .with_image_state(false, false),
    );
    component.set_focused(true);
    component.on(&tuirealm::event::Event::Keyboard(
        tuirealm::event::KeyEvent {
            code: tuirealm::event::Key::Right,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        },
    ));
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| component.view(f, f.area())).unwrap();

    let rail = component.test_layout().tv_wide_list_area;
    // A row two below the letter heading is panel body, not the selected row.
    // Before `unify-surface-colour` 3.2 this was `SURFACE_RESTING`; the
    // per-screen `episode_focused` bit no longer chooses the fill.
    assert_eq!(
        terminal.backend().buffer()[(rail.x, rail.y + 2)].bg,
        surface_fill(palette::Surface::LibraryPanel, true),
        "right rail keeps the focused surface while the library column holds focus"
    );
}

#[test]
fn wide_tv_focused_series_browser_uses_focused_surface() {
    fn render(focused: bool) -> (ratatui::buffer::Buffer, LayoutMain) {
        let mut app = tv_app();
        let area = Rect::new(0, 0, 100, 30);
        let mut layout = LayoutMain::default();
        let mut component = TvWorkspaceComponent::new();
        component.set_content(app.wide_tv_render_ctx(0, None));
        component.set_focused(focused);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal
            .draw(|f| {
                f.render_widget(
                    Block::default().style(
                        Style::default().bg(surface_fill(palette::Surface::LibraryColumn, false)),
                    ),
                    area,
                );
                app.render_library(f, area, &mut layout, None);
                component.view(f, area);
            })
            .unwrap();
        (terminal.backend().buffer().clone(), layout)
    }

    // `tv_wide_list_area.y` is the letter heading; `y + 1` is the first
    // (selected) series row, `y - 1` the panel top edge.
    let (focused_buffer, focused_layout) = render(true);
    let fla = focused_layout.tv_wide_list_area;
    // Focused rail: panel body is the focused surface, and the selected row
    // takes the library backdrop so it reads against the green panel body.
    assert_eq!(
        focused_buffer[(fla.x.saturating_sub(1), fla.y.saturating_sub(1))].bg,
        surface_fill(palette::Surface::LibraryPanel, true)
    );
    assert_eq!(
        focused_buffer[(fla.x, fla.y + 1)].bg,
        surface_fill(palette::Surface::SelectedRow, false)
    );
    assert_ne!(
        focused_buffer[(fla.x, fla.y + 1)].bg,
        focused_buffer[(fla.x, fla.y + 2)].bg,
        "selected row must be distinct from the panel body"
    );

    let (unfocused_buffer, unfocused_layout) = render(false);
    let ula = unfocused_layout.tv_wide_list_area;
    // Unfocused rail: panel drops to the resting surface and there is no
    // selection highlight — the selected row is indistinguishable from the
    // body.
    assert_eq!(
        unfocused_buffer[(ula.x.saturating_sub(1), ula.y.saturating_sub(1))].bg,
        surface_fill(palette::Surface::LibraryPanel, false)
    );
    assert_eq!(
        unfocused_buffer[(ula.x, ula.y + 1)].bg,
        unfocused_buffer[(ula.x, ula.y + 2)].bg,
        "unfocused rail shows no selection highlight"
    );
}
