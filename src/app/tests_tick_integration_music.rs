use super::*;
use crate::app::LibEvent;

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
        Some(&crate::app::app_struct::LevelFillState::Failed)
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack.len(),
        2,
        "warm-up does not alter the existing browse stack"
    );
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
        fetched_rows: 0,
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
        fetched_rows: 0,
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
        fetched_rows: 0,
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

/// Task 6.2 (design D6): "Go to Library" on a queued track selects the track
/// in the workspace track list once the activated album's track rows arrive —
/// including when the fetch lands after the landing.
#[test]
fn navigated_track_is_selected_in_the_workspace_track_list() {
    let (mut harness, id) = wide_music_harness();
    // The queued track: the navigation carries it as deep selection bound to
    // the activated album.
    harness.model_mut().app.pending_track_selection = Some((0, "track-2".into()));

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the album landing stands"
    );
    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(1),
        "the chosen track is selected in the track list"
    );
    assert!(
        harness.model().pending_music_track_selection.is_none(),
        "the deep selection is fully consumed"
    );
    assert_ne!(
        harness.model().app.status_severity,
        crate::app::notify_actions::ToastSeverity::Error,
        "a successful deep selection does not flash: {}",
        harness.model().app.status
    );
}

/// Task 6.2: the track is absent from the fetched track list — the album
/// landing stands with the default selection and no error.
#[test]
fn absent_track_keeps_the_landing_with_default_selection() {
    let (mut harness, id) = wide_music_harness();
    harness.model_mut().app.pending_track_selection = Some((0, "track-gone".into()));

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the album landing stands"
    );
    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(0),
        "default selection: first track"
    );
    assert!(
        harness.model().pending_music_track_selection.is_none(),
        "the absent-track pending is cleared"
    );
    assert_ne!(
        harness.model().app.status_severity,
        crate::app::notify_actions::ToastSeverity::Error,
        "absence is not failure: {}",
        harness.model().app.status
    );
}

/// Task 6.2: the album's tracks have not arrived when the activation drains —
/// the pending selection stays armed and applies on the tracks re-push.
#[test]
fn navigated_track_selection_waits_for_the_album_tracks() {
    let mut app = crate::app::render::make_music_group_app();
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _id = ComponentId::Library;

    harness.model_mut().app.pending_track_selection = Some((0, "track-2".into()));
    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();
    assert!(
        harness.model().pending_music_track_selection.is_some(),
        "the pending selection stays armed while the track fetch is in flight"
    );

    // The tracks arrive (the `AlbumTracksFetched` drain re-pushes).
    let mut first = crate::app::tests::make_item("Track One", "Audio");
    first.id = "track-1".into();
    let mut second = crate::app::tests::make_item("Track Two", "Audio");
    second.id = "track-2".into();
    harness
        .model_mut()
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![first, second]);
    harness.model_mut().sync_mounted_surfaces();

    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(1),
        "the chosen track is selected once its rows arrive"
    );
    assert!(harness.model().pending_music_track_selection.is_none());
}

// ── Task 2.3: the tree is the one Grouped Music browser owner and painter ──

fn music_panel(harness: &TickHarness) -> &crate::app::components::library_panel::LibraryPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        .expect("LibraryPanel")
}

/// Draw one frame at the model's own terminal size (the live paint path).
fn draw_music_frame(harness: &mut TickHarness) {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .expect("music frame");
}

/// The mounted Music app at one Panel-mode fixture, drawn once.
fn mounted_music_at(width: u16, height: u16) -> (TickHarness, ComponentId) {
    mounted_music_app_at(crate::app::render::make_music_group_app(), width, height)
}

/// The mounted Music app for a supplied fixture at one Panel-mode geometry,
/// drawn once.
fn mounted_music_app_at(
    mut app: crate::app::App,
    width: u16,
    height: u16,
) -> (TickHarness, ComponentId) {
    app.terminal_width = width;
    app.terminal_height = height;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    // A single-panel fixture keeps the Library panel's own area the test input
    // at every breakpoint instead of a split whose pane size is an arrangement
    // fact.
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    (harness, ComponentId::Library)
}

