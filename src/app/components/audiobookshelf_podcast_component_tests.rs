use super::audiobookshelf_podcast::AudiobookshelfPodcastComponent;
use super::msg::{
    LegacyTerminalEvent, Msg, PodcastEpisodeIntent, PodcastEpisodeTransition, PodcastShowMove,
    ShellRequest,
};
use crate::app::images::audiobookshelf_cover_cache_key;
use crate::app::shell::Model;
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;
use mbv_core::audiobookshelf::{AudiobookshelfLibrary, AudiobookshelfShow};
use mbv_core::config::{AudiobookshelfSetup, ServiceKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
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

/// Task 5.3d.10c: the component owns its painted geometry (list/right/hero/
/// inline-hero/selected-item rects), so the shell can read it after render
/// ownership moved off `App`. The same mounted component is rendered wide then
/// narrow; the wide right panel must be coherent, and a narrow re-render must
/// not leak the wide `right_area`. A no-show narrow render resets every hero
/// field.
#[test]
fn abs_podcast_component_geometry_is_wide_coherent_and_narrow_resets_wide() {
    let mut state = AudiobookshelfBrowseState::new(AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    });
    state.append_page(
        0,
        10,
        10,
        vec![AudiobookshelfShow {
            library_item_id: "show-a".into(),
            title: "Show A".into(),
            author: Some("Author".into()),
            description: Some("An audacious podcast about everything worth hearing.".into()),
            cover_path: None,
        }],
    );
    state.select(0);

    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, true, false);

    let wide = Rect::new(0, 0, 100, 40);
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal.draw(|frame| component.view(frame, wide)).unwrap();
    let geometry = component.geometry();
    assert!(
        geometry.hero_area.width > 0 && geometry.hero_area.height > 0,
        "wide hero must be painted"
    );
    assert!(
        geometry.right_area.width > 0 && geometry.right_area.height > 0,
        "wide right panel must be painted"
    );
    assert_eq!(
        geometry.list_area, geometry.right_area,
        "wide list == right panel"
    );
    assert_eq!(geometry.hero_area.x, wide.x, "wide hero is the left pane");
    assert!(
        geometry.hero_area.right() <= geometry.right_area.x,
        "wide hero sits left of the right panel"
    );
    assert!(geometry.hero_area.bottom() <= wide.bottom());
    assert!(geometry.right_area.right() <= wide.right());
    assert_eq!(
        geometry.inline_hero_area,
        Rect::default(),
        "wide layout has no inline hero"
    );
    assert!(
        geometry.selected_item_rect.is_none(),
        "wide layout has no selected-item shell"
    );

    // Re-render the same mounted component narrow: the wide `right_area` must
    // not survive, and the admitted inline hero must agree across fields.
    let narrow = Rect::new(0, 0, 60, 40);
    terminal
        .draw(|frame| component.view(frame, narrow))
        .unwrap();
    let geometry = component.geometry();
    assert_eq!(
        geometry.right_area,
        Rect::default(),
        "narrow render must reset the wide right_area"
    );
    assert!(
        geometry.list_area.width > 0 && geometry.list_area.height > 0,
        "narrow list area must be nonzero"
    );
    assert!(
        geometry.list_area.y >= narrow.y && geometry.list_area.bottom() <= narrow.bottom(),
        "narrow list sits within the area"
    );
    assert!(
        geometry.hero_area.width > 0 && geometry.hero_area.height > 0,
        "narrow inline hero must be admitted for a short selected show"
    );
    assert_eq!(
        geometry.inline_hero_area, geometry.hero_area,
        "narrow inline hero must equal the painted hero"
    );
    assert_eq!(
        geometry.selected_item_rect,
        Some(geometry.hero_area),
        "narrow selected-item rect must equal the painted hero"
    );
    assert!(geometry.hero_area.right() <= narrow.right());
    assert!(geometry.hero_area.bottom() <= narrow.bottom());

    // No-show narrow render: every hero/right/selected field resets.
    let empty = AudiobookshelfBrowseState::new(AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    });
    let mut empty_component = AudiobookshelfPodcastComponent::new();
    empty_component.set_content(&empty, true, false);
    let mut empty_terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();

    empty_terminal
        .draw(|frame| empty_component.view(frame, wide))
        .unwrap();
    let empty_wide_geometry = empty_component.geometry();
    assert!(
        empty_wide_geometry.right_area.width > 0,
        "no-show wide layout still paints its right placeholder panel"
    );
    assert_eq!(
        empty_wide_geometry.list_area,
        empty_wide_geometry.right_area
    );
    assert_eq!(
        empty_wide_geometry.hero_area,
        Rect::default(),
        "no-show wide layout must not report an unpainted hero"
    );
    assert!(empty_wide_geometry.selected_item_rect.is_none());

    empty_terminal
        .draw(|frame| empty_component.view(frame, narrow))
        .unwrap();
    let empty_narrow_geometry = empty_component.geometry();
    assert_eq!(
        empty_narrow_geometry.list_area, narrow,
        "no-show narrow list_area is the whole area"
    );
    assert_eq!(empty_narrow_geometry.right_area, Rect::default());
    assert_eq!(empty_narrow_geometry.hero_area, Rect::default());
    assert_eq!(empty_narrow_geometry.inline_hero_area, Rect::default());
    assert!(empty_narrow_geometry.selected_item_rect.is_none());
}
