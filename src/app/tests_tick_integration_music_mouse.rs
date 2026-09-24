use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::list::tree_browser::TreeOperation;
use crate::app::components::music_tree_target::MusicTreeTarget;
use crate::app::components::{ComponentId, ModalId, Msg, ShellRequest, UserEvent};
use crate::app::render::{make_music_group_app, make_music_group_app_with_second_album};
use crate::app::tests::make_item;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{PanelFocus, PanelMode};

/// The framed Wide track table is a second canonical control in Grouped Music:
/// its retained claim resolves click, double-click, right-click, and one-step
/// wheel input through a live Application tick without a parent row map.
#[test]
fn music_wide_track_table_uses_retained_geometry_for_live_mouse_gestures() {
    let mut app = make_music_group_app();
    let tracks = (0..3)
        .map(|index| {
            let mut track = make_item(&format!("Track {}", index + 1), "Audio");
            track.id = format!("track-{}", index + 1);
            track.index_number = index + 1;
            track
        })
        .collect::<Vec<_>>();
    app.album_tracks_cache.insert("album-1".into(), tracks);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let music_id = ComponentId::Library;

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().mouse_subscribed.contains(&music_id));
    let track_state = |harness: &TickHarness| {
        let music = harness.model().test_music_owner();
        (music.track_focused(), music.track_selected_row())
    };
    let (_track_point, second_track_point) = {
        let music = harness.model().test_music_owner();
        let content = music.track_list
            .current_content_rect()
            .expect("Wide track table retained its current content rect");
        let selected = music
            .track_list.current_selected_row_rect()
            .expect("Wide track table retained its selected row");
        (
            (selected.x, selected.y),
            (content.x, content.y.saturating_add(1)),
        )
    };
    let click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };
    let right_click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };

    // The first click focuses the second track from the child result. The
    // second click at the unchanged painted point becomes a double-click and
    // activates that same stable provider target.
    harness.inject(click(second_track_point.0, second_track_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| {
        !matches!(message, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
    }));
    assert_eq!(track_state(&harness), (true, Some(1)));
    harness.inject(click(second_track_point.0, second_track_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicTrackActivate { .. })
    )));

    // Right-click resolves the same retained row and translates directly to
    // the track context intent; no shell-side coordinate lookup is involved.
    harness.inject(right_click(second_track_point.0, second_track_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicRowContextMenu(_, Some((x, y)))) if *x == second_track_point.0 && *y == second_track_point.1
    )));

    // Wheel over the painted table advances exactly one local track row and
    // emits no shell-side cursor movement.
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: second_track_point.0,
        row: second_track_point.1,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert_eq!(track_state(&harness), (true, Some(2)));
    assert!(outcome
        .messages
        .iter()
        .all(|message| !matches!(message, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))));
}

/// Modifier clicks on the Wide Music track list are delivered through the
/// LibraryPanel surface gesture and retain the shared owner's selection.
#[test]
fn music_wide_track_modifier_clicks_toggle_range_and_plain_clear() {
    let mut app = make_music_group_app();
    let tracks = (0..3)
        .map(|index| {
            let mut track = make_item(&format!("Track {}", index + 1), "Audio");
            track.id = format!("track-{}", index + 1);
            track.index_number = index + 1;
            track
        })
        .collect::<Vec<_>>();
    app.album_tracks_cache.insert("album-1".into(), tracks);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    let (x, y) = {
        let music = harness.model().test_music_owner();
        let content = music
            .track_list
            .current_content_rect()
            .expect("track table retained its content rect");
        (content.x, content.y)
    };
    let mouse = |column, row, modifiers| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers,
        })
    };

    harness.inject(mouse(x, y + 1, tuirealm::event::KeyModifiers::CONTROL));
    harness.step();
    assert_eq!(
        harness.model().test_music_owner().track_list.multi_selection(),
        &["track-1".to_string(), "track-2".to_string()]
    );

    harness.inject(mouse(x, y + 2, tuirealm::event::KeyModifiers::SHIFT));
    harness.step();
    assert_eq!(
        harness.model().test_music_owner().track_list.multi_selection(),
        &[
            "track-1".to_string(),
            "track-2".to_string(),
            "track-3".to_string()
        ]
    );

    harness.inject(mouse(
        x,
        y,
        tuirealm::event::KeyModifiers::NONE,
    ));
    harness.step();
    assert!(harness
        .model()
        .test_music_owner()
        .track_list
        .multi_selection()
        .is_empty());
}

