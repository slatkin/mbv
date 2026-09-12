use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::podcast_content::PodcastContent;
use crate::app::components::{BrowserKey, BrowserKind, ComponentId, Msg, ShellRequest, TerminalObserverEvent};
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::tests_tick_harness::TickHarness;
use mbv_core::config::ServiceKind;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

fn podcast(harness: &mut TickHarness) -> &mut PodcastContent {
    let key = LibraryKey::Service(BrowserKey { service: ServiceKind::Audiobookshelf, library_id: "abs-podcasts".into(), kind: BrowserKind::AudiobookshelfPodcast });
    harness.model_mut().application.get_component_mut(&ComponentId::Library)
        .and_then(|c| c.as_any_mut().downcast_mut::<LibraryPanel>()).and_then(|p| p.owner_mut(&key))
        .and_then(|o| o.as_any_mut().downcast_mut::<PodcastContent>()).expect("podcast owner")
}

#[test]
fn podcast_owner_is_registered_and_focuses_library_panel() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().application.mounted(&ComponentId::Library));
    assert!(podcast(&mut harness).selected_id().is_some());
    harness.inject(Event::Keyboard(KeyEvent { code: Key::Down, modifiers: KeyModifiers::NONE }));
    let result = harness.step();
    assert!(result.raw_messages.iter().any(|msg| matches!(msg, Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove { .. }))));
}

#[test]
fn podcast_owner_survives_tab_reselection() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_libraries.push(mbv_core::audiobookshelf::AudiobookshelfLibrary { id: "abs-books".into(), name: "Books".into(), media_type: "book".into() });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let before = podcast(&mut harness).selected_id();
    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.tab = crate::app::TabSelection::AudiobookshelfLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(podcast(&mut harness).selected_id(), before);
    assert_eq!(harness.model().application.mounted(&ComponentId::Browser(BrowserKey { service: ServiceKind::Audiobookshelf, library_id: "abs-podcasts".into(), kind: BrowserKind::AudiobookshelfPodcast })), false);
    let _ = TerminalObserverEvent::NoOp;
}
