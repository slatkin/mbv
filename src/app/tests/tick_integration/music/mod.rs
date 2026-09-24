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
        Some(&crate::app::state::app_struct::LevelFillState::Failed)
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
        outcome.messages.iter().any(|m| matches!(
            m,
            Msg::Shell(ShellRequest::RowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Queue(_),
                _
            ))
        )),
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

mod artist;
mod landing;

/// Row 3.3: a playlist activation on a populated, dirty saved-playlist queue
/// asks the replacement question through the production shell path; confirming
/// it reaches the existing save/discard prompt instead of replacing the queue.
#[test]
fn playlist_activation_on_a_populated_dirty_queue_asks_then_reaches_the_save_prompt() {
    use crate::app::tests::make_item;
    let mut app = make_app_stub();
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("saved-1".into()),
        name: "Saved".into(),
    };
    app.queue_dirty = true;
    let playlist = make_item("P1", "Playlist");
    let mut song = make_item("Song", "Audio");
    song.id = "item-1".into();
    app.playlists = vec![playlist.clone()];
    app.playlists_open = Some(playlist);
    app.playlists_open_items = vec![song];
    app.playlists_open_cursor = 0;
    let mut harness = TickHarness::new(app);

    harness
        .model_mut()
        .handle_playlists_request(ShellRequest::PlaylistsActivate {
            open: true,
            index: 0,
        });
    assert!(matches!(
        &harness.model().app.pending_overlay,
        Some(OverlayRequest::Confirm(modal))
            if modal.on_confirm == ConfirmAction::ReplacePopulatedQueue
    ));
    assert!(harness.model().app.pending_queue_replacement.is_some());
    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["existing"],
        "the load has not touched the queue before confirmation"
    );
    // Mount the gate modal so the shell's confirm-intent path reads its action.
    harness.model_mut().sync_mounted_surfaces();
    let confirm_id = ComponentId::Modal(ModalId::Confirm);
    assert!(
        harness.model().application.mounted(&confirm_id),
        "the replacement gate mounts its confirm modal"
    );

    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::ConfirmIntent(ConfirmIntent::Accept)),
        &mut music_resize,
        &mut tv_resize,
    );

    assert!(matches!(
        &harness.model().app.pending_overlay,
        Some(OverlayRequest::Confirm(modal))
            if modal.on_confirm == ConfirmAction::DiscardOrSaveDirtyPlaylist
    ));
    assert!(harness.model().app.pending_queue_replacement.is_none());
    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["existing"],
        "the dirty-playlist save/discard second step still defers the load"
    );
}

/// Mounts the Playlists sidebar through the production overlay request and
/// returns its component id. Used by the sidebar-dismiss quadrant tests below,
/// which care about `run_replacement`'s dismiss/focus tail rather than the
/// key-routing path the Enter-family tests already cover.
fn mount_playlists_sidebar(harness: &mut TickHarness) -> ComponentId {
    harness.model_mut().app.pending_overlay = Some(OverlayRequest::OpenSidebar(
        crate::app::SidebarId::Playlists,
    ));
    harness.model_mut().sync_mounted_surfaces();
    let id = ComponentId::Overlay(OverlayId::Playlists);
    assert!(
        harness.model().application.mounted(&id),
        "the Playlists sidebar is mounted before the load"
    );
    id
}

/// Answers the mounted Confirm modal through the shell's confirm-intent
/// boundary, then runs the production sync pass so the resulting
/// dismiss/mount request lands on the TuiRealm tree.
fn answer_confirm(harness: &mut TickHarness, intent: ConfirmIntent) {
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::ConfirmIntent(intent)),
        &mut music_resize,
        &mut tv_resize,
    );
    harness.model_mut().sync_mounted_surfaces();
}

/// Builds the open-playlist fixture the PlaylistsActivate path reads: one
/// non-folder track selected.
fn open_playlist_fixture(app: &mut crate::app::App) {
    use crate::app::tests::make_item;
    let playlist = make_item("P1", "Playlist");
    let mut song = make_item("Song", "Audio");
    song.id = "item-1".into();
    app.playlists = vec![playlist.clone()];
    app.playlists_open = Some(playlist);
    app.playlists_open_items = vec![song];
    app.playlists_open_cursor = 0;
}