fn music_panel_mut(harness: &mut TickHarness) -> &mut crate::app::components::library_panel::LibraryPanel {
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any_mut()
        .downcast_mut::<crate::app::components::library_panel::LibraryPanel>()
        .expect("LibraryPanel")
}

/// The tree's summary, context origin, and capability gate all cross the
/// mounted composition path: modified clicks mutate the owner, the status
/// projection observes only its count, and folder albums cannot manufacture
/// context actions merely because they were selected.
#[test]
fn grouped_music_tree_selection_projects_status_and_context_origin() {
    let mut app = crate::app::render::make_music_group_app();
    let mut second = crate::app::tests::make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    second.is_folder = true;
    app.libs[0].nav_stack[1].items[0].is_folder = true;
    app.libs[0].nav_stack[1].items.push(second);
    app.terminal_width = 100;
    app.terminal_height = 30;
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    let album_points = {
        let music = harness.model().test_music_owner();
        ["album-1", "album-2"].map(|target| {
            let node = music
                .browser
                .projected_nodes()
                .iter()
                .find(|node| music.browser.target_of(node.id()) == Some(target))
                .expect("painted album node");
            let row = music.browser.row_rect_for(node.id()).expect("painted album row");
            (row.x, row.y)
        })
    };
    let click = |column, row, modifiers| Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers,
    });
    for (column, row) in album_points {
        harness.inject(click(
            column,
            row,
            KeyModifiers::CONTROL,
        ));
        let outcome = harness.step();
        let (mut music_resize, mut tv_resize) = (false, false);
        for message in outcome.messages {
            harness
                .model_mut()
                .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
        }
        harness.model_mut().sync_mounted_surfaces();
    }

    assert_eq!(
        harness.model().test_music_owner().browser.selected_album_targets_in_display_order(),
        vec!["album-1".to_string(), "album-2".to_string()]
    );
    assert_eq!(
        harness.model().visual_selection,
        Some((PanelFocus::Library, 2)),
        "status projection carries the tree count, not tree membership"
    );

    let right = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: album_points[0].0,
        row: album_points[0].1,
        modifiers: KeyModifiers::NONE,
    });
    harness.inject(right);
    let outcome = harness.step();
    let context_items = outcome.messages.iter().find_map(|message| match message {
        Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            _,
        )) => Some(items),
        _ => None,
    });
    assert_eq!(
        context_items
            .expect("tree selection reaches the existing bulk path")
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["album-1", "album-2"]
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    assert_eq!(
        harness.model().context_menu_origin,
        Some(crate::app::components::media_list::SelectionOrigin::Library(
            crate::app::components::media_list::LibrarySelectionOrigin::Service(
                crate::app::components::library_panel::owner::LibraryKey::Service {
                    service: mbv_core::config::ServiceKind::Emby,
                    library_id: "lib-music".into(),
                    kind: crate::app::components::library_panel::owner::LibraryKind::Music,
                },
            ),
        )),
        "bulk context captures the originating library identity"
    );
    assert_eq!(
        harness.model().visual_selection,
        Some((PanelFocus::Library, 2)),
        "folder-gated albums do not bypass capability intersection"
    );

    // The status projection has a clickable clear affordance; exercise its
    // captured-origin contract through the same shell request the mounted
    // panel emits (the status component's pointer-delivery coverage is shared
    // with the other library destinations).
    draw_music_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness
        .model()
        .application
        .get_component(&ComponentId::StatusBarPanel)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::StatusBarPanel>()
        })
        .and_then(|panel| panel.regions().visual_clear)
        .is_some());
    let origin = crate::app::components::media_list::SelectionOrigin::Library(
        crate::app::components::media_list::LibrarySelectionOrigin::Service(
            crate::app::components::library_panel::owner::LibraryKey::Service {
                service: mbv_core::config::ServiceKind::Emby,
                library_id: "lib-music".into(),
                kind: crate::app::components::library_panel::owner::LibraryKind::Music,
            },
        ),
    );
    let wrong_origin = crate::app::components::media_list::SelectionOrigin::Library(
        crate::app::components::media_list::LibrarySelectionOrigin::Home,
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::ClearMultiSelection(wrong_origin)),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(
        harness.model().test_music_owner().browser.selected_album_targets().len(),
        2,
        "a clear for another Library origin cannot clear the tree"
    );
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::ClearMultiSelection(origin)),
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(harness
        .model()
        .test_music_owner()
        .browser
        .selected_album_targets()
        .is_empty());
}