/// A mounted Grouped Music tree click focuses Library, while a click on an
/// already-focused Queue leaves Queue focused. Context clicks also cross the
/// Music-specific focus-before-menu shell boundary; generic Queue context
/// requests remain generic.
#[test]
fn music_tree_click_and_context_menu_focus_library_but_queue_stays_generic() {
    let mut app = make_music_group_app();
    app.panel_mode = PanelMode::Both;
    app.panel_focus = PanelFocus::Queue;
    app.player_tab.set_items(vec![make_item("Queue Item", "Audio")], 0);
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    let tree_point = {
        let music = harness.model().test_music_owner();
        let root = music
            .browser
            .visible_targets()
            .first()
            .expect("tree root")
            .clone();
        let row = music
            .browser
            .row_rect_for(&root)
            .expect("painted tree row");
        (row.x, row.y)
    };
    let click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };
    let right_click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };

    let list_area = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(|panel| panel.test_list_rect())
        .expect("painted Music list area");
    assert!(list_area.contains(ratatui::layout::Position::new(tree_point.0, tree_point.1)));
    assert!(harness.model().mouse_subscribed.contains(&ComponentId::Library));

    // Queue's ordinary context request remains the generic shell variant and
    // does not acquire Library focus merely because Music has a special arm.
    let queue_point = harness
        .model()
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::QueueComponent>()
        })
        .and_then(|queue| queue.selected_row_rect())
        .expect("painted queue row");
    harness.inject(right_click(queue_point.x, queue_point.y));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::state::types::context_menu::ContextMenuTargets::Queue(_),
            Some((x, y)),
        )) if *x == queue_point.x && *y == queue_point.y
    )));
    assert_eq!(harness.model().app.effective_panel_focus(), PanelFocus::Queue);

    harness.inject(click(tree_point.0, tree_point.1));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::LibraryPanelFocus))
            || matches!(message, Msg::Shell(ShellRequest::MusicArtistTracks { .. }))
    }), "tree click messages: {:?}", outcome.raw_messages);
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    assert_eq!(harness.model().app.effective_panel_focus(), PanelFocus::Library);

    // The click's mutation invalidated the completed frame; re-paint so the
    // right-click resolves the latest geometry.
    draw_frame(&mut harness);
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.inject(right_click(tree_point.0, tree_point.1));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicRowContextMenu(_, Some((x, y))))
            if *x == tree_point.0 && *y == tree_point.1
    )));
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    assert_eq!(harness.model().app.effective_panel_focus(), PanelFocus::Library);
    harness.model_mut().sync_mounted_surfaces();
    let menu_id = ComponentId::Overlay(crate::app::components::OverlayId::ContextMenu);
    assert!(harness.model().application.mounted(&menu_id));
}

/// Grouped Music resolves its artist-root and album-leaf clicks through the
/// tree's completed hit map, then rejects that map as soon as a new frame is
/// configured. The mounted path is the real LibraryPanel/Application tick;
/// there is no shell-side album row geometry to fall back to.
#[test]
fn music_tree_mouse_resolves_current_rows_and_rejects_an_invalidated_frame() {
    let mut app = make_music_group_app();
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(second_album);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    let list_area = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(|panel| panel.test_list_rect())
        .expect("Music tree list geometry");
    let (root_point, album_point) = {
        let music = harness.model().test_music_owner();
        let point_for = |target: Option<&str>| {
            let node = music
                .browser
                .visible_targets()
                .into_iter()
                .find(|candidate| candidate.album_leaf_target() == target)
                .expect("painted tree node");
            let row = music
                .browser
                .row_rect_for(&node)
                .expect("painted tree row");
            (row.x, row.y)
        };
        (point_for(None), point_for(Some("album-1")))
    };
    assert!(list_area.contains(ratatui::layout::Position::new(
        root_point.0,
        root_point.1
    )));
    assert!(list_area.contains(ratatui::layout::Position::new(
        album_point.0,
        album_point.1
    )));

    let click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };

    // The artist root is a painted, focusable tree row but not an album
    // target, so its click changes only the component-local selection.
    harness.inject(click(root_point.0, root_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| {
        !matches!(message, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
    }));
    assert!(harness.model().test_music_owner().selected_is_artist());

    // The root click's mutation invalidated the completed frame; re-paint so
    // the leaf click resolves the latest geometry.
    draw_frame(&mut harness);
    harness.inject(click(album_point.0, album_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicAlbumCursor { target: 0, .. })
    )));
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .browser
            .selected_target()
            .and_then(|target| target.album_leaf_target()),
        Some("album-1")
    );

    // A content/geometry transition invalidates the completed tree result
    // before the replacement frame paints. The mounted parent still receives
    // the event through Application::tick, but MusicContent claims no stale
    // row and emits no cursor request.
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .invalidate_paint();
    harness.inject(click(album_point.0, album_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| {
        !matches!(message, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
    }));
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .browser
            .selected_target()
            .and_then(|target| target.album_leaf_target()),
        Some("album-1"),
        "the stale click did not mutate the tree selection"
    );
}

