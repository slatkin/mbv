use super::audiobookshelf_podcast::AudiobookshelfPodcastComponent;
use super::audiobookshelf_podcast_test_support::{narrow_grid_component_state, view_narrow};
use super::msg::{
    Msg, PodcastEpisodeIntent, PodcastEpisodeTarget, PodcastEpisodeTransition, ShellRequest,
};
use crate::app::images::audiobookshelf_cover_cache_key;
use crate::app::shell::Model;
use crate::app::tests_podcast::audiobookshelf_app;
use crate::app::types_audiobookshelf_browse::{
    AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
};
use mbv_core::audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfProgress,
    AudiobookshelfShow,
};
use mbv_core::config::{AudiobookshelfSetup, ServiceKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

/// split-audiobookshelf-cursor-ownership D4 / task 1.3 → 5.1: when a content
/// push drops the show the component had selected, the component resets its
/// own `episode_focused` / `episode_filter` / `scroll` to their defaults —
/// it never adopts the values carried in the shell's snapshot for those
/// fields.
#[test]
fn abs_podcast_component_drops_stale_episode_state_when_selection_vanishes() {
    let library = AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let show = |id: &str, title: &str| AudiobookshelfShow {
        library_item_id: id.into(),
        title: title.into(),
        author: None,
        description: None,
        cover_path: None,
    };

    let mut first = AudiobookshelfBrowseState::new(library.clone());
    first.append_page(
        0,
        20,
        2,
        vec![show("show-a", "Show A"), show("show-b", "Show B")],
    );
    first.select(0);

    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&first, false);
    component.set_focused(true);
    component.enter_episode_focus();
    component.set_episode_filter(AudiobookshelfEpisodeFilter::Unplayed);

    // New content: show-a is gone. The projected content type no longer
    // carries episode filter / selection / scroll, so the component's own
    // interaction state is all there is -- and it must reset.
    let mut second = AudiobookshelfBrowseState::new(library);
    second.append_page(0, 20, 1, vec![show("show-b", "Show B")]);

    component.set_content(&second, false);
    component.set_focused(true);

    assert!(
        !component.episode_focused(),
        "stale episode-pane focus must reset, not adopt the snapshot's stale value"
    );
    assert_eq!(
        component.episode_filter(),
        AudiobookshelfEpisodeFilter::All,
        "stale episode filter must reset to All, not adopt the snapshot's Played"
    );
}

#[test]
fn abs_podcast_component_keeps_local_show_cursor_and_renders_without_app_state() {
    let app = crate::app::tests_podcast::audiobookshelf_app();
    let state = &app.audiobookshelf_browse[0];
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(state, false);
    component.set_focused(true);

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove { library_item_id })) = message
    else {
        panic!("show movement should carry the resolved show index");
    };
    assert_eq!(library_item_id.as_deref(), Some("show-a"));
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
    let app = crate::app::tests_podcast::audiobookshelf_app();
    let state = &app.audiobookshelf_browse[0];
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(state, false);
    component.set_focused(true);
    component.enter_episode_focus();

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeTransition(transition))) =
        message
    else {
        panic!("episode movement should be a typed episode-transition request, got {message:?}");
    };
    assert!(matches!(transition, PodcastEpisodeTransition::NextEpisode));

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
fn abs_podcast_component_cycles_show_title_buckets_with_brackets() {
    let library = AudiobookshelfLibrary {
        id: "abs-podcasts".into(),
        name: "ABS Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut state = AudiobookshelfBrowseState::new(library);
    state.append_page(
        0,
        20,
        2,
        vec![
            AudiobookshelfShow {
                library_item_id: "alpha".into(),
                title: "Alpha".into(),
                author: None,
                description: None,
                cover_path: None,
            },
            AudiobookshelfShow {
                library_item_id: "zulu".into(),
                title: "Zulu".into(),
                author: None,
                description: None,
                cover_path: None,
            },
        ],
    );

    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);

    for (key, index) in [(Key::Char('['), 1), (Key::Char(']'), 0)] {
        assert_eq!(
            component.on(&Event::Keyboard(KeyEvent {
                code: key,
                modifiers: KeyModifiers::NONE,
            })),
            Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
                library_item_id: if index == 1 {
                    Some("zulu".into())
                } else {
                    Some("alpha".into())
                }
            }))
        );
    }
}

#[test]
fn abs_podcast_component_emits_typed_action_intents_without_raw_key_replay() {
    let state = &crate::app::tests_podcast::audiobookshelf_app().audiobookshelf_browse[0];
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(state, false);
    component.set_focused(true);

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
            ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::FocusOrPlay(
                None
            ))
        ))
    ));

    let enter = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        enter,
        Some(Msg::Shell(
            ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                None
            ))
        ))
    ));

    let ctrl_a = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('a'),
        modifiers: KeyModifiers::CONTROL,
    }));
    assert!(matches!(
        ctrl_a,
        Some(Msg::Shell(
            ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::Enqueue(None))
        ))
    ));

    let unrelated = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('z'),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(unrelated, None);
}

