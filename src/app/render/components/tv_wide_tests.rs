use crate::app::components::library_panel::{HeroImageState, LibraryKey, LibraryPanel};
use crate::app::components::msg::TvHit;
use crate::app::components::tv_content::TvContent;
use crate::app::components::{BrowserKey, BrowserKind};
use crate::app::render::test_helpers::buffer_to_string;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibraryTab, SeriesDetail, TabSelection};
use mbv_core::config::ServiceKind;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::collections::HashMap;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute};

fn tv_app() -> crate::app::App {
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
    let mut episode = make_item("Pilot", "Episode");
    episode.id = "episode".into();
    episode.index_number = 1;

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

/// The TV owner's `LibraryKey` under the mounted panel (task 8.4).
fn tv_key() -> LibraryKey {
    LibraryKey::Service(BrowserKey {
        service: ServiceKind::Emby,
        library_id: "library".into(),
        kind: BrowserKind::TvShows,
    })
}

/// Host one TV owner in the mounted `LibraryPanel` (task 8.4: the panel is the
/// event boundary; the owner never mounts), focused, and paint it.
fn render_tv_workspace(app: &mut crate::app::App, area: Rect) -> (String, LibraryPanel) {
    let mut owner = TvContent::new();
    owner.set_is_wide(true);
    owner.set_content(app.wide_tv_render_ctx(0, None));
    let mut panel = LibraryPanel::new();
    panel.insert_owner(tv_key(), Box::new(owner));
    panel.set_active(Some(tv_key()));
    Component::attr(&mut panel, Attribute::Focus, AttrValue::Flag(true));
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, area))
        .unwrap();
    (buffer_to_string(&terminal), panel)
}

#[test]
fn wide_tv_uses_shared_panel_skeleton_geometry_and_output() {
    let mut app = tv_app();
    let area = Rect::new(0, 0, 100, 40);
    let (output, panel) = render_tv_workspace(&mut app, area);
    let geometry = panel.test_wide_geometry().expect("Wide panel geometry");
    assert!(geometry.browser.width > 0);
    assert!(geometry.hero.width > 0);
    assert!(geometry.list_area.height > 0);
    assert!(geometry.workspace.is_some());
    assert!(output.contains("The Series"));
    assert!(output.contains("Season 1"));
    assert!(output.contains("Pilot"));
    assert!(!output.contains("Series:"));
}

#[test]
fn narrow_tv_uses_the_shared_panel_inline_skeleton() {
    let app = tv_app();
    let mut owner = TvContent::new();
    owner.set_is_wide(false);
    owner.set_content(app.wide_tv_render_ctx(0, None));
    let mut panel = LibraryPanel::new();
    panel.insert_owner(tv_key(), Box::new(owner));
    panel.set_active(Some(tv_key()));
    Component::attr(&mut panel, Attribute::Focus, AttrValue::Flag(true));
    let area = Rect::new(0, 0, 60, 30);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, area))
        .unwrap();
    let output = buffer_to_string(&terminal);
    assert!(output.contains("The Series"));
    assert!(!output.contains("Series:"));
}

#[test]
fn wide_tv_season_pills_resolve_to_typed_hits() {
    let mut app = tv_app();
    let mut second = make_item("Season 2", "Season");
    second.id = "season-2".into();
    app.series_detail_cache
        .get_mut("series")
        .unwrap()
        .seasons
        .push(second);
    let area = Rect::new(0, 0, 100, 40);
    let (_output, mut panel) = render_tv_workspace(&mut app, area);
    let (rect, _) = panel
        .test_workspace_selector_hits()
        .regions()
        .first()
        .cloned()
        .expect("season pill hit");
    let message = panel.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        message,
        Some(crate::app::components::Msg::Shell(
            crate::app::components::ShellRequest::TvHitClick {
                hit: TvHit::SeasonTab(_)
            }
        ))
    ));
}

#[test]
fn wide_tv_episode_rows_resolve_through_workspace_list() {
    let mut app = tv_app();
    let area = Rect::new(0, 0, 100, 40);
    let (_output, mut panel) = render_tv_workspace(&mut app, area);
    let workspace = panel
        .test_wide_geometry()
        .and_then(|geometry| geometry.workspace)
        .expect("Workspace geometry")
        .1;
    let message = panel.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: workspace.x,
        row: workspace.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        message,
        Some(crate::app::components::Msg::Shell(
            crate::app::components::ShellRequest::TvHitClick {
                hit: TvHit::EpisodeRow(_)
            }
        ))
    ));
}

#[test]
fn wide_tv_hero_consumes_projected_image_state_only() {
    let app = tv_app();
    let mut owner = TvContent::new();
    owner.set_is_wide(true);
    owner.set_content(
        app.wide_tv_render_ctx(0, None)
            .with_hero_image(HeroImageState::None),
    );
    let mut panel = LibraryPanel::new();
    panel.insert_owner(tv_key(), Box::new(owner));
    panel.set_active(Some(tv_key()));
    Component::attr(&mut panel, Attribute::Focus, AttrValue::Flag(true));
    let area = Rect::new(0, 0, 100, 30);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, area))
        .unwrap();
    // Painting projects no image paint: the placeholder is final, and only
    // the shell's projection (never the painter) issues a fetch.
    assert!(panel.take_image_paint().is_none());
}

#[test]
fn is_right_panel_wide_reflects_terminal_size_paint_free() {
    let mut app = make_app_stub();
    app.terminal_width = 150;
    app.terminal_height = 24;
    assert!(app.is_right_panel_wide());
    app.terminal_width = 60;
    assert!(!app.is_right_panel_wide());
}