/// Sidebar-dismiss quadrant 1: an empty queue needs no gate, so the playlist
/// load runs immediately and `run_replacement` dismisses the Playlists sidebar
/// and moves panel focus to Queue.
#[test]
fn playlist_load_on_empty_queue_dismisses_the_sidebar() {
    let mut app = make_app_stub();
    open_playlist_fixture(&mut app);
    let mut harness = TickHarness::new(app);
    let playlists_id = mount_playlists_sidebar(&mut harness);

    harness
        .model_mut()
        .handle_playlists_request(ShellRequest::PlaylistsActivate {
            open: true,
            index: 0,
        });
    assert!(
        harness.model().app.pending_queue_replacement.is_none(),
        "an empty queue needs no replacement gate"
    );
    assert!(!harness
        .model()
        .application
        .mounted(&ComponentId::Modal(ModalId::Confirm)));

    harness.model_mut().sync_mounted_surfaces();

    assert!(
        !harness.model().application.mounted(&playlists_id),
        "the ungated playlist load dismisses the Playlists sidebar"
    );
    assert_eq!(
        harness.model().app.effective_panel_focus(),
        PanelFocus::Queue,
        "panel focus moves to Queue when the load runs"
    );
    assert_eq!(
        harness.model().app.playback_queue().total_queue_len(),
        1,
        "the empty queue is replaced immediately"
    );
}

/// Sidebar-dismiss quadrant 2: a populated queue is gated; confirming runs the
/// replacement, so the dismiss/focus tail fires exactly as it does ungated.
#[test]
fn playlist_load_on_populated_queue_dismisses_the_sidebar_on_confirm() {
    use crate::app::tests::make_item;
    let mut app = make_app_stub();
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    open_playlist_fixture(&mut app);
    let mut harness = TickHarness::new(app);
    let playlists_id = mount_playlists_sidebar(&mut harness);

    harness
        .model_mut()
        .handle_playlists_request(ShellRequest::PlaylistsActivate {
            open: true,
            index: 0,
        });
    assert!(matches!(
        &harness.model().app.pending_overlay,
        Some(OverlayRequest::Confirm(modal))
            if modal.on_confirm == ConfirmAction::ReplacePopulatedQueue
    ));
    harness.model_mut().sync_mounted_surfaces();

    answer_confirm(&mut harness, ConfirmIntent::Accept);

    assert!(
        !harness.model().application.mounted(&playlists_id),
        "a confirmed gated load dismisses the Playlists sidebar"
    );
    assert_eq!(
        harness.model().app.effective_panel_focus(),
        PanelFocus::Queue,
        "panel focus moves to Queue on confirm"
    );
    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["item-1"],
        "the confirmed load replaces the populated queue"
    );
}

/// Sidebar-dismiss quadrant 3: cancelling the gate leaves both the queue and
/// the Playlists sidebar untouched — the dismiss tail never runs.
#[test]
fn playlist_load_on_populated_queue_keeps_the_sidebar_on_cancel() {
    use crate::app::tests::make_item;
    let mut app = make_app_stub();
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    open_playlist_fixture(&mut app);
    let mut harness = TickHarness::new(app);
    let playlists_id = mount_playlists_sidebar(&mut harness);

    harness
        .model_mut()
        .handle_playlists_request(ShellRequest::PlaylistsActivate {
            open: true,
            index: 0,
        });
    harness.model_mut().sync_mounted_surfaces();

    answer_confirm(&mut harness, ConfirmIntent::Cancel);

    assert!(
        harness.model().application.mounted(&playlists_id),
        "a cancelled load leaves the Playlists sidebar open"
    );
    assert!(harness.model().app.pending_queue_replacement.is_none());
    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["existing"],
        "a cancelled load leaves the queue unchanged"
    );
}

/// Sidebar-dismiss quadrant 4: confirming a gated load of a populated dirty
/// saved-playlist queue raises the second-step save/discard prompt instead of
/// executing, so the sidebar-dismiss tail is suppressed and the sidebar stays.
#[test]
fn playlist_load_on_dirty_saved_playlist_keeps_the_sidebar_at_the_save_prompt() {
    use crate::app::tests::make_item;
    let mut app = make_app_stub();
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("saved-1".into()),
        name: "Saved".into(),
    };
    app.queue_dirty = true;
    open_playlist_fixture(&mut app);
    let mut harness = TickHarness::new(app);
    let playlists_id = mount_playlists_sidebar(&mut harness);

    harness
        .model_mut()
        .handle_playlists_request(ShellRequest::PlaylistsActivate {
            open: true,
            index: 0,
        });
    harness.model_mut().sync_mounted_surfaces();

    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::ConfirmIntent(ConfirmIntent::Accept)),
        &mut music_resize,
        &mut tv_resize,
    );

    assert!(
        matches!(
            &harness.model().app.pending_overlay,
            Some(OverlayRequest::Confirm(modal))
                if modal.on_confirm == ConfirmAction::DiscardOrSaveDirtyPlaylist
        ),
        "the dirty saved-playlist second step raises the save/discard prompt"
    );
    // Run the sync pass too: mounting the save/discard modal must not dismiss
    // the sidebar before the user answers it.
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness.model().application.mounted(&playlists_id),
        "a raised save/discard prompt keeps the Playlists sidebar"
    );
    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["existing"],
        "the save/discard second step still defers the load"
    );
}