// ── Row 5.2/5.3: filtered tree pointer routing and grouped-track playback ──

/// Dispatch every surviving message of one step through the shell and re-run
/// the production sync pass.
fn dispatch_step(harness: &mut TickHarness) {
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
}

fn draw_frame(harness: &mut TickHarness) {
    harness.model_mut().sync_mounted_surfaces();
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .expect("music frame");
}

/// Inject one key through the real router, dispatch every surviving message
/// through the shell, and re-run the production sync pass.
fn inject_key(harness: &mut TickHarness, code: Key) {
    harness.inject(Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }));
    dispatch_step(harness);
}

fn left_click(column: u16, row: u16) -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

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

/// The settled album leaf's stable target, resolved from the tree projection
/// without needing a completed frame.
fn album_target(harness: &TickHarness, target: &str) -> MusicTreeTarget {
    let music = harness.model().test_music_owner();
    music
        .browser
        .visible_targets()
        .into_iter()
        .find(|candidate| candidate.album_leaf_target() == Some(target))
        .expect("projected album node")
}

// ── Row 5.5: double-click and track-activation tick coverage ──

/// The two expandable tree rows a double-click case can claim.
#[derive(Clone, Copy, Debug)]
enum TreeDoubleClickNode {
    ArtistRoot,
    AlbumLeaf,
}

impl TreeDoubleClickNode {
    /// The case's settled node, resolved from the projection by identity.
    fn resolve(self, harness: &TickHarness) -> MusicTreeTarget {
        let music = harness.model().test_music_owner();
        music
            .browser
            .visible_targets()
            .into_iter()
            .find(|candidate| match self {
                TreeDoubleClickNode::ArtistRoot => candidate.is_artist(),
                TreeDoubleClickNode::AlbumLeaf => {
                    candidate.album_leaf_target() == Some("album-1")
                }
            })
            .expect("the case's projected tree node")
    }

    /// The filter query that keeps this node's own row visible.
    fn filter_query(self) -> &'static str {
        match self {
            TreeDoubleClickNode::ArtistRoot => "Alpha",
            TreeDoubleClickNode::AlbumLeaf => "First Album",
        }
    }
}

/// One cached `album-1` track for the tree-track activation cases.
fn cached_track(id: &str, number: i64) -> mbv_core::api::EmbyItem {
    let mut track = make_item(&format!("Track {number}"), "Audio");
    track.id = id.into();
    track.album_id = "album-1".into();
    track.media_type = "Audio".into();
    track.index_number = number;
    track
}

/// The painted point of any tree row, resolved from the tree's completed
/// frame through the shared read-only stable-target row geometry.
fn tree_node_point(harness: &TickHarness, target: &MusicTreeTarget) -> (u16, u16) {
    let music = harness.model().test_music_owner();
    let row = music
        .browser
        .row_rect_for(target)
        .expect("painted tree row");
    (row.x, row.y)
}

/// Whether the shared confirm modal is mounted in the current composition.
fn confirm_mounted(harness: &TickHarness) -> bool {
    harness
        .model()
        .application
        .mounted(&ComponentId::Modal(ModalId::Confirm))
}

/// A Wide `make_music_group_app` whose `album-1` leaf owns two cached tracks,
/// ready for a tree-track activation case.
fn music_tree_track_app(autoload: bool) -> crate::app::App {
    let mut app = make_music_group_app();
    app.config.lock().unwrap().autoload = autoload;
    app.terminal_width = 160;
    app.terminal_height = 40;
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    app.album_tracks_cache.insert(
        "album-1".into(),
        vec![cached_track("track-1", 1), cached_track("track-2", 2)],
    );
    app
}

