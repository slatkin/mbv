use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::application::PollStrategy;
use tuirealm::component::Component;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::msg::TvHit;
use crate::app::components::{Msg, ShellRequest, TvWorkspaceComponent};
use crate::app::shell::{apply_router_outcome, fold_mouse_messages};
use crate::app::render::make_movie_app;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{PanelFocus, PanelMode, TabSelection};

fn tv_harness() -> TickHarness {
    let mut app = make_movie_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::Both;
    app.terminal_width = 160;
    app.terminal_height = 50;
    app.libs[0].library.collection_type = "tvshows".into();
    for (index, item) in app.libs[0].nav_stack[0].items.iter_mut().enumerate() {
        item.item_type = "Series".into();
        item.id = format!("series-{index}");
        item.overview.clear();
    }
    let mut season = crate::app::tests::make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = crate::app::tests::make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    app.series_detail_cache.insert(
        "series-0".into(),
        crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: [("season-1".into(), vec![episode])].into_iter().collect(),
        },
    );
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_tv_workspace();
    harness.model_mut().sync_active_destination();
    harness
}

/// The one merged TV owner (task 8.1, design D12): mounted at every
/// breakpoint under `ComponentId::TvWorkspace`.
fn tv(harness: &TickHarness) -> &TvWorkspaceComponent {
    harness
        .model()
        .application
        .get_component(harness.model().tv_workspace_id.as_ref().expect("TV workspace id"))
        .expect("TV workspace mounted")
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .expect("TV workspace component")
}

fn draw(harness: &mut TickHarness) {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
}

fn step_without_sync(harness: &mut TickHarness) -> Vec<Msg> {
    let pre_fold_focus = harness.model().application.focus().cloned();
    let raw_messages = harness
        .model_mut()
        .application
        .tick(PollStrategy::Once(std::time::Duration::from_millis(500)))
        .expect("tick injected event");
    let folded = fold_mouse_messages(raw_messages);
    let router = harness.model_mut().router_outcome(&folded);
    apply_router_outcome(folded, pre_fold_focus.as_ref(), &router)
}

fn draw_tv_workspace(harness: &mut TickHarness) {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    let area = harness
        .model()
        .app
        .wide_tv_library_area(0)
        .expect("TV Wide area");
    let id = harness
        .model()
        .tv_workspace_id
        .clone()
        .expect("TV workspace id");
    let context = harness.model().app.wide_tv_render_ctx(0, None);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            let component = harness
                .model_mut()
                .application
                .get_component_mut(&id)
                .expect("TV workspace component")
                .as_any_mut()
                .downcast_mut::<TvWorkspaceComponent>()
                .expect("TV workspace component type");
            component.set_is_wide(true);
            component.set_content(context);
            component.view(frame, area);
        })
        .unwrap();
}

#[test]
fn tv_narrow_tick_navigation_updates_the_painted_control() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    let before = tv(&harness)
        .viewport_anchor(tv(&harness).painted_viewport_height())
        .expect("narrow TV selection");

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);

    let after = tv(&harness)
        .viewport_anchor(tv(&harness).painted_viewport_height())
        .expect("narrow TV selection after navigation");
    assert_ne!(after.selected_target, before.selected_target);

    // Narrow TV is the same merged owner (task 8.1) -- no second component
    // id, and the pointer never clears at any breakpoint.
    assert!(harness.model().tv_workspace_id.is_some());
}

/// unify-screens-under-panel-components task 8.1 (design D12, stable-target
/// re-anchor): the merged owner keeps its selected target across a
/// Wide->Narrow->Wide breakpoint round trip driven through real ticks.
#[test]
fn tv_wide_narrow_wide_tick_navigation_keeps_the_selected_target() {
    let mut harness = tv_harness();
    draw(&mut harness);
    assert_eq!(tv(&harness).selected_item_id(), Some("series-0".into()));

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);
    assert_eq!(tv(&harness).selected_item_id(), Some("series-1".into()));
    let wide_anchor = tv(&harness)
        .viewport_anchor(tv(&harness).painted_viewport_height())
        .expect("wide TV viewport anchor");

    // Narrow: the same owner keeps the same selected target and viewport
    // offset across the shared Wide/Inline presentation transition.
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    assert_eq!(tv(&harness).selected_item_id(), Some("series-1".into()));
    let narrow_anchor = tv(&harness)
        .viewport_anchor(tv(&harness).painted_viewport_height())
        .expect("narrow TV viewport anchor");
    assert_eq!(narrow_anchor.selected_target, wide_anchor.selected_target);
    assert_eq!(narrow_anchor.selected_row_offset, wide_anchor.selected_row_offset);

    // Wide again: still the same target and viewport offset.
    harness.model_mut().app.terminal_width = 160;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    assert_eq!(tv(&harness).selected_item_id(), Some("series-1".into()));
    let final_anchor = tv(&harness)
        .viewport_anchor(tv(&harness).painted_viewport_height())
        .expect("final Wide TV viewport anchor");
    assert_eq!(final_anchor.selected_target, narrow_anchor.selected_target);
    assert_eq!(final_anchor.selected_row_offset, narrow_anchor.selected_row_offset);
}

#[test]
fn tv_wide_tick_navigation_updates_the_painted_control() {
    let mut harness = tv_harness();
    draw(&mut harness);
    assert_eq!(tv(&harness).selected_item_id(), Some("series-0".into()));

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);

    assert_eq!(tv(&harness).selected_item_id(), Some("series-1".into()));
}

#[test]
fn tv_wide_tick_click_resolves_season_pill() {
    let mut harness = tv_harness();
    draw(&mut harness);
    draw_tv_workspace(&mut harness);
    let (rect, _) = tv(&harness)
        .test_season_hits()
        .regions()
        .first()
        .cloned()
        .expect("painted season pill");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvHitClick {
            hit: TvHit::SeasonTab(0)
        })
    )));
}

#[test]
fn tv_wide_tick_click_resolves_episode_row() {
    let mut harness = tv_harness();
    draw(&mut harness);
    draw_tv_workspace(&mut harness);
    assert_eq!(tv(&harness).selected_episode_item().map(|item| item.id), Some("episode-1".into()));
    let episode_claim = tv(&harness)
        .test_episode_claim_rect()
        .expect("painted episode rows");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: episode_claim.x,
        row: episode_claim.y,
        modifiers: KeyModifiers::NONE,
    }));
    let messages = step_without_sync(&mut harness);
    assert!(messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::TvHitClick {
            hit: TvHit::EpisodeRow(target)
        }) if target == "episode-1"
    )), "tick messages: {:?}", messages);
}