/// Inject one key through the real router, dispatch every surviving message
/// through the shell, and re-run the production sync pass.
fn tick_key(harness: &mut TickHarness, code: Key) {
    harness.inject(key(code));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
}

/// Task 2.3: at every Panel mode the Grouped Music browser's one owner and
/// painter is the destination-local tree. The Library panel's browser slot
/// drives it through the object-safe `PanelList` surface, so the panel's
/// retained browser geometry *is* the tree's own latest-render geometry and the
/// tree's own hit map claims the row it painted. The removed parallel flat album
/// carrier has no field, painter, or point-resolution path left to run here.
#[test]
fn grouped_music_browser_has_one_tree_owner_and_painter_in_every_panel_mode() {
    for (width, height) in [(160, 40), (81, 30), (60, 30)] {
        let (harness, id) = mounted_music_at(width, height);
        let owner = music_workspace(&harness, &id);

        // The panel drove exactly one skeleton this frame (Wide xor Narrow),
        // and its browser slot's selected row is the tree's own retained row.
        let panel = music_panel(&harness);
        let (wide, narrow) = (
            panel.test_wide_geometry(),
            panel.test_narrow_geometry(),
        );
        assert!(
            wide.is_some() ^ narrow.is_some(),
            "{width}x{height}: exactly one Library skeleton painted a browser slot"
        );
        let browser = wide.or(narrow).expect("a browser slot painted");
        let tree_selected = owner.browser.selected_row_rect();
        assert!(
            tree_selected.is_some(),
            "{width}x{height}: the tree retained its selected row"
        );
        assert_eq!(
            browser.selected, tree_selected,
            "{width}x{height}: the panel's browser geometry is the tree's latest-render row"
        );

        // The tree's own hit map — not a second carrier — claims the row it
        // painted, and the panel's browser list rect contains that row.
        let row = tree_selected.expect("tree selected row");
        let position = ratatui::layout::Position {
            x: row.x,
            y: row.y,
        };
        assert!(
            owner.browser.claims_point(position),
            "{width}x{height}: the tree claims the row it painted"
        );
        assert!(
            browser.list_area.contains(position),
            "{width}x{height}: the painted row sits in the browser slot"
        );

        // The projected browser flow is the tree's artist-root/album-leaf
        // model: an artist root row and at least one album leaf row. The
        // removed flat album carrier projected no focusable artist root.
        let targets = owner.album_flow_targets();
        assert!(
            targets.iter().any(Option::is_none),
            "{width}x{height}: the tree projects an artist root row"
        );
        assert!(
            targets.iter().any(Option::is_some),
            "{width}x{height}: the tree projects album leaf rows"
        );
    }
}

