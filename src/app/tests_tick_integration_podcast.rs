use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::library_panel::owner::LibraryContentOwner;
use crate::app::components::media_list::MediaListRow;
use crate::app::components::podcast_content::PodcastContent;
use crate::app::components::{LibraryKind, ComponentId, Msg, ShellRequest, TerminalObserverEvent};
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::tests_tick_harness::TickHarness;
use mbv_core::config::ServiceKind;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn podcast(harness: &mut TickHarness) -> &mut PodcastContent {
    podcast_for(harness, "abs-podcasts")
}

fn podcast_for<'a>(harness: &'a mut TickHarness, library_id: &str) -> &'a mut PodcastContent {
    let key = LibraryKey::Service { service: ServiceKind::Audiobookshelf, library_id: library_id.into(), kind: LibraryKind::AudiobookshelfPodcast };
    harness.model_mut().application.get_component_mut(&ComponentId::Library)
        .and_then(|c| c.as_any_mut().downcast_mut::<LibraryPanel>()).and_then(|p| p.owner_mut(&key))
        .and_then(|o| o.as_any_mut().downcast_mut::<PodcastContent>()).expect("podcast owner")
}

fn episode(library_item_id: &str, episode_id: &str) -> mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
    mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
        library_item_id: library_item_id.into(),
        episode_id: episode_id.into(),
        title: episode_id.into(),
        description: None,
        published_at: None,
        duration_seconds: None,
    }
}