/// The same app with the album leaf expanded and its track rows painted.
fn expanded_track_harness(app: crate::app::App) -> TickHarness {
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let album_id = album_target(&harness, "album-1");
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(album_id.clone()));
    draw_frame(&mut harness);
    harness
}

fn expanded_music_tree_track_harness(autoload: bool) -> TickHarness {
    expanded_track_harness(music_tree_track_app(autoload))
}

/// The playback-target queue's item ids in queue order.
fn playback_queue_ids(harness: &TickHarness) -> Vec<String> {
    harness
        .model()
        .app
        .playback_queue()
        .emby_items()
        .iter()
        .map(|item| item.id.clone())
        .collect()
}

/// The two tree-track activation routes that share the grouped resolver.
#[derive(Clone, Copy, Debug)]
enum TrackActivation {
    Enter,
    DoubleClick,
}

/// Select the painted track row, then activate it by the case's route.
fn activate_track(harness: &mut TickHarness, at: (u16, u16), kind: TrackActivation) {
    harness.inject(left_click(at.0, at.1));
    dispatch_step(harness);
    draw_frame(harness);
    match kind {
        TrackActivation::Enter => inject_key(harness, Key::Enter),
        TrackActivation::DoubleClick => {
            harness.inject(left_click(at.0, at.1));
            dispatch_step(harness);
        }
    }
}

/// D5/rows 5.2 and 5.5: a double-click on an expandable tree node — an artist
/// root or an album leaf with cached track children — toggles its persistent
/// expansion and opens no Hero, in Wide and non-Wide and both with and without
/// the local tree filter active. With the filter active the production Grouped
/// Music surface paints the tree and leaves the flat Inline Search result
/// carrier empty, so the gesture must resolve current-frame filtered tree
/// geometry and keep tree semantics.
#[rstest]
#[case::wide_unfiltered_artist(160, 40, false, TreeDoubleClickNode::ArtistRoot)]
#[case::wide_filtered_artist(160, 40, true, TreeDoubleClickNode::ArtistRoot)]
#[case::wide_unfiltered_album(160, 40, false, TreeDoubleClickNode::AlbumLeaf)]
#[case::wide_filtered_album(160, 40, true, TreeDoubleClickNode::AlbumLeaf)]
#[case::narrow_unfiltered_artist(81, 30, false, TreeDoubleClickNode::ArtistRoot)]
#[case::narrow_filtered_artist(81, 30, true, TreeDoubleClickNode::ArtistRoot)]
#[case::narrow_unfiltered_album(81, 30, false, TreeDoubleClickNode::AlbumLeaf)]
#[case::narrow_filtered_album(81, 30, true, TreeDoubleClickNode::AlbumLeaf)]
fn double_click_expands_artist_and_album_nodes_without_a_hero(
    #[case] width: u16,
    #[case] height: u16,
    #[case] filtered: bool,
    #[case] node: TreeDoubleClickNode,
) {
    let mut app = make_music_group_app();
    app.album_tracks_cache
        .insert("album-1".into(), vec![cached_track("track-1", 1)]);
    app.terminal_width = width;
    app.terminal_height = height;
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw_frame(&mut harness);

    let node_id = node.resolve(&harness);
    let was_expanded = harness
        .model()
        .test_music_owner()
        .browser
        .is_expanded(&node_id);

    // Open the production filter through the router, then feed the
    // destination's own query (the panel paints the tree, never a flat result
    // list). Filter-forced visibility never overwrites persistent expansion.
    if filtered {
        inject_key(&mut harness, Key::Char('/'));
        harness
            .model_mut()
            .test_music_owner_mut()
            .browser
            .apply(TreeOperation::EditFilter(node.filter_query().to_string()));
        draw_frame(&mut harness);
        assert!(
            harness.model().test_music_owner().inline_search.results_len() == 0,
            "{width}x{height}: production filtering leaves the flat carrier empty"
        );
    }

    // The first click resolves the row; the run loop paints the next frame
    // before the second press, as the real event loop does.
    let at = tree_node_point(&harness, &node_id);
    harness.inject(left_click(at.0, at.1));
    dispatch_step(&mut harness);
    draw_frame(&mut harness);
    harness.inject(left_click(at.0, at.1));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().all(|message| !matches!(
            message,
            Msg::Shell(ShellRequest::MusicAlbumActivate { .. })
                | Msg::Shell(ShellRequest::InlineSearchActivate { .. })
        )),
        "{width}x{height} {node:?} filtered={filtered}: double-click keeps tree semantics: {:?}",
        outcome.raw_messages
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert_ne!(
        harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&node_id),
        was_expanded,
        "{width}x{height} {node:?} filtered={filtered}: double-click toggles persistent expansion"
    );
    if filtered {
        assert!(
            harness
                .model()
                .test_music_owner()
                .browser
                .filter_active(),
            "{width}x{height}: the forced filtered projection stays until the filter closes"
        );
    }
    assert!(
        !music_panel(&harness).test_hero_overlay_open(),
        "{width}x{height} {node:?} filtered={filtered}: no Hero opens over the tree"
    );
}