/// Task 2.3 / design D3: a responsive Panel-mode change reuses the *same* tree
/// owner instead of copying its selection into another control. The selected
/// album and the projected row flow survive Wide -> Mini -> Wide unchanged.
#[test]
fn grouped_music_browser_reuses_one_tree_owner_across_panel_modes() {
    let (mut harness, id) = mounted_music_at(160, 40);
    let selected = music_workspace(&harness, &id)
        .browser
        .selected_album_target()
        .map(str::to_owned);
    assert!(
        selected.is_some(),
        "the tree adopted the shell's projected album"
    );
    let wide_flow = music_workspace(&harness, &id).album_flow_targets();

    // Shrink to Mini (single panel, narrow skeleton): the same owner, no
    // re-adoption, same visible projection.
    harness.model_mut().app.terminal_width = 60;
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel(&harness).test_narrow_geometry().is_some(),
        "Mini painted the narrow skeleton"
    );
    let mini = music_workspace(&harness, &id);
    assert_eq!(
        mini.browser.selected_album_target().map(str::to_owned),
        selected,
        "the responsive change keeps the tree's selected album"
    );
    assert_eq!(
        mini.album_flow_targets(),
        wide_flow,
        "the responsive change keeps the tree's visible projection"
    );

    // Grow back to Wide: still the same owner and selection.
    harness.model_mut().app.terminal_width = 160;
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel(&harness).test_wide_geometry().is_some(),
        "Wide painted the wide skeleton"
    );
    let wide = music_workspace(&harness, &id);
    assert_eq!(
        wide.browser.selected_album_target().map(str::to_owned),
        selected,
        "the tree's selection survives the full round trip"
    );
    assert_eq!(
        wide.album_flow_targets(),
        wide_flow,
        "the tree's visible projection survives the full round trip"
    );
}

/// Tasks 6.1–6.3 correction: a Service `ArtistItems` root's Workspace rows come
/// from the shell-owned artist-detail cache, never `album_tracks_cache`. Enter
/// on a mounted artist Workspace row must cross the typed activation and play
/// the row's album group through the shell dispatch arm's artist-cache
/// resolution.
#[test]
fn enter_on_a_mounted_artist_workspace_row_plays_its_album_group() {
    let mut app = crate::app::render::make_music_group_app();
    app.terminal_width = 160;
    app.terminal_height = 40;
    app.panel_focus = PanelFocus::Library;
    // A configured-but-unroutable client: `play_album_track`'s availability
    // gate passes without a live server.
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    // The album carries a Service artist identity and a settled revision; the
    // completed artist-ID query is the only source of the Workspace rows.
    {
        let level = app.libs[0].nav_stack.last_mut().expect("album level");
        for item in &mut level.items {
            item.artist_items = vec![mbv_core::api::EmbyArtistRef {
                name: "Alpha".into(),
                id: "artist-alpha".into(),
            }];
        }
        let mut catalog = crate::app::music_grouping::build_grouped_album_catalog(
            &level.items,
            &Default::default(),
        );
        catalog.revision = 7;
        catalog.parent_id = level.parent_id.clone();
        level.music_grouping = Some(crate::app::music_grouping::MusicGroupingState {
            revision: 7,
            candidate: None,
            settled: Some(catalog),
        });
    }
    let destination = crate::app::components::library_panel::LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-music".into(),
        kind: crate::app::components::LibraryKind::Music,
    };
    let target = crate::app::components::msg::MusicArtistTarget {
        artist_id: Some("artist-alpha".into()),
        artist_name: "Alpha".into(),
        album_targets: vec!["album-1".into()],
        revision: 7,
    };
    let detail_key = app
        .artist_detail_key(&destination, &target)
        .expect("artist ID key");
    let mut track = crate::app::tests::make_item("Artist Track", "Audio");
    track.id = "artist-track-1".into();
    track.album_id = "album-1".into();
    app.artist_detail_cache.insert(
        detail_key,
        crate::app::music_artist_detail::ArtistDetailCacheEntry {
            tracks: vec![track],
            failed: false,
        },
    );
    assert!(
        app.album_tracks_cache.is_empty(),
        "the artist-ID path populates only the artist cache"
    );

    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .select_first_visible();
    assert!(
        harness.model().test_music_owner().selected_is_artist(),
        "the fixture focuses the Service artist root"
    );
    harness.model_mut().push_music_workspace_content();
    harness.model_mut().test_music_owner_mut().enter_track_focus();
    assert!(harness.model().test_music_owner().track_focused());

    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    let activate = outcome
        .messages
        .into_iter()
        .find(|message| matches!(message, Msg::Shell(ShellRequest::MusicTrackActivate { .. })))
        .expect("Enter on the artist Workspace row activates its track");
    let (mut music_resize, mut tv_resize) = (false, false);
    harness
        .model_mut()
        .handle_terminal_message(activate, &mut music_resize, &mut tv_resize);

    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["artist-track-1"],
        "the activated row's artist-cache album group becomes the queue"
    );
}

