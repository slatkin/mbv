use super::*;
use crate::app::LibEvent;

/// Warm-up arrivals use the same shell event boundary as Inline Search's
/// other library events. This exercises `Model::handle_inline_search_lib_event`
/// rather than calling `App::handle_lib_event` directly.
#[test]
fn inline_search_warmup_event_starts_level_fills_without_opening_a_view() {
    let mut harness = TickHarness::new(crate::app::render::make_music_group_app());
    let mut group = crate::app::tests::make_item("Alpha", "MusicArtist");
    group.id = "group-1".into();
    harness
        .model_mut()
        .handle_inline_search_lib_event(LibEvent::MusicGroupWarmupListed {
            generation: Default::default(),
            groups: vec![group],
        });

    // The fixture has no Emby client, so the attempted level fill reaches the
    // terminal failure state synchronously without creating a live request.
    assert_eq!(
        harness.model().app.album_artist_levels.get("group-1"),
        Some(&crate::app::state::app_struct::LevelFillState::Failed)
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack.len(),
        2,
        "warm-up does not alter the existing browse stack"
    );
}

/// Ctrl+A on a selected Inline Search result reaches the focused destination
/// through a real `Application::tick()`: the text-entry router gate lets the
/// chord fall through to the workspace, which enqueues the selected result row
/// rather than the ordinary album cursor.
#[test]
fn ctrl_a_on_inline_search_result_enqueues_that_result_through_live_tick() {
    let (mut harness, _id) = wide_music_harness();

    // Open Inline Search from the focused Music workspace.
    harness.inject(key(Key::Char('/')));
    harness.step();
    assert!(harness.model().active_inline_search_is_open());

    // Seed a known result row (the shell content push would otherwise supply
    // the library's own albums); an empty query shows no results, so type a
    // character and fire the debounce to score the row before acting on it.
    {
        let workspace = harness.model_mut().test_music_owner_mut();
        let mut result = crate::app::tests::make_item("Result Album", "MusicAlbum");
        result.id = "result-album".into();
        workspace
            .inline_search_mut()
            .set_pool(SearchPool::Items(vec![result]));
    };
    harness.inject(key(Key::Char('a')));
    harness.step();
    harness
        .model_mut()
        .tick_inline_search_clock(Instant::now() + Duration::from_millis(301));

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('a'),
        modifiers: KeyModifiers::CONTROL,
    }));
    let outcome = harness.step();
    assert!(
        outcome.messages.iter().any(|msg| matches!(
            msg,
            Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryEnqueue { item } if item.id == "result-album"))),
        "Ctrl+A acts on the selected Inline Search result: {:?}",
        outcome.messages
    );
    assert!(
        !harness.model().active_inline_search_is_open(),
        "launching a result exits search so focus is not trapped in the text entry"
    );
}

/// A shell clock tick must invoke the destination host's debounce callback,
/// not merely clear the shared editor's deadline. The future clock is injected
/// directly; the test never waits for wall-clock time.
#[test]
fn inline_search_debounce_applies_grouped_music_filter_through_shell_host() {
    let mut app = crate::app::render::make_music_group_app();
    let mut second_album = crate::app::tests::make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0]
        .nav_stack
        .last_mut()
        .expect("music album level")
        .items
        .push(second_album);
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    harness.inject(key(Key::Char('/')));
    harness.step();
    assert!(harness.model().active_inline_search_is_open());

    for character in "second".chars() {
        harness.inject(key(Key::Char(character)));
        harness.step();
    }
    assert_eq!(
        harness.model().test_music_owner().inline_search().query(),
        "second"
    );
    assert!(
        harness
            .model_mut()
            .tick_inline_search_clock(Instant::now() + Duration::from_millis(301)),
        "the injected clock fires the pending debounce"
    );

    let owner = harness.model().test_music_owner();
    let titles: Vec<&str> = owner
        .browser
        .visible_targets()
        .iter()
        .filter_map(|target| owner.browser.node(target).map(|node| node.title.as_str()))
        .collect();
    assert_eq!(
        titles,
        ["Alpha", "Second Album"],
        "the shell debounce callback applies the query to the retained tree"
    );
}