/// The painted point of the `make_music_group_app` album leaf's cached track
/// row, resolved from the tree's completed frame through the shared read-only
/// stable-target row geometry.
fn track_point(harness: &TickHarness, track: &str) -> (u16, u16) {
    let music = harness.model().test_music_owner();
    let node = music
        .browser
        .visible_targets()
        .into_iter()
        .find(|candidate| {
            matches!(candidate,
                MusicTreeTarget::Track { album, track: track_target }
                    if album == "album-1" && track_target == track)
        })
        .expect("painted track node");
    let row = music
        .browser
        .row_rect_for(&node)
        .expect("painted track row");
    (row.x, row.y)
}

/// Rows 5.2/5.3 end to end through the mounted composition: the tree's track
/// Enter chord and its track double-click both cross as the stable-identity
/// play-now intent, and the shell's one grouped-track resolver feeds the album
/// queue through the existing executor with the selected start index.
#[test]
fn music_tree_track_activation_plays_through_the_grouped_resolver() {
    let mut harness = expanded_music_tree_track_harness(true);

    // Enter on the selected tree track: the router reaches the owner's
    // track arm, whose request the shell resolves.
    let first_track = track_point(&harness, "track-1");
    harness.inject(left_click(first_track.0, first_track.1));
    dispatch_step(&mut harness);
    draw_frame(&mut harness);
    inject_key(&mut harness, Key::Enter);
    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["track-1", "track-2"],
        "autoload on queues the cached album from the selected tree track"
    );
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 0);

    // Double-click on the second track: the same resolver starts at that
    // track's own index. The run loop repaints between the two presses, as
    // the real event loop does. The previous activation left a populated
    // target queue, so row 5.4's gate raises the replacement confirmation and
    // only the confirmed action executes.
    let second_track = track_point(&harness, "track-2");
    harness.inject(left_click(second_track.0, second_track.1));
    dispatch_step(&mut harness);
    draw_frame(&mut harness);
    harness.inject(left_click(second_track.0, second_track.1));
    dispatch_step(&mut harness);
    assert!(
        confirm_mounted(&harness),
        "a populated target queue asks before the replacement"
    );
    assert_eq!(
        harness.model().app.playback_queue().queue_cursor,
        0,
        "the queue is unchanged before the confirmation"
    );
    inject_key(&mut harness, Key::Char('y'));
    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["track-1", "track-2"]
    );
    assert_eq!(
        harness.model().app.playback_queue().queue_cursor,
        1,
        "the double-clicked track is the start index after confirmation"
    );
}

/// Row 5.5: a double-click on an album leaf with no cached track children
/// claims the gesture without changing its expansion, opening a Hero, or
/// starting playback.
#[test]
fn double_click_a_childless_album_claims_the_gesture_unchanged() {
    let mut app = make_music_group_app_with_second_album();
    app.album_tracks_cache
        .insert("album-1".into(), vec![cached_track("track-1", 1)]);
    app.terminal_width = 160;
    app.terminal_height = 40;
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw_frame(&mut harness);

    let childless = album_target(&harness, "album-2");
    assert!(
        !harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&childless),
        "the childless leaf starts collapsed"
    );
    let at = tree_node_point(&harness, &childless);

    harness.inject(left_click(at.0, at.1));
    dispatch_step(&mut harness);
    draw_frame(&mut harness);
    harness.inject(left_click(at.0, at.1));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().all(|message| !matches!(
            message,
            Msg::Shell(ShellRequest::MusicTreeTrackActivate { .. })
        )),
        "a childless album claims the gesture without a track activation: {:?}",
        outcome.raw_messages
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        !harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&childless),
        "the claimed childless leaf keeps its expansion state"
    );
    assert!(
        !music_panel(&harness).test_hero_overlay_open(),
        "a childless album double-click opens no Hero"
    );
    assert_eq!(playback_queue_ids(&harness), Vec::<String>::new());
    assert!(harness.model().app.pending_queue_replacement.is_none());
}

