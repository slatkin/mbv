use super::*;

#[test]
fn launch_reanchor_restores_podcast_show_pill_before_detail_arrival() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_browse[0].detail_cache.clear();
    app.audiobookshelf_browse[0]
        .detail_loading_ids
        .insert("show-a".into(), 0);
    app.pending_launch_tab_resolved = true;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Audiobookshelf,
            library_id: "abs-podcasts".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Audiobookshelf {
            key: mbv_core::config::AudiobookshelfSelectorKey::PodcastShow("show-a".into()),
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Audiobookshelf {
            id: "show-a\0episode-restored".into(),
        }),
    });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().app.pending_launch_state.is_some());
    assert_eq!(
        podcast(&mut harness).pill(),
        &crate::app::state::types::audiobookshelf_browse::PillSelection::Show("show-a".into())
    );
    assert!(podcast(&mut harness).selected_episode_target().is_none());
    let selector = podcast(&mut harness)
        .content()
        .selector
        .expect("podcast selector");
    assert_eq!(
        selector.active,
        Some(4),
        "the saved show pill is the active painted selector"
    );

    let generation = harness.model().app.audiobookshelf_runtime.generation();
    harness
        .model_mut()
        .app
        .handle_lib_event(crate::app::LibEvent::AudiobookshelfDetailFetched {
            generation,
            request: 0,
            library_item_id: "show-a".into(),
            result: Ok(vec![episode("show-a", "episode-restored")]),
        });
    harness.model_mut().push_audiobookshelf_podcast_content();
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().app.pending_launch_state.is_none());
    assert_eq!(
        podcast(&mut harness)
            .selected_episode_target()
            .unwrap()
            .episode_id(),
        "episode-restored"
    );
}

#[test]
fn launch_reanchor_restores_podcast_filter_pill_on_cold_start() {
    let mut app = audiobookshelf_app();
    app.pending_launch_tab_resolved = true;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Audiobookshelf,
            library_id: "abs-podcasts".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Audiobookshelf {
            key: mbv_core::config::AudiobookshelfSelectorKey::PodcastFilter(
                mbv_core::config::AudiobookshelfPodcastFilter::Unplayed,
            ),
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Audiobookshelf {
            id: "show-a\0episode-a".into(),
        }),
    });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().app.pending_launch_state.is_none());
    assert_eq!(
        podcast(&mut harness).pill(),
        &crate::app::state::types::audiobookshelf_browse::PillSelection::State(
            crate::app::state::types::audiobookshelf_browse::AudiobookshelfEpisodeFilter::Unplayed,
        )
    );
    assert_eq!(
        podcast(&mut harness)
            .selected_episode_target()
            .expect("the saved unplayed episode is selected")
            .episode_id(),
        "episode-a"
    );
    let selector = podcast(&mut harness)
        .content()
        .selector
        .expect("podcast selector");
    assert_eq!(
        selector.active,
        Some(2),
        "the saved filter pill is the active selector"
    );
}

#[test]
fn podcast_owner_survives_tab_reselection_with_the_remembered_pill() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_libraries
        .push(mbv_core::audiobookshelf::AudiobookshelfLibrary {
            id: "abs-books".into(),
            name: "Books".into(),
            media_type: "book".into(),
        });
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    let before = podcast(&mut harness).pill().clone();
    // Switching to another Service destination in the same column must not
    // disturb the pill either (the owner is retained while its library stays
    // in the catalog; only the list selection re-anchors on reactivation).
    // The crossing really resolves `AudiobookshelfLibrary(1)` to the Books
    // destination: the shell registers that tab's owner as part of the sync
    // pass, so the pill survives an actual destination change, not a no-op.
    let book_key = crate::app::components::LibraryKey::Service {
        service: ServiceKind::Audiobookshelf,
        library_id: "abs-books".into(),
        kind: LibraryKind::AudiobookshelfBook,
    };
    harness.model_mut().app.tab = crate::app::TabSelection::AudiobookshelfLibrary(1);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness.model().library_panel_has_owner(&book_key),
        "the crossing resolved AudiobookshelfLibrary(1) to the Books destination"
    );
    harness.model_mut().app.tab = crate::app::TabSelection::AudiobookshelfLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.tab = crate::app::TabSelection::AudiobookshelfLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(podcast(&mut harness).pill(), &before);
    let _ = TerminalObserverEvent::NoOp;
}

/// A keyboard show-pill commit scopes the shell's episode fan-out to that
/// show (reorganize-podcast-pill-navigation 3.3, design D5): the committed
/// pill's identity lands in the App's browse state through the shell sync
/// pass and its message dispatch, and wrapping back to a state pill returns
/// the scope to every listed show.
#[test]
fn keyboard_show_pill_commit_scopes_the_fan_out_through_the_shell() {
    let mut app = audiobookshelf_app();
    // A second show and no cached episodes: the pill bar has two show pills
    // and neither show's episodes are fetched.
    app.audiobookshelf_browse[0].append_page(
        0,
        20,
        2,
        vec![mbv_core::audiobookshelf::AudiobookshelfShow {
            library_item_id: "show-b".into(),
            title: "Show B".into(),
            author: None,
            description: None,
            cover_path: None,
        }],
    );
    app.audiobookshelf_browse[0].detail_cache.clear();
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let step_and_dispatch = |harness: &mut TickHarness| {
        let outcome = harness.step();
        let (mut music, mut tv) = (false, false);
        for message in outcome.messages {
            harness
                .model_mut()
                .handle_terminal_message(message, &mut music, &mut tv);
        }
    };

    // Walk to the first show pill (All -> Unplayed -> Played -> Show A).
    for _ in 0..3 {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Char(']'),
            modifiers: KeyModifiers::NONE,
        }));
        step_and_dispatch(&mut harness);
    }
    assert_eq!(
        harness.model().app.audiobookshelf_browse[0]
            .committed_show_pill
            .as_deref(),
        Some("show-a"),
        "the committed show pill scopes the fan-out"
    );

    // Wrapping forward past the last show pill (Show B and Latest) lands on
    // `All`: the scope follows the state pill.
    for _ in 0..3 {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Char(']'),
            modifiers: KeyModifiers::NONE,
        }));
        step_and_dispatch(&mut harness);
    }
    assert_eq!(
        harness.model().app.audiobookshelf_browse[0].committed_show_pill,
        None,
        "a state pill's scope is every listed show"
    );
}
