use super::*;

fn navigated_series(id: &str, name: &str) -> Box<mbv_core::api::EmbyItem> {
    let mut item = crate::app::tests::make_item(name, "Series");
    item.id = id.into();
    Box::new(item)
}

/// Task 3.1: a landing on a Series runs the same detail hand-off Inline
/// Search runs -- the retained TV owner re-anchors, the Wide workspace opens
/// with episode selection focused -- driven through the shell drain and the
/// `Application::tick()` sync pass.
#[test]
fn navigated_series_opens_the_wide_workspace() {
    let mut harness = tv_harness();
    // The retained owner starts on series-0; the navigation targets series-1.
    harness
        .model_mut()
        .handle_inline_search_lib_event(LibEvent::NavigateTo {
            lib_idx: 0,
            landing: NavigateLanding::Series {
                reveal: navigated_series("series-1", "Second"),
                episode_id: None,
            },
            switch_tab: true,
        });
    harness.step();

    assert_eq!(
        harness.model().app.tab,
        TabSelection::EmbyLibrary(0),
        "the landing switches to the target library"
    );
    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("series-1".to_string()),
        "the retained TV owner re-anchors onto the navigated series"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the Wide workspace is open with episode selection focused"
    );
}

/// Task 3.1: the Narrow landing opens the Library Hero overlay for the
/// navigated show, through the same hand-off. Starts on the Home tab so the
/// hand-off must run AFTER the sync pass retargets the panel's active owner
/// to the landed library; opening the overlay during the event drain would
/// target the pre-navigation owner instead.
#[test]
fn navigated_series_opens_the_hero_overlay_narrow() {
    let mut harness = tv_harness();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().app.tab = TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);

    harness
        .model_mut()
        .handle_inline_search_lib_event(LibEvent::NavigateTo {
            lib_idx: 0,
            landing: NavigateLanding::Series {
                reveal: navigated_series("series-1", "Second"),
                episode_id: None,
            },
            switch_tab: true,
        });
    harness.step();
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);

    assert_eq!(harness.model().app.tab, TabSelection::EmbyLibrary(0));
    assert!(harness.model().app.wide_tv_library_area(0).is_none());
    assert!(
        panel(&harness).test_hero_overlay_open(),
        "narrow navigation opens the Library Hero overlay for the show"
    );
    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("series-1".to_string()),
        "the retained TV owner re-anchors onto the navigated series"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the overlay belongs to the TV owner, not the pre-navigation one"
    );
}

/// Task 3.1 design watch: when the target library's corpus cannot satisfy the
/// landing yet, the hand-off must fire on the pending-landing retry drain --
/// not at the original `NavigateTo` -- and still open the Wide workspace.
#[test]
fn deferred_series_landing_runs_the_handoff_on_its_retry_drain() {
    let mut harness = tv_harness();
    harness.model_mut().app.tab = TabSelection::Home;
    {
        // A paginated root: the whole-library corpus (`all_items`) is absent,
        // so the show can still be satisfied by the prefetch drain.
        let level = harness.model_mut().app.libs[0]
            .nav_stack
            .last_mut()
            .expect("root level");
        level.total_count = 5;
        level.all_items = None;
    }
    harness.model_mut().sync_mounted_surfaces();

    harness
        .model_mut()
        .handle_inline_search_lib_event(LibEvent::NavigateTo {
            lib_idx: 0,
            landing: NavigateLanding::Series {
                reveal: navigated_series("series-9", "Ninth"),
                episode_id: None,
            },
            switch_tab: true,
        });
    assert!(
        harness.model().app.pending_series_landing.is_some(),
        "an unsaturated corpus arms the pending landing"
    );
    assert!(
        harness.model().app.pending_series_handoff.is_none(),
        "no hand-off before the landing actually completes"
    );
    assert_eq!(
        harness.model().app.tab,
        TabSelection::Home,
        "the tab switch is deferred with the landing"
    );

    harness
        .model_mut()
        .handle_inline_search_lib_event(LibEvent::AllItemsPrefetched {
            lib_idx: 0,
            parent_id: "lib-movies".into(),
            items: vec![
                *navigated_series("series-0", "First"),
                *navigated_series("series-9", "Ninth"),
            ],
        });
    assert!(
        harness.model().app.pending_series_landing.is_none(),
        "the retry landed"
    );
    assert_eq!(harness.model().app.tab, TabSelection::EmbyLibrary(0));
    harness.step();

    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("series-9".to_string()),
        "the deferred landing's hand-off re-anchors the owner"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the deferred landing opens the Wide workspace"
    );
}

