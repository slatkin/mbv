use super::audiobookshelf_podcast::AudiobookshelfPodcastComponent;
use super::msg::{
    Msg, PodcastEpisodeIntent, PodcastEpisodeTransition, PodcastShowMove, ShellRequest,
};
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
    let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(movement))) = message else {
        panic!("show movement should be a typed show-move request");
    };
    assert_eq!(movement, PodcastShowMove::NextRow);
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

#[test]
fn abs_podcast_component_emits_typed_episode_transitions_in_episode_mode() {
    let mut app = crate::app::tests_podcast::audiobookshelf_app();
    app.audiobookshelf_browse[0].episode_selection = Some(0);
    let state = &app.audiobookshelf_browse[0];
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(state, true, false);

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeTransition(transition))) =
        message
    else {
        panic!("episode movement should be a typed episode-transition request, got {message:?}");
    };
    assert_eq!(transition, PodcastEpisodeTransition::NextEpisode);

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeTransition(transition))) =
        message
    else {
        panic!("filter cycling should be a typed episode-transition request, got {message:?}");
    };
    assert_eq!(transition, PodcastEpisodeTransition::NextFilter);

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Esc,
        modifiers: KeyModifiers::NONE,
    }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeTransition(transition))) =
        message
    else {
        panic!("episode exit should be a typed episode-transition request, got {message:?}");
    };
    assert_eq!(transition, PodcastEpisodeTransition::Exit);
}

#[test]
fn abs_podcast_component_emits_typed_action_intents_without_raw_key_replay() {
    let state = &crate::app::tests_podcast::audiobookshelf_app().audiobookshelf_browse[0];
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(state, true, false);

    // One representative action key per intent: the component reports only the
    // matched intent (task 5.3d.7); the shell resolves conditions at the Model
    // boundary.
    let space = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char(' '),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        space,
        Some(Msg::Shell(
            ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::FocusOrPlay)
        ))
    ));

    let enter = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        enter,
        Some(Msg::Shell(
            ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay)
        ))
    ));

    let ctrl_a = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('a'),
        modifiers: KeyModifiers::CONTROL,
    }));
    assert!(matches!(
        ctrl_a,
        Some(Msg::Shell(
            ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::Enqueue)
        ))
    ));

    // An unrelated key no longer produces a raw request: unmatched keyboard
    // keys return no message.
    let unrelated = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('z'),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(
        unrelated.is_none(),
        "unmatched key must produce no message, got {unrelated:?}"
    );
}
