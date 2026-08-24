use super::feeds::FeedsComponent;
use super::msg::{LegacyTerminalEvent, Msg};
use super::user_event::UserEvent;
use crate::app::types_feed_tab::WatchedFilter;
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::FeedEntry;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

fn entry(title: &str, played: bool) -> FeedEntry {
    FeedEntry {
        guid: title.into(),
        title: title.into(),
        enclosure_url: Some(format!("https://example.test/{title}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played,
    }
}

fn component() -> FeedsComponent {
    let subscriptions = [FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![entry("First", false), entry("Second", true)];
    let mut component = FeedsComponent::new();
    component.set_content(
        &subscriptions,
        &[entries.clone()],
        &entries,
        WatchedFilter::All,
        0,
        0,
        0,
        false,
        true,
    );
    component
}

#[test]
fn down_moves_the_component_cursor_without_app_state() {
    let mut component = component();
    let msg = component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));

    assert_eq!(component.cursor(), 1);
    assert_eq!(msg, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
}

#[test]
fn watched_filter_rebuilds_the_component_visible_list() {
    let mut component = component();
    component.on(&Event::<UserEvent>::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));

    assert_eq!(component.watched_filter(), WatchedFilter::Watched);
    assert_eq!(component.visible_titles(), ["Second"]);
}

#[test]
fn feeds_render_without_app_state() {
    let mut component = component();
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 20)))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let output: String = (0..buffer.area().height)
        .flat_map(|y| (0..buffer.area().width).map(move |x| buffer[(x, y)].symbol().to_owned()))
        .collect();
    assert!(output.contains("Test Feed"));
    assert!(output.contains("First"));
}
