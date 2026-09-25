use super::*;

#[test]
fn podcast_latest_uses_cached_shelf_and_resolves_provider_targets_without_emby() {
    let app = audiobookshelf_app();
    let mut harness = TickHarness::new(app);
    draw(&mut harness, 80);
    assert!(harness.model().app.emby_client().is_none());
    // Latest remains selectable while its cached shelf is empty.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('['),
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    assert_eq!(
        podcast(&mut harness).pill(),
        &crate::app::state::types::audiobookshelf_browse::PillSelection::Latest
    );
    assert!(podcast(&mut harness).episode_rows().is_empty());

    // Deliver the existing shelf-fetch completion; the normal shell projection
    // updates Latest without fetching on selector entry or refreshing shows.
    let item = mbv_core::playback_queue::AudiobookshelfQueueItem {
        library_item_id: "shelf-show".into(),
        episode_id: "shelf-episode".into(),
        title: "Shelf episode".into(),
        show_title: Some("Shelf show".into()),
        author: None,
        description: Some("Shelf description".into()),
        duration_ticks: Some(90_000_000),
        position_ticks: 0,
        played: false,
        pub_date_secs: Some(1_700_000_000),
        is_finished: false,
        cover_path: None,
    };
    let generation = harness.model().app.audiobookshelf_runtime.generation();
    harness
        .model_mut()
        .app
        .handle_lib_event(crate::app::LibEvent::AudiobookshelfShelfFetched {
            generation,
            library_id: "abs-podcasts".into(),
            result: Ok(vec![mbv_core::audiobookshelf::AudiobookshelfShelf {
                label: "Newest Episodes".into(),
                entries: vec![mbv_core::audiobookshelf::AudiobookshelfShelfEntry::Episode(
                    item.clone(),
                )],
            }]),
        });
    harness.model_mut().push_audiobookshelf_podcast_content();
    draw(&mut harness, 80);
    assert_eq!(
        podcast(&mut harness).pill(),
        &crate::app::state::types::audiobookshelf_browse::PillSelection::Latest
    );
    assert!(
        !podcast(&mut harness).content().selector.unwrap().markers[0],
        "selection before async completion acknowledges the Latest marker"
    );
    let target = podcast(&mut harness)
        .selected_episode_target()
        .expect("shelf target selected");
    assert_eq!(
        (target.library_item_id(), target.episode_id()),
        ("shelf-show", "shelf-episode")
    );
    assert!(podcast(&mut harness).episode_rows().iter().any(|row| matches!(row,
        MediaListRow::Item { target: row_target, primary, .. } if row_target == &target && primary == "Shelf show")));
    draw(&mut harness, 160);
    assert_eq!(
        podcast(&mut harness).selected_episode_target().as_ref(),
        Some(&target)
    );
    draw(&mut harness, 80);

    // Both activation intents carry the provider-native shelf target, and
    // the shell resolver returns the shelf's full playable snapshot.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let play = harness.step();
    assert!(play.raw_messages.iter().any(|msg| matches!(msg,
        Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::AudiobookshelfPodcastEpisodeIntent(
            crate::app::components::msg::PodcastEpisodeIntent::OpenOrPlay(Some(target))
        ) if target.library_item_id() == "shelf-show" && target.episode_id() == "shelf-episode"))));
    let resolved = harness
        .model()
        .app
        .selected_audiobookshelf_queue_item_target(0, &target)
        .expect("provider target resolves from this library's shelf");
    assert!(
        matches!(resolved, mbv_core::playback_queue::QueueItem::Audiobookshelf(episode) if episode.title == "Shelf episode" && episode.description.as_deref() == Some("Shelf description"))
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('a'),
        modifiers: KeyModifiers::CONTROL,
    }));
    let enqueue = harness.step();
    assert!(enqueue.raw_messages.iter().any(|msg| matches!(msg,
        Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::AudiobookshelfPodcastEpisodeIntent(
            crate::app::components::msg::PodcastEpisodeIntent::Enqueue(Some(target))
        ) if target.library_item_id() == "shelf-show" && target.episode_id() == "shelf-episode"))));
}

#[test]
fn podcast_latest_marker_is_visible_until_the_pill_is_selected() {
    let mut app = audiobookshelf_app();
    app.home_latest_launch_window = crate::app::state::home_latest::HomeLatestLaunchWindow {
        previous: Some(1_600_000_000),
        current: 1_800_000_000,
    };
    app.audiobookshelf_shelf_cache.insert(
        "abs-podcasts".into(),
        vec![mbv_core::playback_queue::QueueItem::Audiobookshelf(
            mbv_core::playback_queue::AudiobookshelfQueueItem {
                library_item_id: "show-a".into(),
                episode_id: "new-episode".into(),
                title: "New".into(),
                show_title: Some("Show A".into()),
                author: None,
                description: None,
                duration_ticks: None,
                position_ticks: 0,
                played: false,
                pub_date_secs: Some(1_700_000_000),
                is_finished: false,
                cover_path: None,
            },
        )],
    );
    let mut harness = TickHarness::new(app);
    harness.model_mut().app.home_latest_launch_window =
        crate::app::state::home_latest::HomeLatestLaunchWindow {
            previous: Some(1_600_000_000),
            current: 1_800_000_000,
        };
    harness.model_mut().sync_mounted_surfaces();
    assert!(podcast(&mut harness).content().selector.unwrap().markers[0]);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('['),
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(!podcast(&mut harness).content().selector.unwrap().markers[0]);
}

#[test]
fn podcast_latest_launch_snapshot_is_selected_tab_only() {
    let mut app = audiobookshelf_app();
    let second = mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "abs-podcasts-2".into(),
        name: "Second podcast library".into(),
        media_type: "podcast".into(),
    };
    app.audiobookshelf_browse.push(
        crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseState::new(
            second.clone(),
        ),
    );
    app.audiobookshelf_libraries.push(second);
    app.pending_launch_tab_resolved = true;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Audiobookshelf,
            library_id: "abs-podcasts".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Audiobookshelf {
            key: mbv_core::config::AudiobookshelfSelectorKey::Latest,
        }),
        item: None,
    });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        podcast(&mut harness).pill(),
        &crate::app::state::types::audiobookshelf_browse::PillSelection::Latest
    );
    let (selector, _) = podcast(&mut harness).launch_snapshot();
    assert_eq!(
        selector,
        Some(mbv_core::config::SelectorIdentity::Audiobookshelf {
            key: mbv_core::config::AudiobookshelfSelectorKey::Latest
        })
    );

    harness.model_mut().app.tab = crate::app::TabSelection::AudiobookshelfLibrary(1);
    harness.model_mut().sync_mounted_surfaces();
    assert!(matches!(
        podcast_for(&mut harness, "abs-podcasts-2").pill(),
        crate::app::state::types::audiobookshelf_browse::PillSelection::State(
            crate::app::state::types::audiobookshelf_browse::AudiobookshelfEpisodeFilter::All
        )
    ));
}
