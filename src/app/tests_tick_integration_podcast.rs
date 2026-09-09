use ratatui::backend::TestBackend;
use ratatui::Terminal;
use crate::app::components::{BrowserKey, BrowserKind, ComponentId};
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::tests_tick_harness::TickHarness;

#[test]
fn podcast_tick_mounts_and_paints_through_shell_sync() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    let id = ComponentId::Browser(BrowserKey {
        service: mbv_core::config::ServiceKind::Audiobookshelf,
        library_id: "abs-podcasts".into(),
        kind: BrowserKind::AudiobookshelfPodcast,
    });
    assert!(harness.model().application.get_component(&id).is_some());
}
