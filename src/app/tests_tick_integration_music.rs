use super::*;

fn wide_music_harness() -> (TickHarness, ComponentId) {
    let mut app = crate::app::render::make_music_group_app();
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut first = crate::app::tests::make_item("Track One", "Audio");
    first.id = "track-1".into();
    let mut second = crate::app::tests::make_item("Track Two", "Audio");
    second.id = "track-2".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![first, second]);
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    (harness, ComponentId::Library)
}

/// The focused track-pane row, or `None` when the track pane is unfocused
/// (design.md D5: focus is parent state, the owner holds the selection).
fn music_track_focus_row(harness: &TickHarness, _id: &ComponentId) -> Option<usize> {
    let music = harness.model().test_music_owner();
    music
        .track_focused()
        .then(|| music.track_selected_row())
        .flatten()
}

fn music_workspace<'a>(harness: &'a TickHarness, _id: &ComponentId) -> &'a MusicContent {
    harness.model().test_music_owner()
}

fn music_album_cursor(harness: &TickHarness, id: &ComponentId) -> usize {
    music_workspace(harness, id).album_cursor()
}

fn music_selected_album_id(harness: &TickHarness, id: &ComponentId) -> Option<String> {
    music_workspace(harness, id)
        .selected_item()
        .map(|item| item.id)
}

/// A Library → Queue → Library round trip through real `Application::tick()`:
/// the Music workspace cannot navigate while Queue holds focus, navigates
/// immediately when Library focus returns (no click, no content refresh
/// between the focus return and the key), and keeps its private track-pane
/// selection across the whole trip.
#[test]
fn music_library_queue_library_round_trip_keeps_focus_and_pane_state() {
    let (mut harness, id) = wide_music_harness();
    assert_eq!(harness.model().application.focus(), Some(&id));

    // Enter the inline track pane, then move the track cursor: private pane
    // state a blur must not disturb.
    harness.inject(key(Key::Enter));
    harness.step();
    assert_eq!(music_track_focus_row(&harness, &id), Some(0));
    harness.inject(key(Key::Down));
    harness.step();
    assert_eq!(music_track_focus_row(&harness, &id), Some(1));

    // Panel focus moves to Queue through the production sync order.
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Queue)
    );

    // A Music navigation key while blurred does not reach the workspace.
    harness.inject(key(Key::Up));
    let raw = harness
        .model_mut()
        .application
        .tick(PollStrategy::Once(Duration::from_millis(500)))
        .expect("tick blurred music");
    assert!(!raw
        .iter()
        .any(|msg| matches!(msg, Msg::Shell(ShellRequest::MusicTrackActivate { .. }))));
    assert_eq!(
        music_track_focus_row(&harness, &id),
        Some(1),
        "blurred Music must not navigate its track pane"
    );

    // Library regains focus through the destination pass only — no content
    // push.
    harness.model_mut().app.panel_focus = PanelFocus::Library;
    harness.model_mut().sync_active_destination();
    assert_eq!(harness.model().application.focus(), Some(&id));
    assert_eq!(
        music_track_focus_row(&harness, &id),
        Some(1),
        "the private track cursor survives the focus round trip"
    );

    // Keyboard navigation lands immediately, with no click and no content
    // refresh between the focus return and the key.
    harness.inject(key(Key::Up));
    harness
        .model_mut()
        .application
        .tick(PollStrategy::Once(Duration::from_millis(500)))
        .expect("tick refocused music");
    assert_eq!(
        music_track_focus_row(&harness, &id),
        Some(0),
        "Music navigates immediately once Library focus returns"
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
    }
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
            Msg::Shell(ShellRequest::EmbyLibraryEnqueue { item }) if item.id == "result-album"
        )),
        "Ctrl+A acts on the selected Inline Search result: {:?}",
        outcome.messages
    );
    assert!(
        !harness.model().active_inline_search_is_open(),
        "launching a result exits search so focus is not trapped in the text entry"
    );
}

fn album_row(id: &str, name: &str) -> mbv_core::api::EmbyItem {
    let mut album = crate::app::tests::make_item(name, "MusicAlbum");
    album.id = id.into();
    album
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
        crate::app::AlbumIndexState::Ready(vec![crate::app::AlbumSearchEntry {
            album: album.clone(),
            ancestors: Vec::new(),
            display_label: "First Album".into(),
            search_text: "first album".into(),
        }]),
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
    }

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
            parent_id: "lib-music".into(),
            title: "Music".into(),
            items: vec![album_row("group-0", "Alpha")],
            total_count: 1,
            resting: crate::app::types_browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        },
        crate::app::BrowseLevel {
            parent_id: "group-0".into(),
            title: "Alpha".into(),
            items: vec![
                album_row("album-0", "Zeroth Album"),
                album_row("album-1", "First Album"),
            ],
            total_count: 2,
            resting: crate::app::types_browse::BrowseResting::new(1, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        },
    ];

    // Drive the production `LibEvent::RecursiveAlbumActivated` arm
    // (shell_run.rs): App installs the path, then the shell arms the re-anchor
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