fn draw(harness: &mut TickHarness, width: u16) {
    harness.model_mut().app.panel_mode = crate::app::PanelMode::LibraryOnly;
    harness.model_mut().app.terminal_width = width;
    harness.model_mut().app.terminal_height = 60;
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(width, 60)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event<crate::app::components::UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn podcast_owner_is_registered_and_starts_on_the_first_episode() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().application.mounted(&ComponentId::Library));
    assert_eq!(harness.model().application.focus(), Some(&ComponentId::Library));
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
    harness.inject(Event::Keyboard(KeyEvent { code: Key::Down, modifiers: KeyModifiers::NONE }));
    let result = harness.step();
    assert!(result.raw_messages.iter().any(|msg| matches!(msg, Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove { library_item_id: None }))));
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
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y));
    let outcome = harness.step();
    assert!(matches!(
        podcast(&mut harness).pill(),
        crate::app::state::types::audiobookshelf_browse::PillSelection::State(
            crate::app::state::types::audiobookshelf_browse::AudiobookshelfEpisodeFilter::Played
        )
    ));
    assert!(outcome.raw_messages.iter().any(|message| matches!(message, Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove { library_item_id: None }))));
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
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), point.0, point.1));
    let single = harness.step();
    // A resolved episode click is click-to-focus: the shell pulls panel
    // focus to the Library and persists the tab slot (no show selection).
    assert!(single.raw_messages.iter().any(|message| matches!(message, Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove { library_item_id: None }))));

    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), point.0, point.1));
    let double = harness.step();
    assert!(double.raw_messages.iter().any(|message| matches!(message, Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(crate::app::components::msg::PodcastEpisodeIntent::OpenOrPlay(Some(_)))))));
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
        assert!(panel.test_list_rect().is_some(), "the panel paints its list at {width}px");
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
    harness.model_mut().app.handle_lib_event(crate::app::LibEvent::AudiobookshelfDetailFetched {
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
    harness.model_mut().app.handle_lib_event(crate::app::LibEvent::AudiobookshelfDetailFetched {
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
    harness.model_mut().app.audiobookshelf_runtime.remove_setup();
    assert_ne!(
        harness.model().app.audiobookshelf_runtime.generation(),
        spawned_generation,
        "the runtime's generation advanced past the spawned request"
    );
    // The arrival carries the OLDER generation: the lib-event arm rejects
    // the payload whole — it is never cached and never lands on the mounted
    // owner's flat list through the re-projection and sync pass.
    harness.model_mut().app.handle_lib_event(crate::app::LibEvent::AudiobookshelfDetailFetched {
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

#[test]
fn launch_reanchor_restores_podcast_show_pill_before_detail_arrival() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_browse[0].detail_cache.clear();
    app.audiobookshelf_browse[0]
        .detail_loading_ids
        .insert("show-a".into(), 0);
    app.pending_launch_tab_resolved = true;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Audiobookshelf,
            library_id: "abs-podcasts".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Audiobookshelf {
            key: mbv_core::config::AudiobookshelfSelectorKey::PodcastShow("show-a".into()),
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Audiobookshelf {
            id: "show-a\0episode-restored".into(),
        }),
    });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().app.pending_launch_state.is_some());
    assert_eq!(podcast(&mut harness).pill(), &crate::app::state::types::audiobookshelf_browse::PillSelection::Show("show-a".into()));
    assert!(podcast(&mut harness).selected_episode_target().is_none());
    let selector = podcast(&mut harness).content().selector.expect("podcast selector");
    assert_eq!(selector.active, Some(4), "the saved show pill is the active painted selector");

    let generation = harness.model().app.audiobookshelf_runtime.generation();
    harness.model_mut().app.handle_lib_event(crate::app::LibEvent::AudiobookshelfDetailFetched {
        generation,
        request: 0,
        library_item_id: "show-a".into(),
        result: Ok(vec![episode("show-a", "episode-restored")]),
    });
    harness.model_mut().push_audiobookshelf_podcast_content();
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().app.pending_launch_state.is_none());
    assert_eq!(podcast(&mut harness).selected_episode_target().unwrap().episode_id(), "episode-restored");
}

#[test]
fn launch_reanchor_restores_podcast_filter_pill_on_cold_start() {
    let mut app = audiobookshelf_app();
    app.pending_launch_tab_resolved = true;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Audiobookshelf,
            library_id: "abs-podcasts".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Audiobookshelf {
            key: mbv_core::config::AudiobookshelfSelectorKey::PodcastFilter(
                mbv_core::config::AudiobookshelfPodcastFilter::Unplayed,
            ),
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Audiobookshelf {
            id: "show-a\0episode-a".into(),
        }),
    });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().app.pending_launch_state.is_none());
    assert_eq!(
        podcast(&mut harness).pill(),
        &crate::app::state::types::audiobookshelf_browse::PillSelection::State(
            crate::app::state::types::audiobookshelf_browse::AudiobookshelfEpisodeFilter::Unplayed,
        )
    );
    assert_eq!(
        podcast(&mut harness)
            .selected_episode_target()
            .expect("the saved unplayed episode is selected")
            .episode_id(),
        "episode-a"
    );
    let selector = podcast(&mut harness).content().selector.expect("podcast selector");
    assert_eq!(selector.active, Some(2), "the saved filter pill is the active selector");
}

#[test]
fn podcast_owner_survives_tab_reselection_with_the_remembered_pill() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_libraries.push(mbv_core::audiobookshelf::AudiobookshelfLibrary { id: "abs-books".into(), name: "Books".into(), media_type: "book".into() });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Keyboard(KeyEvent { code: Key::Char(']'), modifiers: KeyModifiers::NONE }));
    harness.step();
    let before = podcast(&mut harness).pill().clone();
    // Switching to another Service destination in the same column must not
    // disturb the pill either (the owner is retained while its library stays
    // in the catalog; only the list selection re-anchors on reactivation).
    // The crossing really resolves `AudiobookshelfLibrary(1)` to the Books
    // destination: the shell registers that tab's owner as part of the sync
    // pass, so the pill survives an actual destination change, not a no-op.
    let book_key = crate::app::components::LibraryKey::Service {
        service: ServiceKind::Audiobookshelf,
        library_id: "abs-books".into(),
        kind: LibraryKind::AudiobookshelfBook,
    };
    harness.model_mut().app.tab = crate::app::TabSelection::AudiobookshelfLibrary(1);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness.model().library_panel_has_owner(&book_key),
        "the crossing resolved AudiobookshelfLibrary(1) to the Books destination"
    );
    harness.model_mut().app.tab = crate::app::TabSelection::AudiobookshelfLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.tab = crate::app::TabSelection::AudiobookshelfLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(podcast(&mut harness).pill(), &before);
    let _ = TerminalObserverEvent::NoOp;
}

