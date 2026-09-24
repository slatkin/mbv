use super::*;

#[test]
fn podcast_owner_is_registered_and_starts_on_the_first_episode() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().application.mounted(&ComponentId::Library));
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Library)
    );
    assert!(harness
        .model()
        .mouse_eligible_ids()
        .contains(&ComponentId::Library));
    // The remembered pill starts at `All`; the flat browser starts on its
    // first episode row.
    assert!(matches!(
        podcast(&mut harness).pill(),
        crate::app::state::types::audiobookshelf_browse::PillSelection::State(
            crate::app::state::types::audiobookshelf_browse::AudiobookshelfEpisodeFilter::All
        )
    ));
    let target = podcast(&mut harness)
        .selected_episode_target()
        .expect("the first episode is selected");
    assert_eq!(
        target.episode_id(),
        "episode-a",
        "the flat browser starts on the remembered pill's first episode"
    );
    // Keyboard list movement resolves like a row click: the landed cursor
    // persists and re-projects through the same ShowMove request.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let result = harness.step();
    assert!(result.raw_messages.iter().any(|msg| matches!(
        msg,
        Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: None
        })
    )));
}

#[test]
fn podcast_panel_mouse_pill_click_commits_the_state_selection() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    draw(&mut harness, 160);
    let (rect, _) = {
        let panel = harness
            .model()
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .expect("library panel");
        let region = panel
            .test_selector_hits()
            .regions()
            .iter()
            .find(|(_, index)| *index == 3)
            .map(|(rect, _)| *rect)
            .expect("the Played state pill paints in the Selector row");
        (region, panel.test_wide_geometry().expect("wide panel"))
    };
    harness.inject(mouse(
        MouseEventKind::Down(MouseButton::Left),
        rect.x,
        rect.y,
    ));
    let outcome = harness.step();
    assert!(matches!(
        podcast(&mut harness).pill(),
        crate::app::state::types::audiobookshelf_browse::PillSelection::State(
            crate::app::state::types::audiobookshelf_browse::AudiobookshelfEpisodeFilter::Played
        )
    ));
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: None
        })
    )));
}

#[test]
fn podcast_panel_mouse_episode_clicks_claim_and_open_or_play() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    draw(&mut harness, 160);
    let list = {
        let panel = harness
            .model()
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .expect("library panel");
        panel.test_list_rect().expect("podcast list paints")
    };
    // The fixture's episode is undated, so the grouped flow's first painted
    // row is its `Unknown date` heading and the episode row is next.
    let point = (list.x + 1, list.y + 1);
    harness.inject(mouse(
        MouseEventKind::Down(MouseButton::Left),
        point.0,
        point.1,
    ));
    let single = harness.step();
    // A resolved episode click is click-to-focus: the shell pulls panel
    // focus to the Library and persists the tab slot (no show selection).
    assert!(single.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: None
        })
    )));

    harness.inject(mouse(
        MouseEventKind::Down(MouseButton::Left),
        point.0,
        point.1,
    ));
    let double = harness.step();
    assert!(double.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(
            crate::app::components::msg::PodcastEpisodeIntent::OpenOrPlay(Some(_))
        ))
    )));
}

#[test]
fn podcast_panel_owns_one_surface_at_wide_and_normal_breakpoints() {
    for width in [160, 80] {
        let mut harness = TickHarness::new(audiobookshelf_app());
        draw(&mut harness, width);
        let panel = harness
            .model()
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .expect("library panel");
        assert_eq!(panel.test_wide_geometry().is_some(), width == 160);
        assert_eq!(panel.test_narrow_geometry().is_some(), width == 80);
        assert!(
            panel.test_list_rect().is_some(),
            "the panel paints its list at {width}px"
        );
    }
}

/// The episode browser declares the shared reveal-on-selection title policy:
/// the destination opts in once at construction and the shared row painter
/// applies it. The rows themselves keep both title parts, so the reveal is a
/// paint-time policy rather than a content change.
#[test]
fn podcast_episode_list_declares_the_reveal_on_selection_title_policy() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    draw(&mut harness, 160);

    let owner = podcast(&mut harness);
    assert_eq!(
        owner.episode_title_reveal(),
        crate::app::components::media_list::MediaListTitleReveal::OnSelection,
        "the podcast episode list reveals its rows' titles on selection"
    );
    assert!(
        owner.episode_rows().iter().any(|row| matches!(
            row,
            MediaListRow::Item { secondary: Some(title), .. } if !title.is_empty()
        )),
        "the rows still carry the episode title: the reveal is a paint-time policy"
    );
}

