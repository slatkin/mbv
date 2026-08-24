use super::test_helpers::*;
use super::*;
use crate::app::components::{InlineSearchComponent, SearchPool};
use ratatui::{backend::TestBackend, Terminal};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[test]
fn searched_album_listing_does_not_duplicate_artist_row_in_plain_framing() {
    let app = make_music_group_app();
    let items = app.libs[0].nav_stack.last().unwrap().items.clone();
    let mut component = InlineSearchComponent::new();
    component.set_content(
        SearchPool::Items(items),
        false,
        true,
        ratatui::layout::Rect::new(0, 0, 60, 20),
    );
    for key in "First Album".chars() {
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Char(key),
            modifiers: KeyModifiers::NONE,
        }));
    }
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let out = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert_eq!(
        out.lines().filter(|line| line.trim() == "Alpha").count(),
        0,
        "plain search framing emits no artist header row:\n{out}"
    );
}