/// A keyboard show-pill commit scopes the shell's episode fan-out to that
/// show (reorganize-podcast-pill-navigation 3.3, design D5): the committed
/// pill's identity lands in the App's browse state through the shell sync
/// pass and its message dispatch, and wrapping back to a state pill returns
/// the scope to every listed show.
#[test]
fn keyboard_show_pill_commit_scopes_the_fan_out_through_the_shell() {
    let mut app = audiobookshelf_app();
    // A second show and no cached episodes: the pill bar has two show pills
    // and neither show's episodes are fetched.
    app.audiobookshelf_browse[0].append_page(0, 20, 2, vec![mbv_core::audiobookshelf::AudiobookshelfShow {
        library_item_id: "show-b".into(), title: "Show B".into(), author: None, description: None, cover_path: None,
    }]);
    app.audiobookshelf_browse[0].detail_cache.clear();
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let step_and_dispatch = |harness: &mut TickHarness| {
        let outcome = harness.step();
        let (mut music, mut tv) = (false, false);
        for message in outcome.messages {
            harness
                .model_mut()
                .handle_terminal_message(message, &mut music, &mut tv);
        }
    };

    // Walk to the first show pill (All -> Unplayed -> Played -> Show A).
    for _ in 0..3 {
        harness.inject(Event::Keyboard(KeyEvent { code: Key::Char(']'), modifiers: KeyModifiers::NONE }));
        step_and_dispatch(&mut harness);
    }
    assert_eq!(
        harness.model().app.audiobookshelf_browse[0].committed_show_pill.as_deref(),
        Some("show-a"),
        "the committed show pill scopes the fan-out"
    );

    // Wrapping forward past the last show pill (Show B and Latest) lands on
    // `All`: the scope follows the state pill.
    for _ in 0..3 {
        harness.inject(Event::Keyboard(KeyEvent { code: Key::Char(']'), modifiers: KeyModifiers::NONE }));
        step_and_dispatch(&mut harness);
    }
    assert_eq!(
        harness.model().app.audiobookshelf_browse[0].committed_show_pill,
        None,
        "a state pill's scope is every listed show"
    );
}

/// The shell's hero image projection (row 4.1, design D7): the podcast
/// episode hero's cover is projected keyed by the parent show's
/// `library_item_id` — the episode's own identity, not a show selection —
/// and the Ready state lands on the owner through the shell sync pass. The
/// image cache is pre-seeded under exactly that key so the proof is
/// hermetic: the projection finds the cache hit the episode's identity
/// dictates and never issues a fetch.
#[test]
fn podcast_hero_image_projection_keys_the_parent_show_cover_through_the_sync_pass() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_browse[0].shows[0].cover_path = Some("cover".into());
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(mbv_core::config::AudiobookshelfSetup::new("http://abs.test"));
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    let cache_key = crate::app::images::audiobookshelf_hero_cover_cache_key(
        "http://abs.test",
        "show-a",
        app.current_protocol_suffix(),
    );

    let mut harness = TickHarness::new(app);
    // The first draw settles the terminal size (its sync pass also clears
    // the image caches on the resize) and registers the podcast owner; the
    // seed below lands after that, so the projection that follows reads a
    // warm cache instead of issuing a fetch.
    draw(&mut harness, 80);
    harness.model_mut().app.card_image_states.insert(
        cache_key.clone(),
        crate::app::images::CachedImage {
            img: Some(image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                8,
                8,
                image::Rgba([10, 20, 30, 255]),
            ))),
            protocols: std::collections::HashMap::new(),
            cover_box: None,
            applied_logo_key: None,
        },
    );
    harness.model_mut().sync_mounted_surfaces();

    let image = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|c| c.as_any_mut().downcast_mut::<LibraryPanel>())
        .and_then(|panel| panel.active_hero_data())
        .expect("the podcast hero projects")
        .facts
        .artwork
        .image;
    assert!(
        matches!(
            &image,
            crate::app::components::library_panel::HeroImageState::Ready {
                cache_key: key,
                ..
            } if key == &cache_key
        ),
        "the hero image state is Ready under the parent show's cover key: {image:?}"
    );
    // The pre-seeded cache entry means the projection issued no fetch: the
    // once-per-key discipline holds through the sync pass.
    assert!(harness.model().app.card_image_loading.is_empty());
}