#[test]
fn podcast_flat_browser_updates_in_place_when_episodes_arrive() {
    let mut app = audiobookshelf_app();
    let browse = &mut app.audiobookshelf_browse[0];
    // Under the per-show cache the fixture's pre-cached episodes would
    // already be the view; a fetch in flight means the show is not cached
    // yet, so drop the fixture's entry for this scenario.
    browse.detail_cache.clear();
    browse.detail_loading_ids.insert("show-a".into(), 0);
    let mut harness = TickHarness::new(app);
    draw(&mut harness, 160);
    assert!(podcast(&mut harness).episode_rows().is_empty());
    assert!(harness.model().app.audiobookshelf_browse[0]
        .detail_loading_ids
        .contains_key("show-a"));

    // Provider completion is dispatched as the shell's `LibEvent::
    // AudiobookshelfDetailFetched` arrival (the lib-event arm accepts the
    // generation, retires the in-flight mark, and caches the batch); the
    // drain's trailing podcast re-projection lands it on the mounted owner
    // through the sync pass, without a sleep.
    let generation = harness.model().app.audiobookshelf_runtime.generation();
    harness
        .model_mut()
        .app
        .handle_lib_event(crate::app::LibEvent::AudiobookshelfDetailFetched {
            generation,
            request: 0,
            library_item_id: "show-a".into(),
            result: Ok(vec![episode("show-a", "episode-ready")]),
        });
    harness.model_mut().push_audiobookshelf_podcast_content();
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness, 160);
    assert!(!harness.model().app.audiobookshelf_browse[0]
        .detail_loading_ids
        .contains_key("show-a"));
    // The arrival joined the grouped flow as a selectable episode row and
    // the selection landed on it.
    let rows = podcast(&mut harness).episode_rows().to_vec();
    let selected = podcast(&mut harness).selected_episode_target();
    assert!(rows.iter().any(|row| matches!(
        row,
        MediaListRow::Item { target, .. } if Some(target) == selected.as_ref()
    )));

    // An empty provider completion overwrites the previously cached rows —
    // the view must not keep a stale copy of the previous episodes — driven
    // through the same lib-event arrival, with the show's earlier batch
    // still in the cache when the empty result lands.
    let browse = &mut harness.model_mut().app.audiobookshelf_browse[0];
    browse.detail_loading_ids.insert("show-a".into(), 1);
    let generation = harness.model().app.audiobookshelf_runtime.generation();
    harness
        .model_mut()
        .app
        .handle_lib_event(crate::app::LibEvent::AudiobookshelfDetailFetched {
            generation,
            request: 1,
            library_item_id: "show-a".into(),
            result: Ok(Vec::new()),
        });
    harness.model_mut().push_audiobookshelf_podcast_content();
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness, 160);
    assert!(!harness.model().app.audiobookshelf_browse[0]
        .detail_loading_ids
        .contains_key("show-a"));
    // The empty result wins: the cached batch for the show is now empty and
    // the owner's list re-projected to no rows. Without dispatching the
    // empty arrival, the earlier batch would still be cached and painted.
    let browse = &harness.model().app.audiobookshelf_browse[0];
    assert!(
        browse
            .detail_cache
            .get("show-a")
            .is_some_and(|episodes| episodes.is_empty()),
        "the empty arrival overwrites the previously cached batch"
    );
    assert!(podcast(&mut harness).episode_rows().is_empty());
}

/// A superseded-generation arrival is rejected by the shell's lib-event
/// arm (`audiobookshelf_runtime.accepts`): the Service setup was replaced
/// after the request spawned (production does this via the runtime's
/// `remove_setup`, which advances the generation), so the payload arrives
/// carrying the OLDER generation it spawned with. It is never cached and
/// the mounted owner's flat list is unchanged through the sync pass (row
/// 4.1). The in-flight mark still retires on the rejected path so the
/// bounded fan-out never stalls behind it.
#[test]
fn stale_generation_arrival_never_reaches_the_owner_through_the_sync_pass() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_browse[0].detail_cache.clear();
    app.audiobookshelf_browse[0]
        .detail_loading_ids
        .insert("show-a".into(), 0);
    let mut harness = TickHarness::new(app);
    draw(&mut harness, 160);
    assert!(podcast(&mut harness).episode_rows().is_empty());

    // The request spawns under the runtime's current generation, then the
    // Service setup is replaced the way production does, advancing the
    // runtime's generation past the one the arrival will carry.
    let spawned_generation = harness.model().app.audiobookshelf_runtime.generation();
    harness
        .model_mut()
        .app
        .audiobookshelf_runtime
        .remove_setup();
    assert_ne!(
        harness.model().app.audiobookshelf_runtime.generation(),
        spawned_generation,
        "the runtime's generation advanced past the spawned request"
    );
    // The arrival carries the OLDER generation: the lib-event arm rejects
    // the payload whole — it is never cached and never lands on the mounted
    // owner's flat list through the re-projection and sync pass.
    harness
        .model_mut()
        .app
        .handle_lib_event(crate::app::LibEvent::AudiobookshelfDetailFetched {
            generation: spawned_generation,
            request: 0,
            library_item_id: "show-a".into(),
            result: Ok(vec![episode("show-a", "episode-stale")]),
        });
    harness.model_mut().push_audiobookshelf_podcast_content();
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness, 160);

    let state = &harness.model().app.audiobookshelf_browse[0];
    assert!(
        !state.detail_cache.contains_key("show-a"),
        "a rejected payload is never cached"
    );
    assert!(
        state.detail_loading_ids.is_empty(),
        "the in-flight mark still retires so the fan-out never stalls"
    );
    assert!(
        podcast(&mut harness).episode_rows().is_empty(),
        "the rejected arrival never joined the owner's flat list"
    );
}