/// Blocking-overlay focus loss and restoration through live `Application::tick()`:
/// raising a blocking confirm modal moves keyboard delivery off the focused
/// Queue; dismissing it the production way restores Queue focus with no
/// focus-only content projection, and Queue receives keys again immediately.
#[test]
fn blocking_overlay_focus_loss_and_restoration_through_live_tick() {
    let mut harness = queue_focused_harness();
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Queue)
    );

    harness.model_mut().app.pending_overlay = Some(OverlayRequest::Confirm(ConfirmModal {
        title: "Clear queue?".into(),
        message: "Remove queued items".into(),
        hint: "[y] Confirm    [Esc] Cancel".into(),
        on_confirm: ConfirmAction::ClearQueue,
    }));
    harness.model_mut().sync_mounted_surfaces();
    let confirm_id = ComponentId::Modal(ModalId::Confirm);
    assert_eq!(harness.model().application.focus(), Some(&confirm_id));

    // The blocking modal, not Queue, receives keyboard input while it is up.
    harness.inject(key(Key::Char('y')));
    let outcome = harness.step();
    assert_eq!(outcome.pre_fold_focus, Some(confirm_id.clone()));
    assert!(outcome.raw_messages.iter().any(|msg| matches!(
        msg,
        Msg::Shell(ShellRequest::ConfirmIntent(ConfirmIntent::Accept))
    )));

    // Dismiss the modal the production way; the next sync pass restores focus
    // to the underlying Queue without a focus-only projection.
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::ConfirmIntent(ConfirmIntent::Accept)),
        &mut music_resize,
        &mut tv_resize,
    );
    harness.model_mut().sync_mounted_surfaces();
    assert!(!harness.model().application.mounted(&confirm_id));
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Queue),
        "overlay dismiss restores focus to the underlying Queue"
    );

    harness.inject(key(Key::Char('[')));
    let outcome = harness.step();
    assert_eq!(outcome.pre_fold_focus, Some(ComponentId::Queue));
    assert!(outcome.raw_messages.iter().any(|msg| matches!(
        msg,
        Msg::Queue(QueueRequest::Scope(crate::app::QueueScope::Local))
    )));
}

/// Playlist-Enter regression probe (Path A: Enter on an open playlist's item).
/// Drives a real `Application::tick()`: the Playlists sidebar is mounted via
/// the production overlay request, Enter is injected, the surviving messages
/// are dispatched through `handle_terminal_message`, and the production sync
/// pass runs. Asserts the sidebar unmounts, the queue populates, and panel
/// focus moves to Queue.
#[test]
fn playlist_enter_replaces_queue_dismisses_sidebar_and_focuses_queue() {
    use crate::app::tests::make_item;
    let mut app = make_app_stub();
    let playlist = make_item("P1", "Playlist");
    let mut song = make_item("Song", "Audio");
    song.id = "item-1".into();
    app.playlists = vec![playlist.clone()];
    app.playlists_cursor = 0;
    app.playlists_open = Some(playlist);
    app.playlists_open_items = vec![song];
    app.playlists_open_cursor = 0;
    let mut harness = TickHarness::new(app);
    harness.model_mut().app.pending_overlay = Some(OverlayRequest::OpenSidebar(
        crate::app::SidebarId::Playlists,
    ));
    harness.model_mut().sync_mounted_surfaces();
    let playlists_id = ComponentId::Overlay(OverlayId::Playlists);
    assert!(harness.model().application.mounted(&playlists_id));
    assert_eq!(
        harness.model().application.focus(),
        Some(&playlists_id),
        "Playlists sidebar owns focus while mounted"
    );

    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    assert_eq!(outcome.pre_fold_focus, Some(playlists_id.clone()));
    assert!(matches!(outcome.router, RouterOutcome::FallThrough));
    assert!(
        outcome.messages.iter().any(|m| matches!(
            m,
            Msg::Shell(ShellRequest::PlaylistsActivate {
                open: true,
                index: 0
            })
        )),
        "Enter on the open playlist item emits PlaylistsActivate"
    );

    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert_eq!(
        harness.model().app.playback_queue().total_queue_len(),
        1,
        "the queue populates with the playlist item"
    );
    assert!(
        !harness.model().application.mounted(&playlists_id),
        "the Playlists sidebar closes after Enter"
    );
    assert_eq!(
        harness.model().app.effective_panel_focus(),
        PanelFocus::Queue,
        "panel focus moves to Queue after Enter"
    );
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Queue),
        "TuiRealm focus lands on the Queue component"
    );
}