/// Row 5.5: tree-track Enter feeds the one grouped resolver, whose autoload
/// policy decides whether the replacement queue is the whole cached album
/// (starting at the selected track) or only the selected Audio item.
#[rstest]
#[case::autoload_on(true, &["track-1", "track-2"], 1)]
#[case::autoload_off(false, &["track-2"], 0)]
fn tree_track_enter_follows_the_autoload_policy(
    #[case] autoload: bool,
    #[case] expected: &[&str],
    #[case] start: usize,
) {
    let mut harness = expanded_music_tree_track_harness(autoload);
    assert_eq!(
        harness.model().app.playback_queue().total_queue_len(),
        0,
        "the target queue starts empty, so the replacement needs no prompt"
    );

    let at = track_point(&harness, "track-2");
    activate_track(&mut harness, at, TrackActivation::Enter);

    assert!(!confirm_mounted(&harness));
    assert_eq!(playback_queue_ids(&harness), expected);
    assert_eq!(harness.model().app.playback_queue().queue_cursor, start);
}

/// Row 5.5: with an empty target queue both tree-track activation routes play
/// immediately, with no confirmation prompt.
#[rstest]
#[case::enter(TrackActivation::Enter)]
#[case::double_click(TrackActivation::DoubleClick)]
fn tree_track_activation_with_an_empty_queue_plays_immediately(
    #[case] kind: TrackActivation,
) {
    let mut harness = expanded_music_tree_track_harness(true);
    let at = track_point(&harness, "track-2");

    activate_track(&mut harness, at, kind);

    assert!(
        !confirm_mounted(&harness),
        "{kind:?}: an empty queue starts playback without a prompt"
    );
    assert_eq!(playback_queue_ids(&harness), ["track-1", "track-2"]);
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 1);
}

/// Row 5.5: with a populated target queue both tree-track activation routes
/// raise the replacement confirmation, change nothing before it, and only
/// play after the complete confirmation sequence.
#[rstest]
#[case::enter(TrackActivation::Enter)]
#[case::double_click(TrackActivation::DoubleClick)]
fn tree_track_activation_with_a_populated_queue_confirms_before_replacing(
    #[case] kind: TrackActivation,
) {
    let mut app = music_tree_track_app(true);
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    let mut harness = expanded_track_harness(app);

    let at = track_point(&harness, "track-2");
    activate_track(&mut harness, at, kind);

    assert!(
        confirm_mounted(&harness),
        "{kind:?}: a populated target queue asks before the replacement"
    );
    assert_eq!(
        playback_queue_ids(&harness),
        ["existing"],
        "{kind:?}: the prompt changes no queue"
    );
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 0);
    assert!(harness.model().app.pending_queue_replacement.is_some());

    inject_key(&mut harness, Key::Char('y'));

    assert!(!confirm_mounted(&harness));
    assert!(harness.model().app.pending_queue_replacement.is_none());
    assert_eq!(playback_queue_ids(&harness), ["track-1", "track-2"]);
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 1);
}

/// Rows 5.4/5.5: cancelling the populated-queue confirmation leaves the queue
/// and playback unchanged, clears the pending payload, and a later sync pass
/// does not resurrect it.
#[test]
fn cancelling_the_replace_queue_confirmation_leaves_the_queue_unchanged() {
    let mut app = music_tree_track_app(true);
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    let mut harness = expanded_track_harness(app);

    let at = track_point(&harness, "track-2");
    activate_track(&mut harness, at, TrackActivation::Enter);
    assert!(confirm_mounted(&harness));

    inject_key(&mut harness, Key::Esc);

    assert!(!confirm_mounted(&harness));
    assert!(
        harness.model().app.pending_queue_replacement.is_none(),
        "cancellation leaves no executable payload behind"
    );
    assert_eq!(playback_queue_ids(&harness), ["existing"]);
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 0);

    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(playback_queue_ids(&harness), ["existing"]);
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 0);
}