/// The playing podcast's cover must not be one cache entry serving two
/// derivations. The Wide Library hero re-encodes its entry from a cover-fit
/// crop of its artwork box (`ensure_hero_cover_protocol`), while the queue
/// card renders the same show cover through `Resize::Scale`; sharing the entry
/// made each consumer `resize_encode` (take) the other's `ThreadProtocol` every
/// frame, so the hero painted its placeholder block instead of the image — the
/// reported "wide library hero image flashes constantly" while an ABS podcast
/// plays. A non-playing show never collided: its `library_item_id` gave the
/// hero a different key.
#[test]
fn playing_show_cover_keeps_the_hero_and_queue_card_entries_apart() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_browse[0].shows[0].cover_path = Some("cover".into());
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(mbv_core::config::AudiobookshelfSetup::new("http://abs.test"));
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    // The playing episode belongs to the show whose cover the selected hero
    // draws, which is exactly the collision case.
    app.player_tab
        .queue
        .append(mbv_core::playback_queue::QueueItem::Audiobookshelf(
            mbv_core::playback_queue::AudiobookshelfQueueItem {
                library_item_id: "show-a".into(),
                episode_id: "episode-a".into(),
                title: "Episode A".into(),
                show_title: Some("Show A".into()),
                author: None,
                description: None,
                duration_ticks: None,
                position_ticks: 0,
                played: false,
                pub_date_secs: None,
                is_finished: false,
                cover_path: Some("cover".into()),
            },
        ));
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
    }
    let suffix = app.current_protocol_suffix();
    let hero_key = crate::app::images::audiobookshelf_hero_cover_cache_key(
        "http://abs.test",
        "show-a",
        suffix,
    );
    let card_key =
        crate::app::images::audiobookshelf_cover_cache_key("http://abs.test", "show-a", suffix);
    assert_ne!(
        hero_key, card_key,
        "the hero and the queue card must not share an artwork entry"
    );

    let mut harness = TickHarness::new(app);
    // Settle the terminal size first: the resize pass clears the image caches,
    // so the seeds below must land after it.
    draw(&mut harness, 160);
    for key in [&hero_key, &card_key] {
        harness.model_mut().app.card_image_states.insert(
            key.clone(),
            crate::app::images::CachedImage {
                img: Some(image::DynamicImage::ImageRgba8(
                    image::RgbaImage::from_pixel(40, 20, image::Rgba([10, 20, 30, 255])),
                )),
                protocols: std::collections::HashMap::new(),
                cover_box: None,
                applied_logo_key: None,
            },
        );
    }
    harness.model_mut().app.refresh_queue_card_image();
    draw(&mut harness, 160);

    let panel = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        .expect("library panel");
    assert!(
        panel.test_wide_geometry().is_some(),
        "the fixture must paint the Wide skeleton, where the hero's cover-fit box rebuilds the protocol"
    );
    let hero_image = panel
        .active_hero_data()
        .expect("the podcast hero projects")
        .facts
        .artwork
        .image;
    assert!(
        matches!(
            &hero_image,
            crate::app::components::library_panel::HeroImageState::Ready {
                cache_key: key,
                ..
            } if key == &hero_key
        ),
        "the hero image state is Ready under the hero-scoped cover key: {hero_image:?}"
    );

    let entries = &harness.model().app.card_image_states;
    assert!(
        entries[&hero_key].cover_box.is_some(),
        "the hero re-encodes its own entry with the cover-fit box"
    );
    assert!(
        entries[&card_key].cover_box.is_none(),
        "the queue card's entry keeps its plain encoding: the hero's crop must not reach it"
    );
    assert_eq!(
        harness
            .model()
            .app
            .queue_card_projection
            .cache_key
            .as_deref(),
        Some(card_key.as_str()),
        "the queue card paints its own key, not the hero's crop"
    );
}

