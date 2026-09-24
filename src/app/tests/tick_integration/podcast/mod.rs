use crate::app::components::library_panel::owner::LibraryContentOwner;
use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::media_list::MediaListRow;
use crate::app::components::podcast_content::PodcastContent;
use crate::app::components::{ComponentId, LibraryKind, Msg, ShellRequest, TerminalObserverEvent};
use crate::app::tests::podcast::audiobookshelf_app;
use crate::app::tests::tick_integration::harness::TickHarness;
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
    let key = LibraryKey::Service {
        service: ServiceKind::Audiobookshelf,
        library_id: library_id.into(),
        kind: LibraryKind::AudiobookshelfPodcast,
    };
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|c| c.as_any_mut().downcast_mut::<LibraryPanel>())
        .and_then(|p| p.owner_mut(&key))
        .and_then(|o| o.as_any_mut().downcast_mut::<PodcastContent>())
        .expect("podcast owner")
}

fn episode(
    library_item_id: &str,
    episode_id: &str,
) -> mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
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

mod browser;
mod hero;
mod latest;
mod navigation;
