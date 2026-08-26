use super::audiobookshelf_podcast::AudiobookshelfPodcastComponent;
use super::msg::{
    LegacyTerminalEvent, Msg, PodcastEpisodeIntent, PodcastEpisodeTransition, PodcastShowMove,
    ShellRequest,
};
use crate::app::images::audiobookshelf_cover_cache_key;
use crate::app::shell::Model;
use crate::app::tests_podcast::audiobookshelf_app;
use mbv_core::config::{AudiobookshelfSetup, ServiceKind};
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

    // An unrelated key forwards as a raw terminal event through the shared
    // framework bridge: TuiRealm only delivers to the focused component and
    // does not fall through on None, so global App shortcuts depend on this.
    let unrelated = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('z'),
        modifiers: KeyModifiers::NONE,
    }));
    let Some(Msg::Legacy(LegacyTerminalEvent::Key(forwarded))) = unrelated else {
        panic!("unmatched key must forward a raw terminal event, got {unrelated:?}");
    };
    assert_eq!(
        forwarded,
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('z'),
            crossterm::event::KeyModifiers::NONE,
        )
    );
}

#[test]
fn abs_podcast_cover_fetch_bridged_to_content_push_and_gated_by_images() {
    // Image-disabled: a content push must not schedule any cover fetch.
    let mut model = Model::new(audiobookshelf_app());
    model.sync_audiobookshelf_podcast();
    model.push_audiobookshelf_podcast_content();
    assert!(
        model.app.card_image_loading.is_empty(),
        "image-disabled content push must not schedule a cover fetch"
    );

    // Image-enabled with a configured server and secret: the selected show's
    // cover is scheduled through the bridge on the content push.
    model.app.image_protocol_enabled = true;
    model.app.config.lock().unwrap().audiobookshelf_setup =
        Some(AudiobookshelfSetup::new("https://abs.example"));
    mbv_core::config::save_service_secret(ServiceKind::Audiobookshelf, "test-secret").unwrap();
    model.push_audiobookshelf_podcast_content();

    let server = model
        .app
        .config
        .lock()
        .unwrap()
        .audiobookshelf_setup
        .as_ref()
        .unwrap()
        .server_url
        .clone();
    let expected_key =
        audiobookshelf_cover_cache_key(&server, "show-a", model.app.current_protocol_suffix());
    assert!(
        model.app.card_image_loading.contains(&expected_key),
        "image-enabled content push should schedule the selected show's cover fetch"
    );
}
