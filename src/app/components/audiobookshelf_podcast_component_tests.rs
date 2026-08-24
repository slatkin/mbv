use super::audiobookshelf_podcast::AudiobookshelfPodcastComponent;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[test]
fn abs_podcast_component_keeps_local_show_cursor_and_renders_without_app_state() {
    let app = crate::app::tests_podcast::audiobookshelf_app();
    let state = &app.audiobookshelf_browse[0];
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(state, true, false);

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(message.is_some());
    assert_eq!(component.cursor(), 0);

    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect();
    assert!(output.contains("Show A"), "output: {output:?}");
}