// ── Tasks 6.4/6.5: artist Workspace entry, atomic Hero switch, prefetch ──

/// A grouped Music app with five Alpha albums in settled order
/// (album-1..album-5), one cached track each: deterministic artist groups,
/// album titles, and neighbour window.
fn mounted_neighbour_app() -> crate::app::App {
    let mut app = crate::app::render::make_music_group_app();
    app.image_protocol_enabled = true;
    {
        let level = app.libs[0].nav_stack.last_mut().expect("album level");
        level.items[0].name = "Album 1".into();
        for number in 2..=5 {
            let mut album =
                crate::app::tests::make_item(&format!("Album {number}"), "MusicAlbum");
            album.id = format!("album-{number}");
            album.artist = "Alpha".into();
            album.production_year = 2001;
            level.items.push(album);
        }
        level.total_count = 5;
    }
    for number in 1..=5 {
        let mut track = crate::app::tests::make_item(&format!("Track {number}"), "Audio");
        track.id = format!("track-{number}");
        track.album_id = format!("album-{number}");
        app.album_tracks_cache
            .insert(format!("album-{number}"), vec![track]);
    }
    app
}

/// Task 6.4: Right on a collapsed artist root only expands it; a later Right
/// enters the artist Workspace — non-Wide through the Library Hero overlay,
/// whose artist Hero and grouped Workspace paint in the same push.
#[test]
fn right_on_a_collapsed_artist_root_expands_first_then_opens_its_workspace() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 81, 30);
    // The first Left leaves the adopted album leaf for Alpha's root and
    // requests its detail; the second collapses the expanded root.
    tick_key(&mut harness, Key::Left);
    assert!(
        harness.model().test_music_owner().selected_is_artist(),
        "Left moves the leaf to its artist parent"
    );
    tick_key(&mut harness, Key::Left);
    let root = harness
        .model()
        .test_music_owner()
        .browser
        .selected_id()
        .expect("artist root selected");
    assert!(
        !harness
            .model()
            .test_music_owner()
            .browser
            .root_is_expanded(root),
        "Left collapses the expanded root"
    );

    // First Right: expansion only, no overlay.
    tick_key(&mut harness, Key::Right);
    assert!(
        harness
            .model()
            .test_music_owner()
            .browser
            .root_is_expanded(root),
        "the first Right expands the root"
    );
    assert!(
        !music_panel(&harness).test_hero_overlay_open(),
        "expanding must not open the overlay"
    );

    // Later Right: the artist Workspace opens in the Library Hero overlay.
    tick_key(&mut harness, Key::Right);
    assert!(
        music_panel(&harness).test_hero_overlay_open(),
        "the later Right opens the artist Library Hero overlay"
    );
    let hero = music_panel_mut(&mut harness)
        .active_hero_data()
        .expect("the overlay paints the artist Hero");
    assert_eq!(hero.facts.title, "Alpha");
    assert_eq!(
        hero.facts.meta_rows,
        vec!["5 albums".to_string(), "2001".to_string()]
    );
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        10,
        "five album headings plus five grouped track rows"
    );
}