/// A flat `Upcoming` level holding three Emby `/Shows/Upcoming` placeholders:
/// `Type: Episode` with no `Id` and a `series_id` set (the real-server shape
/// for unaired/not-downloaded rows).
fn upcoming_placeholder_harness() -> TickHarness {
    let mut harness = tv_harness();
    let episodes = (0..3)
        .map(|index| {
            let mut episode =
                crate::app::tests::make_item(&format!("Upcoming Episode {index}"), "Episode");
            episode.id.clear();
            episode.series_id = format!("series-{index}");
            episode.series_name = format!("The Show {index}");
            episode
        })
        .collect();
    let level = &mut harness.model_mut().app.libs[0].nav_stack[0];
    level.items = episodes;
    level.item_types = Some("Episode".into());
    level.tv_content_mode = Some(mbv_core::config::TvContentMode::Upcoming);
    level.loading = false;
    harness.model_mut().app.libs[0].library_total = Some(301);
    harness.model_mut().app.libs[0].tv_content_mode =
        Some(mbv_core::config::TvContentMode::Upcoming);
    harness.model_mut().sync_mounted_surfaces();
    harness
}

/// Assert that activation leaves flat Upcoming mode and lands the selected
/// placeholder's series Workspace through the shared ensure-then-land path.
fn assert_placeholder_workspace(harness: &mut TickHarness, index: usize) {
    let expected_id = format!("series-{index}");
    assert_eq!(harness.model().app.player_tab.total_queue_len(), 0);
    let pending = harness
        .model()
        .app
        .pending_series_landing
        .as_ref()
        .expect("the series landing is armed");
    assert_eq!(pending.reveal.id, expected_id);
    assert_eq!(pending.reveal.item_type, "Series");

    // The whole-library corpus drain lands the reveal and opens the Workspace
    // (the same path a Series search/navigation result uses).
    let mut series = crate::app::tests::make_item(&format!("The Show {index}"), "Series");
    series.id = expected_id.clone();
    harness
        .model_mut()
        .handle_inline_search_lib_event(LibEvent::AllItemsPrefetched {
            lib_idx: 0,
            parent_id: "lib-movies".into(),
            items: vec![series],
        });
    harness.step();
    assert!(harness.model().app.pending_series_landing.is_none());
    assert_eq!(
        tv(harness).selected_item().map(|item| item.id),
        Some(expected_id)
    );
    assert!(tv(harness).episode_pane_focused());
}

#[rstest]
#[case::first_row(0)]
#[case::middle_row(1)]
#[case::last_row(2)]
fn keyboard_activation_of_each_upcoming_placeholder_opens_its_series_workspace(
    #[case] index: usize,
) {
    let mut harness = upcoming_placeholder_harness();
    for _ in 0..index {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        step_and_drain(&mut harness);
    }
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    step_and_drain(&mut harness);
    assert_placeholder_workspace(&mut harness, index);
}

#[rstest]
#[case::first_row(0)]
#[case::middle_row(1)]
#[case::last_row(2)]
fn mouse_activation_of_each_upcoming_placeholder_opens_the_clicked_series_workspace(
    #[case] index: usize,
) {
    let mut harness = upcoming_placeholder_harness();
    draw(&mut harness);
    let row_area = panel(&harness)
        .test_wide_geometry()
        .expect("painted browser")
        .list_area;
    let point = (row_area.x + 1, row_area.y + index as u16);
    let click = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: point.0,
        row: point.1,
        modifiers: KeyModifiers::NONE,
    });
    harness.inject(click.clone());
    step_and_drain(&mut harness);
    harness.inject(click);
    step_and_drain(&mut harness);
    assert_placeholder_workspace(&mut harness, index);
}

/// Mouse and keyboard activation of a playable flat episode both play it
/// directly instead of navigating to a series Workspace.
#[rstest]
#[case::latest(mbv_core::config::TvContentMode::Latest)]
#[case::upcoming(mbv_core::config::TvContentMode::Upcoming)]
fn mouse_double_click_on_a_playable_episode_still_plays_through_tick(
    #[case] mode: mbv_core::config::TvContentMode,
) {
    let mut harness = flat_episode_harness(mode);
    draw(&mut harness);
    let row_area = panel(&harness)
        .test_wide_geometry()
        .expect("painted browser")
        .list_area;
    let click = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: row_area.x + 1,
        row: row_area.y,
        modifiers: KeyModifiers::NONE,
    });
    harness.inject(click.clone());
    step_and_drain(&mut harness);
    harness.inject(click);
    step_and_drain(&mut harness);

    assert_eq!(
        harness.model().app.playback_queue().emby_items()[0].id,
        "latest-episode"
    );
    assert!(harness.model().app.pending_series_landing.is_none());
}
