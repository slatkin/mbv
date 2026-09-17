use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::application::PollStrategy;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::msg::TvHit;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::tv_content::TvContent;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::shell::{fold_keyboard_messages, fold_mouse_messages};
use crate::app::render::make_movie_app;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{PanelFocus, PanelMode, TabSelection};
use std::time::{Duration, Instant};

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
    harness.model_mut().sync_tv_content();
    harness.model_mut().sync_active_destination();
    harness
}

/// The one TV owner (task 8.4, design D2): registered inside the mounted
/// `LibraryPanel` under `LibraryKey::Service(TvShows)` at every breakpoint.
fn tv(harness: &TickHarness) -> &TvContent {
    harness.model().test_tv_owner()
}

/// The mounted `LibraryPanel` hosting the TV owner.
fn panel(harness: &TickHarness) -> &LibraryPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("LibraryPanel")
}

fn draw(harness: &mut TickHarness) {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    // One throwaway draw publishes `root_frame` (the shell's startup draw);
    // the sync then mounts/activates the panel, and the recorded draw paints
    // it — the steady state the deleted component's tests saw after its
    // second `view`.
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
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
    fold_keyboard_messages(folded, pre_fold_focus.as_ref(), &router)
}

/// One tick whose shell requests are handled like the run loop's.
fn step_and_drain(harness: &mut TickHarness) {
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
}

/// Opens Inline Search, types a query, and fires its debounce with a clock
/// tick past the deadline (no wall-clock waiting).
fn search_series(harness: &mut TickHarness, query: &str) {
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(harness);
    for ch in query.chars() {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Char(ch),
            modifiers: KeyModifiers::NONE,
        }));
        step_and_drain(harness);
    }
    harness
        .model_mut()
        .tick_inline_search_clock(Instant::now() + Duration::from_millis(301));
}

/// Enter on a Series search result navigates the library list to the
/// series' natural place and opens its workspace (Wide) / the Library Hero
/// overlay (Narrow) -- the ordinary browser Enter flow, launched from
/// search (inline-library-search spec, "Enter on a Series result").
#[test]
fn enter_on_a_series_search_result_navigates_and_opens_the_workspace() {
    let mut harness = tv_harness();
    search_series(&mut harness, "Second");
    assert!(harness.model().active_inline_search_is_open());

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        !harness.model().active_inline_search_is_open(),
        "activation dismisses Inline Search"
    );
    let level = harness.model().app.libs[0].nav_stack.last().unwrap();
    assert_eq!(
        level.resting().cursor(),
        1,
        "the list cursor rests on the activated series"
    );
    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("series-1".to_string()),
        "the workspace targets the activated series"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the workspace is active: episode selection holds the focus"
    );
}

#[test]
fn enter_on_a_series_search_result_narrow_opens_the_hero_overlay() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    search_series(&mut harness, "Second");

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        !harness.model().active_inline_search_is_open(),
        "activation dismisses Inline Search"
    );
    assert!(
        panel(&harness).test_hero_overlay_open(),
        "narrow activation opens the Library Hero overlay"
    );
    let level = harness.model().app.libs[0].nav_stack.last().unwrap();
    assert_eq!(level.resting().cursor(), 1);
    assert!(
        tv(&harness).episode_pane_focused(),
        "the overlay's workspace is active: episode selection holds the focus"
    );
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

    // Narrow TV is the same panel-hosted owner (task 8.4) -- no second
    // surface, and the owner stays installed at every breakpoint.
    assert!(harness
        .model()
        .library_panel_has_owner(&harness.model().test_tv_owner_key()));
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
    assert_eq!(
        narrow_anchor.selected_row_offset,
        wide_anchor.selected_row_offset,
        "wide={wide_anchor:?} narrow={narrow_anchor:?} scroll={} height={}",
        tv(&harness).scroll(),
        tv(&harness).painted_viewport_height()
    );

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

/// Task 8.4: a catalog-retained TV owner keeps its local cursor and scroll
/// while inactive. This goes through the real Application::tick path for the
/// navigation and for each tab transition's sync pass.
#[test]
fn tv_owner_retains_cursor_and_scroll_while_inactive() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().app.terminal_height = 20;
    for index in 2..20 {
        let mut item = crate::app::tests::make_item(&format!("Series {index}"), "Series");
        item.id = format!("series-{index}");
        harness
            .model_mut()
            .app
            .libs[0]
            .nav_stack[0]
            .items
            .push(item);
    }
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);

    for _ in 0..8 {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        harness.step();
        draw(&mut harness);
    }
    let before = tv(&harness)
        .viewport_anchor(tv(&harness).painted_viewport_height())
        .expect("TV owner anchor before inactive transition");
    assert_eq!(before.selected_target, "series-8");
    // The fixed-row owner may keep the selected row visible at offset zero;
    // clamping is asserted by the carrier tests for both viewport sizes.

    harness.model_mut().app.tab = TabSelection::Home;
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('x'),
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);
    let tv_key = harness.model().test_tv_owner_key_at(0);
    assert!(harness.model().library_panel_has_owner(&tv_key));

    harness.model_mut().app.tab = TabSelection::EmbyLibrary(0);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('x'),
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);
    let after = tv(&harness)
        .viewport_anchor(tv(&harness).painted_viewport_height())
        .expect("TV owner anchor after inactive transition");
    assert_eq!(after.selected_target, before.selected_target);
    assert_eq!(after.selected_row_offset, before.selected_row_offset);
}

/// Task 8.4: the season pills are resolved by the mounted panel from the
/// geometry it painted (its Workspace selector row), and the owner translates
/// the resolved slot event into the same `TvHit::SeasonTab` message the
/// deleted component emitted.
#[test]
fn tv_wide_tick_click_resolves_season_pill() {
    let mut harness = tv_harness();
    draw(&mut harness);
    // First frame geometry: the panel retained the season pills it painted.
    let (rect, _) = panel(&harness)
        .test_workspace_selector_hits()
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
    )), "tick messages: {:?}", outcome.messages);
}

/// Task 8.4: the episode rows live in the hero pane's Workspace box; the
/// panel resolves the pointer against its own painted hero pane and the owner
/// resolves the episode target through its own carrier.
#[test]
fn tv_wide_tick_click_resolves_episode_row() {
    let mut harness = tv_harness();
    draw(&mut harness);
    assert_eq!(
        tv(&harness).selected_episode_item().map(|item| item.id),
        Some("episode-1".into())
    );
    let episode_claim = panel(&harness)
        .test_wide_geometry()
        .and_then(|geometry| geometry.workspace)
        .expect("painted episode rows")
        .1;
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