/// Task 6.4: in Wide geometry the later Right enters the inline artist-track
/// Workspace locally, with no overlay and no request.
#[test]
fn right_on_an_expanded_artist_root_enters_the_wide_workspace() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    assert!(harness.model().test_music_owner().selected_is_artist());
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "the tree rail still owns the focus"
    );

    tick_key(&mut harness, Key::Right);
    assert!(
        harness.model().test_music_owner().track_focused(),
        "Wide Right takes the inline artist Workspace's cursor"
    );
    assert!(!music_panel(&harness).test_hero_overlay_open());
}

/// Task 6.4: the Wide Hero and its Workspace switch atomically between the
/// album and artist arms — the title, facts, and rows never come from
/// different selections.
#[test]
fn artist_and_album_hero_workspaces_switch_atomically_in_wide() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 160, 40);

    {
        let album = music_panel_mut(&mut harness)
            .active_hero_data()
            .expect("album hero");
        assert_eq!(album.facts.title, "Album 1");
        assert_eq!(
            album.facts.meta_rows,
            vec!["Alpha".to_string(), "2001".to_string()]
        );
    }
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        1,
        "the album Workspace paints that album's track"
    );

    tick_key(&mut harness, Key::Left);
    {
        let artist = music_panel_mut(&mut harness)
            .active_hero_data()
            .expect("artist hero");
        assert_eq!(artist.facts.title, "Alpha");
        assert_eq!(
            artist.facts.meta_rows,
            vec!["5 albums".to_string(), "2001".to_string()]
        );
    }
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        10,
        "the artist Workspace replaces the album rows in the same push"
    );
    assert!(matches!(
        harness.model().test_music_owner().track_list.rows().first(),
        Some(crate::app::components::media_list::MediaListRow::Heading { text }) if text == "Album 1"
    ));

    tick_key(&mut harness, Key::Down);
    {
        let album = music_panel_mut(&mut harness)
            .active_hero_data()
            .expect("album hero returns");
        assert_eq!(album.facts.title, "Album 1");
    }
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        1
    );
}

/// A two-root Grouped Music app (Alpha over `album-1`, Beta over two albums)
/// with no cached track rows anywhere: every artist Workspace entry arms with
/// its rows still in flight, so the arrival path is observable hermetically.
fn two_artist_app() -> crate::app::App {
    let mut app = crate::app::render::make_music_group_app();
    let level = app.libs[0].nav_stack.last_mut().expect("album level");
    for (name, id) in [("Beta Session", "album-beta-1"), ("Beta Nights", "album-beta-2")] {
        let mut album = crate::app::tests::make_item(name, "MusicAlbum");
        album.id = id.into();
        album.artist = "Beta".into();
        level.items.push(album);
    }
    level.total_count = level.items.len();
    app
}

/// Deposits one cached track for `album_id` (the fallback artist Workspace's
/// aggregation source) and re-projects the workspace: the rows "arrive".
fn arrive_album_tracks(harness: &mut TickHarness, album_id: &str, track_id: &str) {
    let mut track = crate::app::tests::make_item(track_id, "Audio");
    track.album_id = album_id.into();
    harness
        .model_mut()
        .app
        .album_tracks_cache
        .insert(album_id.to_string(), vec![track]);
    harness.model_mut().push_music_workspace_content();
    harness.model_mut().sync_mounted_surfaces();
}

/// Task 6.4: a Wide artist-Workspace entry armed before the rows arrived
/// takes the cursor when its own root's rows land — and only then.
#[test]
fn armed_artist_workspace_entry_takes_the_cursor_when_its_own_rows_arrive() {
    let (mut harness, _id) = mounted_music_app_at(two_artist_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    assert!(harness.model().test_music_owner().selected_is_artist());

    // Right on the expanded root with no rows: the entry arms, the cursor
    // stays with the tree.
    tick_key(&mut harness, Key::Right);
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "an armed entry does not take the cursor before its rows arrive"
    );
    assert!(
        harness
            .model()
            .test_music_owner()
            .pending_artist_workspace_focus_for_test(),
        "the entry is armed for the focused root"
    );

    // The armed root's own rows land: the entry takes the cursor once.
    arrive_album_tracks(&mut harness, "album-1", "alpha-track-1");
    assert!(
        harness.model().test_music_owner().track_focused(),
        "the armed entry takes the cursor when its own root's rows arrive"
    );
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        2,
        "the focused Workspace holds the armed root's own heading and track"
    );
}