/// Narrow-geometry Enter on a podcast episode plays immediately: no Library
/// Hero overlay opens (row 3.4, design D6 — podcast episodes are not
/// hero-bearing rows).
#[test]
fn narrow_enter_plays_the_selected_episode_without_an_overlay() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    draw(&mut harness, 80);
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("library panel");
    assert!(panel.test_narrow_geometry().is_some());
    assert!(!panel.test_hero_overlay_open());

    harness.inject(Event::Keyboard(KeyEvent { code: Key::Enter, modifiers: KeyModifiers::NONE }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(
            crate::app::components::msg::PodcastEpisodeIntent::OpenOrPlay(Some(_))
        ))
    )));
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("library panel");
    assert!(!panel.test_hero_overlay_open(), "no overlay opened");
}

/// Narrow-geometry double-click activates the episode directly (row 3.4):
/// the not-hero-bearing owner skips the overlay attempt and the resolved
/// row's OpenOrPlay intent crosses the shell.
#[test]
fn narrow_double_click_plays_the_selected_episode_without_an_overlay() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    draw(&mut harness, 80);
    let list = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("library panel")
        .test_list_rect()
        .expect("podcast list paints");
    let point = (list.x + 1, list.y + 1);

    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), point.0, point.1));
    harness.step();
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), point.0, point.1));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(
            crate::app::components::msg::PodcastEpisodeIntent::OpenOrPlay(Some(_))
        ))
    )));
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("library panel");
    assert!(!panel.test_hero_overlay_open(), "no overlay opened");
}

fn hero_scroll_offset(harness: &TickHarness) -> usize {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("library panel")
        .test_hero_scroll_offset()
}

/// The Wide hero's overview box is scrollable: the panel claims the wheel
/// against the box it painted and turns the owner's offset, so the box's
/// painted scrollbar follows the wheel. The hero describes one episode, so
/// selecting another starts its description at the top again.
#[test]
fn wide_hero_overview_wheel_scrolls_the_episode_description() {
    let mut app = audiobookshelf_app();
    {
        let episodes = app.audiobookshelf_browse[0]
            .detail_cache
            .get_mut("show-a")
            .expect("the fixture caches show-a's episodes");
        episodes[0].description = Some(
            "A deliberately long episode description that overflows the hero box. ".repeat(80),
        );
        episodes.push(episode("show-a", "episode-b"));
    }
    let mut harness = TickHarness::new(app);
    draw(&mut harness, 160);
    let (box_rect, max_offset) = {
        let panel = harness
            .model()
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .expect("library panel");
        let geometry = panel.test_wide_geometry().expect("wide panel");
        let box_rect = geometry
            .overview_box
            .expect("the episode hero paints an overview box");
        assert_eq!(panel.test_hero_scroll_offset(), 0);
        (
            box_rect,
            geometry.overview_content_length - geometry.overview_viewport,
        )
    };
    assert!(
        max_offset > 0,
        "the long description overflows the hero box"
    );

    harness.inject(mouse(
        MouseEventKind::ScrollDown,
        box_rect.x + 1,
        box_rect.y + 1,
    ));
    let outcome = harness.step();
    assert!(outcome
        .raw_messages
        .iter()
        .any(|message| matches!(message, Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))));
    let scrolled = hero_scroll_offset(&harness);
    assert!(
        scrolled > 0 && scrolled <= max_offset,
        "one wheel step scrolled the overview: {scrolled}"
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    assert_eq!(
        podcast(&mut harness)
            .selected_episode_target()
            .expect("an episode is selected")
            .episode_id(),
        "episode-b"
    );
    assert_eq!(
        hero_scroll_offset(&harness),
        0,
        "the newly selected episode starts at the top of its description"
    );
}

