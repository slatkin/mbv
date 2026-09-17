use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
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
    let key = LibraryKey::Service { service: ServiceKind::Audiobookshelf, library_id: "abs-podcasts".into(), kind: LibraryKind::AudiobookshelfPodcast };
    harness.model_mut().application.get_component_mut(&ComponentId::Library)
        .and_then(|c| c.as_any_mut().downcast_mut::<LibraryPanel>()).and_then(|p| p.owner_mut(&key))
        .and_then(|o| o.as_any_mut().downcast_mut::<PodcastContent>()).expect("podcast owner")
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
        crate::app::types_audiobookshelf_browse::PillSelection::State(
            crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::All
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
            .find(|(_, index)| *index == 2)
            .map(|(rect, _)| *rect)
            .expect("the Played state pill paints in the Selector row");
        (region, panel.test_wide_geometry().expect("wide panel"))
    };
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y));
    let outcome = harness.step();
    assert!(matches!(
        podcast(&mut harness).pill(),
        crate::app::types_audiobookshelf_browse::PillSelection::State(
            crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::Played
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

#[test]
fn podcast_flat_browser_updates_in_place_when_episodes_arrive() {
    let mut app = audiobookshelf_app();
    let browse = &mut app.audiobookshelf_browse[0];
    // Under the per-show cache the fixture's pre-cached episodes would
    // already be the view; a fetch in flight means the show is not cached
    // yet, so drop the fixture's entry for this scenario.
    browse.detail_cache.clear();
    browse.detail_loading_ids.insert("show-a".into());
    let mut harness = TickHarness::new(app);
    draw(&mut harness, 160);
    assert!(podcast(&mut harness).episode_rows().is_empty());
    assert!(harness.model().app.audiobookshelf_browse[0]
        .detail_loading_ids
        .contains("show-a"));

    // Provider completion is injected at the state boundary; the same owner
    // remains mounted and updates its rows in place without a sleep.
    let episode = mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
        library_item_id: "show-a".into(),
        episode_id: "episode-ready".into(),
        title: "Ready Episode".into(),
        description: None,
        published_at: None,
        duration_seconds: None,
    };
    let browse = &mut harness.model_mut().app.audiobookshelf_browse[0];
    browse.detail_loading_ids.remove("show-a");
    browse.cache_detail("show-a".into(), vec![episode]);
    harness.model_mut().push_audiobookshelf_podcast_content();
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness, 160);
    assert!(!harness.model().app.audiobookshelf_browse[0]
        .detail_loading_ids
        .contains("show-a"));
    // The arrival joined the grouped flow as a selectable episode row and
    // the selection landed on it.
    let rows = podcast(&mut harness).episode_rows().to_vec();
    let selected = podcast(&mut harness).selected_episode_target();
    assert!(rows.iter().any(|row| matches!(
        row,
        MediaListRow::Item { target, .. } if Some(target) == selected.as_ref()
    )));

    // An empty provider completion remains an empty view, not a stale copy
    // of the previous rows.
    let browse = &mut harness.model_mut().app.audiobookshelf_browse[0];
    browse.detail_cache.remove("show-a");
    browse.cache_detail("show-a".into(), Vec::new());
    harness.model_mut().push_audiobookshelf_podcast_content();
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness, 160);
    assert!(!harness.model().app.audiobookshelf_browse[0]
        .detail_loading_ids
        .contains("show-a"));
    assert!(podcast(&mut harness).episode_rows().is_empty());
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

    // Wrapping forward past the last show pill (Show B) lands on `All`: the
    // scope follows the state pill.
    for _ in 0..2 {
        harness.inject(Event::Keyboard(KeyEvent { code: Key::Char(']'), modifiers: KeyModifiers::NONE }));
        step_and_dispatch(&mut harness);
    }
    assert_eq!(
        harness.model().app.audiobookshelf_browse[0].committed_show_pill,
        None,
        "a state pill's scope is every listed show"
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