/// Task 6.4: the armed entry carries the root it was armed on. A push for
/// another root never takes the cursor, returning to the armed root does not
/// resurrect the voided entry, and a fresh Right enters the Workspace again.
#[test]
fn armed_artist_workspace_entry_does_not_seize_focus_for_another_root() {
    let (mut harness, _id) = mounted_music_app_at(two_artist_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    tick_key(&mut harness, Key::Right);
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "the entry is armed on the Alpha root with no rows"
    );

    // The selection leaves Alpha for the Beta root; the entry voids with it,
    // and Beta's rows then land.
    tick_key(&mut harness, Key::End);
    tick_key(&mut harness, Key::Left);
    assert!(
        !harness
            .model()
            .test_music_owner()
            .pending_artist_workspace_focus_for_test(),
        "leaving the armed root voids the entry"
    );
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .artist_detail_target()
            .map(|target| target.artist_name),
        Some("Beta".to_string()),
        "the selection moved to the Beta root"
    );
    arrive_album_tracks(&mut harness, "album-beta-1", "beta-track-1");
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "another root's arriving rows never take the cursor"
    );

    // Back on Alpha, its rows arrive too: the voided entry stays dead.
    tick_key(&mut harness, Key::Home);
    assert!(harness.model().test_music_owner().selected_is_artist());
    arrive_album_tracks(&mut harness, "album-1", "alpha-track-1");
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "returning to the armed root does not resurrect the voided entry"
    );

    // A fresh Right on the same root enters its now-resident Workspace.
    tick_key(&mut harness, Key::Right);
    assert!(
        harness.model().test_music_owner().track_focused(),
        "a fresh Right enters the artist Workspace"
    );
}

/// Task 6.4: the armed entry belongs to the Wide inline pane. A Narrow
/// transition voids it: arriving rows never take the cursor on the narrow
/// pane nothing paints, and the entry stays dead when the Wide pane returns
/// — a fresh Right is required to enter the Workspace again.
#[test]
fn armed_artist_workspace_entry_is_voided_by_a_narrow_transition() {
    let (mut harness, _id) = mounted_music_app_at(two_artist_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    tick_key(&mut harness, Key::Right);
    assert!(!harness.model().test_music_owner().track_focused());

    // Wide -> Narrow: the same owner survives, the inline pane does not, and
    // the armed entry is cleared with it.
    harness.model_mut().app.terminal_width = 60;
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel(&harness).test_narrow_geometry().is_some(),
        "the transition painted the narrow skeleton"
    );
    assert!(
        !harness
            .model()
            .test_music_owner()
            .pending_artist_workspace_focus_for_test(),
        "the Narrow transition clears the armed Wide entry"
    );

    // The armed root's rows land while Narrow, then a frame draws: neither
    // the push nor the draw may take the cursor on the pane nothing paints.
    arrive_album_tracks(&mut harness, "album-1", "alpha-track-1");
    draw_music_frame(&mut harness);
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "a voided entry never re-seizes the focus on the narrow pane"
    );

    // Back to Wide: the entry was voided by the transition, so even its own
    // now-resident rows do not take the cursor without a fresh Right.
    harness.model_mut().app.terminal_width = 160;
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel(&harness).test_wide_geometry().is_some(),
        "the return painted the wide skeleton"
    );
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "the entry voided by the Narrow transition stays dead in Wide"
    );
    tick_key(&mut harness, Key::Right);
    assert!(
        harness.model().test_music_owner().track_focused(),
        "a fresh Right still enters the now-resident artist Workspace"
    );
}

