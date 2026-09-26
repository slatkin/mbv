use super::*;

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
           Msg::Shell(ref shell_boxed)
        if matches!(shell_boxed.as_ref(), ShellRequest::PlaylistsActivate {
               open: true,
               index: 0
           }))),
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
        .handle_playlists_request(&ShellRequest::PlaylistsActivate {
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
        Msg::Shell(Box::new(ShellRequest::ConfirmIntent(ConfirmIntent::Accept))),
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
        Msg::Shell(Box::new(ShellRequest::ConfirmIntent(intent))),
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
        .handle_playlists_request(&ShellRequest::PlaylistsActivate {
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
        .handle_playlists_request(&ShellRequest::PlaylistsActivate {
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
        .handle_playlists_request(&ShellRequest::PlaylistsActivate {
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
        .handle_playlists_request(&ShellRequest::PlaylistsActivate {
            open: true,
            index: 0,
        });
    harness.model_mut().sync_mounted_surfaces();

    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::ConfirmIntent(ConfirmIntent::Accept))),
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