/// 7.2: the parent-owned episode-pane focus is separate from the episode
/// owner's selection, a filter transition re-projects and re-parks the owner at
/// its first row, and activation carries the show-qualified stable target the
/// owner resolved (never a numeric index re-derivation).
#[test]
fn abs_podcast_focus_selection_and_filter_transitions_are_owner_authoritative() {
    let library = AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    };
    let mut state = AudiobookshelfBrowseState::new(library);
    state.append_page(
        0,
        20,
        1,
        vec![AudiobookshelfShow {
            library_item_id: "show-a".into(),
            title: "Show A".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    state.select(0);
    state.episodes = Some(vec![
        AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            title: "Episode A".into(),
            published_at: None,
            duration_seconds: None,
        },
        AudiobookshelfDownloadedEpisode {
            library_item_id: "show-a".into(),
            episode_id: "episode-b".into(),
            title: "Episode B".into(),
            published_at: None,
            duration_seconds: None,
        },
    ]);
    state.progress.insert(
        ("show-a".into(), "episode-a".into()),
        AudiobookshelfProgress {
            library_item_id: "show-a".into(),
            episode_id: "episode-a".into(),
            current_time_seconds: 0.0,
            is_finished: true,
        },
    );

    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);

    // Entering episode focus does not move the episode owner's selection.
    assert_eq!(component.episode_cursor(), 0);
    component.enter_episode_focus();
    assert!(component.episode_focused());
    assert_eq!(component.episode_cursor(), 0);

    // A row-local move changes the owner; focus is unchanged, and the resolved
    // target is show-qualified.
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(component.episode_focused());
    assert_eq!(component.episode_cursor(), 1);
    assert_eq!(
        component.episode_target(),
        Some(PodcastEpisodeTarget::new(
            "show-a".into(),
            "episode-b".into()
        ))
    );

    // Leaving focus does not move the owner's selection.
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Esc,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(!component.episode_focused());
    assert_eq!(component.episode_cursor(), 1);
    assert!(component.episode_target().is_none());

    // A filter transition re-projects the filtered rows and re-parks the owner
    // at its first row (the discrete re-projection boundary design.md D5
    // allows).
    component.enter_episode_focus();
    let filter = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        filter,
        Some(Msg::Shell(
            ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                PodcastEpisodeTransition::NextFilter
            )
        ))
    ));
    assert_eq!(
        component.episode_filter(),
        AudiobookshelfEpisodeFilter::Played
    );
    assert_eq!(component.episode_cursor(), 0);
    assert_eq!(
        component.episode_target(),
        Some(PodcastEpisodeTarget::new(
            "show-a".into(),
            "episode-a".into()
        ))
    );

    // Activation carries the owner-resolved show-qualified target.
    assert_eq!(
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Shell(
            ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                Some(PodcastEpisodeTarget::new(
                    "show-a".into(),
                    "episode-a".into()
                ))
            ))
        ))
    );
}

#[test]
fn abs_podcast_component_returns_none_when_unfocused() {
    let state = &crate::app::tests_podcast::audiobookshelf_app().audiobookshelf_browse[0];
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(state, false);
    component.set_focused(false);
    let cursor = component.cursor();

    for code in [Key::Down, Key::Enter, Key::Char('z')] {
        assert_eq!(
            component.on(&Event::Keyboard(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
            })),
            None
        );
        assert_eq!(component.cursor(), cursor);
    }
}

#[test]
fn abs_podcast_cover_fetch_bridged_to_content_push_and_gated_by_images() {
    // Image-disabled: the fresh-mount content push must not schedule any cover
    // fetch.
    let mut model = Model::new(audiobookshelf_app());
    model.sync_audiobookshelf_podcast();
    assert!(
        model.app.card_image_loading.is_empty(),
        "image-disabled content push must not schedule a cover fetch"
    );

    // Image-enabled with a configured server and secret: the selected show's
    // cover is scheduled through the bridge by the fresh-mount content push.
    let mut app = audiobookshelf_app();
    app.image_protocol_enabled = true;
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(AudiobookshelfSetup::new("https://abs.example"));
    mbv_core::config::save_service_secret(ServiceKind::Audiobookshelf, "test-secret").unwrap();
    let mut model = Model::new(app);
    model.sync_audiobookshelf_podcast();

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

#[test]
fn abs_podcast_refresh_preserves_surviving_target_and_clamps_removed_target() {
    let state = narrow_grid_component_state();
    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, false);
    component.set_focused(true);
    view_narrow(&mut component, 100, 12);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    view_narrow(&mut component, 100, 12);
    let selected = component.selected_id().expect("selected target");
    let selected_cursor = component.cursor();

    let mut refreshed = state.clone();
    refreshed.selected_id = Some(selected.clone());
    refreshed.select(selected_cursor);
    component.set_content(&refreshed, false);
    assert_eq!(component.selected_id().as_deref(), Some(selected.as_str()));
    assert_eq!(
        component.cursor(),
        selected_cursor,
        "refresh keeps a surviving selected target"
    );

    refreshed.shows.truncate(2);
    refreshed.selected_id = Some(selected);
    component.set_content(&refreshed, false);
    view_narrow(&mut component, 100, 12);
    assert_eq!(component.selected_id().as_deref(), Some("show-0"));
    assert_eq!(
        component.cursor(),
        0,
        "refresh clamps when the selected target disappears"
    );
}