/// Task 6.5 (design D4): the tree resolves the neighbour window from its
/// completed paint and emits the typed payload in visible order in both
/// presentations; the shell re-resolves no cursor.
#[test]
fn neighbour_prefetch_payload_is_the_painted_trees_order_in_both_presentations() {
    for (width, height) in [(160u16, 40u16), (81, 30)] {
        let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), width, height);
        // The production loop drains the panel's post-paint message before the
        // next frame; clear the adopt-frame request so this test observes the
        // payload for the selection it makes.
        let (mut music_resize, mut tv_resize) = (false, false);
        harness
            .model_mut()
            .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
        assert!(
            harness
                .model_mut()
                .test_music_owner_mut()
                .browser
                .select_album_target("album-3"),
            "{width}x{height}: the fixture interns album-3"
        );
        harness.model_mut().sync_mounted_surfaces();
        assert_eq!(
            harness
                .model()
                .test_music_owner()
                .browser
                .selected_album_target(),
            Some("album-3"),
            "{width}x{height}: the selection survives the sync"
        );
        draw_music_frame(&mut harness);

        match music_panel_mut(&mut harness).take_deferred_msg() {
            Some(Msg::Shell(ShellRequest::MusicNeighbourPrefetch { targets })) => {
                assert_eq!(
                    targets,
                    vec![
                        "album-2".to_string(),
                        "album-4".to_string(),
                        "album-5".to_string(),
                    ],
                    "{width}x{height}: one behind and three ahead over the visible leaves"
                );
            }
            other => panic!("{width}x{height}: expected the neighbour request, got {other:?}"),
        }
    }
}

/// Task 6.5: the shell applies the existing idle gate to the typed targets —
/// a closed gate suppresses every fetch — and an artist-root focus ships no
/// request at all.
#[test]
fn neighbour_prefetch_is_idle_gated_and_suppressed_on_an_artist_root() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 160, 40);
    let (mut music_resize, mut tv_resize) = (false, false);
    // Clear the adopt-frame request (the production loop drains between
    // frames), then select the album whose window this test asserts.
    harness
        .model_mut()
        .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
    assert!(
        harness
            .model_mut()
            .test_music_owner_mut()
            .browser
            .select_album_target("album-3")
    );
    harness.model_mut().sync_mounted_surfaces();
    // Drop the selected hero's own non-idle fetch so the assertions isolate
    // the neighbour window's reservations.
    harness.model_mut().app.card_image_loading.clear();
    draw_music_frame(&mut harness);
    harness
        .model_mut()
        .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
    for key in ["album-2:P", "album-4:P", "album-5:P"] {
        assert!(
            harness.model().app.card_image_loading.contains(key),
            "{key} prefetches while the idle gate is open"
        );
    }
    assert!(
        !harness.model().app.card_image_loading.contains("album-1:P"),
        "a leaf outside the window is not prefetched"
    );

    // A fresh navigation closes the idle gate: the tree still resolves and
    // emits the window, but the shell makes no reservation for it.
    harness.model_mut().app.card_image_loading.clear();
    harness.model_mut().app.last_nav_at = std::time::Instant::now();
    draw_music_frame(&mut harness);
    let before = harness.model().app.card_image_fetch_calls;
    let targets = match music_panel_mut(&mut harness).take_deferred_msg() {
        Some(Msg::Shell(ShellRequest::MusicNeighbourPrefetch { targets })) => targets,
        other => panic!("expected the neighbour request, got {other:?}"),
    };
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::MusicNeighbourPrefetch { targets }),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(
        harness.model().app.card_image_fetch_calls,
        before,
        "the shell's idle gate makes no fetch reservation"
    );
    assert!(harness.model().app.card_image_loading.is_empty());

    // An artist-root focus emits no neighbour request at all.
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .select_first_visible();
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel_mut(&mut harness).take_deferred_msg().is_none(),
        "an artist-root focus ships no neighbour request"
    );
}