/// Enter on an album result is fully asynchronous: `activate_recursive_album`
/// needs an Emby runtime client (absent in this harness) so it returns `false`,
/// and the synchronous branch must then change nothing -- the search stays open
/// and no track-focus / re-anchor one-shot is armed. (Regression: task 2.1's
/// first cut dismissed the search and armed the flags synchronously, stranding
/// the user in track-selection on the pre-search album when activation failed.)
#[test]
fn enter_on_inline_search_album_result_defers_to_async_activation() {
    let (mut harness, id) = wide_music_harness();

    harness.inject(key(Key::Char('/')));
    harness.step();
    assert!(harness.model().active_inline_search_is_open());

    let album = album_row("album-1", "First Album");
    harness.model_mut().app.album_indexes.insert(
        "lib-music".into(),
        crate::app::AlbumIndexState::Ready(std::sync::Arc::new(crate::app::AlbumIndex::new(vec![
            crate::app::AlbumSearchEntry {
                album: album.clone(),
                ancestors: Vec::new(),
                display_label: "First Album".into(),
                search_text: "first album".into(),
            },
        ]))),
    );
    {
        let workspace = harness.model_mut().test_music_owner_mut();
        workspace
            .inline_search_mut()
            .set_pool(SearchPool::Albums(vec![crate::app::AlbumSearchEntry {
                album,
                ancestors: Vec::new(),
                display_label: "First Album".into(),
                search_text: "first album".into(),
            }]));
    };

    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        harness.model().active_inline_search_is_open(),
        "a failed recursive activation leaves Inline Search open"
    );
    assert_eq!(
        music_track_focus_row(&harness, &id),
        None,
        "a failed recursive activation does not enter track-selection mode"
    );
}

/// The `RecursiveAlbumActivated` lib event is the sole owner of the return to
/// the standard Music presentation: it replaces the nav stack, then re-anchors
/// the workspace onto the activated album at its natural list position and
/// enters track-selection for THAT album -- not for whatever album sat under
/// the pre-search cursor.
#[test]
fn recursive_album_activation_event_reanchors_onto_the_activated_album() {
    let (mut harness, id) = wide_music_harness();

    // Two sibling albums; the pre-search cursor rests on the first.
    assert_eq!(music_album_cursor(&harness, &id), 0);

    // The async worker replaces the whole nav stack with the resolved path;
    // its resting cursor points at the activated album (the second sibling,
    // "album-1", whose tracks the harness has cached).
    let nav_stack = vec![
        crate::app::BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-music".into(),
            title: "Music".into(),
            items: vec![album_row("group-0", "Alpha")],
            total_count: 1,
            resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        },
        crate::app::BrowseLevel {
            fetched_rows: 0,
            parent_id: "group-0".into(),
            title: "Alpha".into(),
            items: vec![
                album_row("album-0", "Zeroth Album"),
                album_row("album-1", "First Album"),
            ],
            total_count: 2,
            resting: crate::app::state::types::browse::BrowseResting::new(1, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        },
    ];

    // Drive the production `LibEvent::RecursiveAlbumActivated` arm
    // (shell/run.rs): App installs the path, then the shell arms the re-anchor
    // + track-focus one-shots and re-projects the workspace.
    harness
        .model_mut()
        .app
        .handle_lib_event(crate::app::LibEvent::RecursiveAlbumActivated {
            library_id: "lib-music".into(),
            nav_stack,
        });
    harness.model_mut().music_track_focus_request =
        Some(crate::app::shell::MusicTrackFocusRequest::Enter {
            album_id: "album-1".into(),
        });
    harness.model_mut().music_workspace_reanchor = true;
    harness.model_mut().push_music_workspace_content();
    harness.model_mut().sync_mounted_surfaces();

    assert_eq!(
        music_album_cursor(&harness, &id),
        1,
        "the workspace re-anchors onto the activated album's natural position"
    );
    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the focused album is the activated one, not the pre-search cursor"
    );
    assert_eq!(
        music_track_focus_row(&harness, &id),
        Some(0),
        "track-selection mode is entered for the activated album"
    );
}