#[test]
fn podcast_latest_uses_cached_shelf_and_resolves_provider_targets_without_emby() {
    let app = audiobookshelf_app();
    let mut harness = TickHarness::new(app);
    draw(&mut harness, 80);
    assert!(harness.model().app.emby_client().is_none());
    // Latest remains selectable while its cached shelf is empty.
    harness.inject(Event::Keyboard(KeyEvent { code: Key::Char('['), modifiers: KeyModifiers::NONE }));
    harness.step();
    assert_eq!(podcast(&mut harness).pill(), &crate::app::state::types::audiobookshelf_browse::PillSelection::Latest);
    assert!(podcast(&mut harness).episode_rows().is_empty());

    // Deliver the existing shelf-fetch completion; the normal shell projection
    // updates Latest without fetching on selector entry or refreshing shows.
    let item = mbv_core::playback_queue::AudiobookshelfQueueItem {
        library_item_id: "shelf-show".into(), episode_id: "shelf-episode".into(),
        title: "Shelf episode".into(), show_title: Some("Shelf show".into()), author: None,
        description: Some("Shelf description".into()), duration_ticks: Some(90_000_000),
        position_ticks: 0, played: false, pub_date_secs: Some(1_700_000_000),
        is_finished: false, cover_path: None,
    };
    let generation = harness.model().app.audiobookshelf_runtime.generation();
    harness.model_mut().app.handle_lib_event(crate::app::LibEvent::AudiobookshelfShelfFetched {
        generation,
        library_id: "abs-podcasts".into(),
        result: Ok(vec![mbv_core::audiobookshelf::AudiobookshelfShelf {
            label: "Newest Episodes".into(),
            entries: vec![mbv_core::audiobookshelf::AudiobookshelfShelfEntry::Episode(item.clone())],
        }]),
    });
    harness.model_mut().push_audiobookshelf_podcast_content();
    draw(&mut harness, 80);
    assert_eq!(podcast(&mut harness).pill(), &crate::app::state::types::audiobookshelf_browse::PillSelection::Latest);
    assert!(!podcast(&mut harness).content().selector.unwrap().markers[0], "selection before async completion acknowledges the Latest marker");
    let target = podcast(&mut harness).selected_episode_target().expect("shelf target selected");
    assert_eq!((target.library_item_id(), target.episode_id()), ("shelf-show", "shelf-episode"));
    assert!(podcast(&mut harness).episode_rows().iter().any(|row| matches!(row,
        MediaListRow::Item { target: row_target, primary, .. } if row_target == &target && primary == "Shelf show")));
    draw(&mut harness, 160);
    assert_eq!(podcast(&mut harness).selected_episode_target().as_ref(), Some(&target));
    draw(&mut harness, 80);

    // Both activation intents carry the provider-native shelf target, and
    // the shell resolver returns the shelf's full playable snapshot.
    harness.inject(Event::Keyboard(KeyEvent { code: Key::Enter, modifiers: KeyModifiers::NONE }));
    let play = harness.step();
    assert!(play.raw_messages.iter().any(|msg| matches!(msg,
        Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(
            crate::app::components::msg::PodcastEpisodeIntent::OpenOrPlay(Some(target))
        )) if target.library_item_id() == "shelf-show" && target.episode_id() == "shelf-episode")));
    let resolved = harness.model().app.selected_audiobookshelf_queue_item_target(0, &target).expect("provider target resolves from this library's shelf");
    assert!(matches!(resolved, mbv_core::playback_queue::QueueItem::Audiobookshelf(episode) if episode.title == "Shelf episode" && episode.description.as_deref() == Some("Shelf description")));

    harness.inject(Event::Keyboard(KeyEvent { code: Key::Char('a'), modifiers: KeyModifiers::CONTROL }));
    let enqueue = harness.step();
    assert!(enqueue.raw_messages.iter().any(|msg| matches!(msg,
        Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(
            crate::app::components::msg::PodcastEpisodeIntent::Enqueue(Some(target))
        )) if target.library_item_id() == "shelf-show" && target.episode_id() == "shelf-episode")));
}