/// Finding 1: `.` is a selection-dependent chord, so the central router falls
/// it through to the focused `QueueComponent`; the emitted `RowContextMenu`
/// request is dispatched by the shell into a pending context-menu overlay.
#[test]
fn tick_routes_dot_to_focused_queue_and_opens_the_context_menu() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    app.player_tab.set_queue_items(
        vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(
            crate::app::tests::make_item("queued", "Movie"),
        ))],
        0,
    );
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Queue)
    );

    harness.inject(key(Key::Char('.')));
    let outcome = harness.step();

    assert_eq!(outcome.pre_fold_focus, Some(ComponentId::Queue));
    assert!(matches!(outcome.router, RouterOutcome::FallThrough));
    assert!(
        outcome
            .messages
            .iter()
            .any(|m| matches!(m, Msg::Shell(ShellRequest::RowContextMenu(crate::app::types_context_menu::ContextMenuTargets::Queue(_), _)))),
        "`.` falls through to the focused Queue component"
    );

    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    assert!(
        matches!(
            harness.model().app.pending_overlay,
            Some(OverlayRequest::ContextMenu(_))
        ),
        "dispatching RowContextMenu opens a context-menu overlay"
    );
}

fn landed_album_level(
    parent_id: &str,
    title: &str,
    items: Vec<mbv_core::api::EmbyItem>,
) -> crate::app::BrowseLevel {
    crate::app::BrowseLevel {
        parent_id: parent_id.into(),
        title: title.into(),
        total_count: items.len(),
        items,
        resting: crate::app::types_browse::BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    }
}

/// The landed nav stack the recursive album activation builds for a grouped
/// library: root artist level + the artist's album level, the navigated album
/// at the album level's resting cursor. The sibling album is present (the
/// activation fetches the whole parent listing), so preserving the owner's
/// prior target would miss the navigated album without the re-anchor.
fn landed_grouped_album_stack(album_id: &str) -> Vec<crate::app::BrowseLevel> {
    let mut artist = crate::app::tests::make_item("Alpha", "MusicArtist");
    artist.id = "group-0".into();
    artist.is_folder = true;
    let mut album = crate::app::tests::make_item("First Album", "MusicAlbum");
    album.id = album_id.into();
    album.artist = "Alpha".into();
    let mut sibling = crate::app::tests::make_item("Second Album", "MusicAlbum");
    sibling.id = "album-2".into();
    sibling.artist = "Alpha".into();
    vec![
        landed_album_level("lib-music", "Music", vec![artist]),
        landed_album_level("group-0", "Alpha", vec![album, sibling]),
    ]
}

/// Task 3.2 (grouped shape): a navigated album lands through the same
/// `RecursiveAlbumActivated` shell arm Inline Search activation uses, so the
/// retained Music owner's workspace re-anchors onto the navigated album and
/// shows its track list -- even after the user moved the owner's own cursor
/// elsewhere.
#[test]
fn navigated_album_reanchors_the_grouped_owner_workspace() {
    let (mut harness, id) = wide_music_harness();
    // A sibling album so the owner's own cursor can move off the navigated
    // one before the landing.
    {
        let app = &mut harness.model_mut().app;
        let mut second = crate::app::tests::make_item("Second Album", "MusicAlbum");
        second.id = "album-2".into();
        second.artist = "Alpha".into();
        let level = app.libs[0].nav_stack.last_mut().expect("album level");
        level.items.push(second);
        level.total_count = 2;
        app.album_tracks_cache
            .insert("album-2".into(), vec![crate::app::tests::make_item("Other Track", "Audio")]);
    }
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(key(Key::Down));
    harness.step();
    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-2"),
        "the owner's local cursor moved off the navigated album"
    );

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the owner workspace re-anchors onto the navigated album"
    );
    assert_eq!(
        music_track_focus_row(&harness, &id),
        Some(0),
        "track-selection mode is entered for the navigated album's track list"
    );
}

/// Task 3.2 (flat shape): an album directly under the library root lands as a
/// one-level stack whose resting cursor is on the album. The group-view owner
/// only exists above a group level, so the flat surface is the library
/// browser; the arm still lands the album at the root cursor.
#[test]
fn navigated_flat_album_lands_the_album_at_the_root_cursor() {
    let (mut harness, _id) = wide_music_harness();
    let mut album = crate::app::tests::make_item("First Album", "MusicAlbum");
    album.id = "album-1".into();
    harness.model_mut().on_recursive_album_activated(
        "lib-music".into(),
        vec![landed_album_level("lib-music", "Music", vec![album])],
    );
    harness.step();

    let level = harness.model().app.libs[0]
        .nav_stack
        .last()
        .expect("landed level");
    assert_eq!(level.items[level.resting().cursor()].id, "album-1");
    assert!(
        harness.model().music_owner().is_none(),
        "the flat shape has no group-view owner to re-anchor"
    );
}
