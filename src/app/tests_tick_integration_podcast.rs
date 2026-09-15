use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::podcast_content::PodcastContent;
use crate::app::components::{LibraryKind, ComponentId, Msg, ShellRequest, TerminalObserverEvent};
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::tests_tick_harness::TickHarness;
use mbv_core::config::ServiceKind;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

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
fn podcast_owner_is_registered_and_focuses_library_panel() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().application.mounted(&ComponentId::Library));
    assert_eq!(harness.model().application.focus(), Some(&ComponentId::Library));
    assert!(harness
        .model()
        .mouse_eligible_ids()
        .contains(&ComponentId::Library));
    assert!(podcast(&mut harness).selected_id().is_some());
    harness.inject(Event::Keyboard(KeyEvent { code: Key::Down, modifiers: KeyModifiers::NONE }));
    let result = harness.step();
    assert!(result.raw_messages.iter().any(|msg| matches!(msg, Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove { .. }))));
}

#[test]
fn podcast_panel_mouse_filter_click_updates_owner_and_claims_event() {
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
            .test_workspace_selector_hits()
            .regions()
            .get(1)
            .map(|(rect, _)| *rect)
            .expect("Played filter pill paints in the Workspace");
        (region, panel.test_wide_geometry().expect("wide panel"))
    };
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y));
    let outcome = harness.step();
    assert_eq!(podcast(&mut harness).episode_filter(), crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::Played);
    assert!(outcome.raw_messages.iter().any(|message| matches!(message, Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))));
}

#[test]
fn podcast_panel_mouse_episode_clicks_claim_and_open_or_play() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    draw(&mut harness, 160);
    let content = {
        let panel = harness
            .model()
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .expect("library panel");
        panel
            .test_wide_geometry()
            .and_then(|geometry| geometry.workspace.map(|(_, content)| content))
            .expect("podcast Workspace paints")
    };
    let point = (content.x + 1, content.y);
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), point.0, point.1));
    let single = harness.step();
    assert!(single.raw_messages.iter().any(|message| matches!(message, Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))));

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
fn podcast_owner_survives_tab_reselection() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_libraries.push(mbv_core::audiobookshelf::AudiobookshelfLibrary { id: "abs-books".into(), name: "Books".into(), media_type: "book".into() });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let before = podcast(&mut harness).selected_id();
    podcast(&mut harness).set_episode_filter(
        crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::Played,
    );
    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.tab = crate::app::TabSelection::AudiobookshelfLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(podcast(&mut harness).selected_id(), before);
    assert_eq!(
        podcast(&mut harness).episode_filter(),
        crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter::Played
    );
    let _ = TerminalObserverEvent::NoOp;
}