#[test]
fn podcast_latest_marker_is_visible_until_the_pill_is_selected() {
    let mut app = audiobookshelf_app();
    app.home_latest_launch_window = crate::app::home_latest::HomeLatestLaunchWindow {
        previous: Some(1_600_000_000), current: 1_800_000_000,
    };
    app.audiobookshelf_shelf_cache.insert("abs-podcasts".into(), vec![
        mbv_core::playback_queue::QueueItem::Audiobookshelf(mbv_core::playback_queue::AudiobookshelfQueueItem {
            library_item_id: "show-a".into(), episode_id: "new-episode".into(), title: "New".into(),
            show_title: Some("Show A".into()), author: None, description: None, duration_ticks: None,
            position_ticks: 0, played: false, pub_date_secs: Some(1_700_000_000), is_finished: false, cover_path: None,
        }),
    ]);
    let mut harness = TickHarness::new(app);
    harness.model_mut().app.home_latest_launch_window = crate::app::home_latest::HomeLatestLaunchWindow {
        previous: Some(1_600_000_000), current: 1_800_000_000,
    };
    harness.model_mut().sync_mounted_surfaces();
    assert!(podcast(&mut harness).content().selector.unwrap().markers[0]);
    harness.inject(Event::Keyboard(KeyEvent { code: Key::Char('['), modifiers: KeyModifiers::NONE }));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness.model_mut().handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(!podcast(&mut harness).content().selector.unwrap().markers[0]);
}

#[test]
fn podcast_latest_launch_snapshot_is_selected_tab_only() {
    let mut app = audiobookshelf_app();
    let second = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts-2".into(), name: "Second podcast library".into(), media_type: "podcast".into(),
    };
    app.audiobookshelf_browse.push(crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseState::new(second.clone()));
    app.audiobookshelf_libraries.push(second);
    app.pending_launch_tab_resolved = true;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary { kind: ServiceKind::Audiobookshelf, library_id: "abs-podcasts".into() },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Audiobookshelf { key: mbv_core::config::AudiobookshelfSelectorKey::Latest }),
        item: None,
    });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(podcast(&mut harness).pill(), &crate::app::state::types::audiobookshelf_browse::PillSelection::Latest);
    let (selector, _) = podcast(&mut harness).launch_snapshot();
    assert_eq!(selector, Some(mbv_core::config::SelectorIdentity::Audiobookshelf { key: mbv_core::config::AudiobookshelfSelectorKey::Latest }));

    harness.model_mut().app.tab = crate::app::TabSelection::AudiobookshelfLibrary(1);
    harness.model_mut().sync_mounted_surfaces();
    assert!(matches!(podcast_for(&mut harness, "abs-podcasts-2").pill(), crate::app::state::types::audiobookshelf_browse::PillSelection::State(
        crate::app::state::types::audiobookshelf_browse::AudiobookshelfEpisodeFilter::All)));
}
