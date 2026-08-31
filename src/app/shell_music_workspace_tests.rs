use super::*;
use crate::app::components::msg::{AlbumCursorKind, ShellRequest};
use crate::app::components::Msg;
use crate::app::layout::LibraryRowTarget;
pub(crate) use crate::app::render::make_music_group_app;
pub(crate) use crate::app::tests::make_item;
use crate::app::{BrowseLevel, LibraryTab, PanelFocus, TabSelection};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
#[test]
fn music_mouse_album_click_emits_and_shell_applies_cursor() {
    let mut app = make_music_group_app();
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(second_album);
    let mut model = Model::new(app);
    assert_eq!(model.app.libs[0].nav_stack[1].resting().cursor(), 0);
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();
    let id = model.music_workspace_id.clone().unwrap();
    let (browser_area, left_row_targets) = {
        let layout = model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<MusicWorkspaceComponent>()
            .unwrap()
            .layout();
        (
            layout.wide_music_browser_area,
            layout.left_row_targets.clone(),
        )
    };
    let (row, target) = left_row_targets
        .iter()
        .enumerate()
        .filter_map(|(row, target)| match target {
            Some(LibraryRowTarget::Album(album)) => Some((row, *album)),
            _ => None,
        })
        .nth(1)
        .expect("second painted album target");
    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: browser_area.x + 1,
            row: browser_area.y + row as u16,
            modifiers: KeyModifiers::NONE,
        }));
    assert_eq!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target,
            kind: AlbumCursorKind::Move
        }))
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    model.handle_terminal_message(
        message.expect("album cursor request"),
        Some(&id),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(model.app.libs[0].nav_stack[1].resting().cursor(), target);
}
#[test]
fn shell_mounts_and_syncs_music_workspace() {
    let mut model = Model::new(make_music_group_app());
    // The mounted browser is in the post-hero right rail, whose width—not
    // the pre-hero left area—determines horizontal navigation availability.
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 200, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(100, 0, 100, 30);
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("Music workspace mounted");
    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
    ));
    // Grouped Music paints one album per row; horizontal keys fall through to
    // the central router rather than being claimed by this component.
    for code in [Key::Left, Key::Right, Key::Char('h'), Key::Char('l')] {
        let message = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
            }));
        assert_eq!(message, None, "unexpected claim for {code:?}");
    }
}
#[test]
fn push_music_workspace_fetches_selected_album_tracks() {
    let mut model = Model::new(make_music_group_app());
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    model.app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    model.sync_music_workspace();
    assert!(model.app.album_tracks_loading.contains("album-1"));
    let component = model
        .application
        .get_component(&model.music_workspace_id.clone().unwrap())
        .unwrap()
        .as_any()
        .downcast_ref::<MusicWorkspaceComponent>()
        .unwrap();
    assert!(
        component.album_tracks_loading(),
        "first mounted content push must project album track loading"
    );
}

#[test]
fn grouped_music_cursor_no_fallthrough_when_left_sorted_indices_empty() {
    let mut model = Model::new(make_music_group_app());
    // Add sibling albums so the display order (sorted by name) differs
    // from raw insertion order: raw [0 "First Album", 1 "Zebra Album",
    // 2 "Mango Album"] sorts to display order [0, 2, 1].
    let mut zebra = crate::app::tests::make_item("Zebra Album", "MusicAlbum");
    zebra.artist = "Charlie".into();
    let mut mango = crate::app::tests::make_item("Mango Album", "MusicAlbum");
    mango.artist = "Bravo".into();
    model.app.libs[0].nav_stack[1].items.extend([zebra, mango]);
    // Force a single column so the display-order move is deterministic.
    model.app.layout.main.left_area.width = 40;

    // No library-list render has run, so the render-output order the
    // legacy fallback would have read is empty.
    assert!(model.app.layout.main.left_sorted_indices.is_empty());

    model.sync_music_workspace();
    let id = model.music_workspace_id.clone().expect("mounted");

    let order = model.app.wide_music_render_ctx(0, None).album_order.clone();
    assert_eq!(order, vec![0, 2, 1], "display order must differ from raw");

    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    let target = match message {
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target,
            kind: AlbumCursorKind::Move,
        })) => target,
        other => panic!("Down must emit an album cursor intent, got {other:?}"),
    };
    // The target is the display-order successor of raw index 0 (== order[1]),
    // never the raw successor (1) the legacy empty-left_sorted_indices path used.
    assert_eq!(target, order[1]);
    assert_ne!(target, 1, "must not fall through to raw-index navigation");

    // The shell arm applies the target via the display-order cursor setter,
    // which must not fall through to raw-index navigation.
    assert!(model.app.move_music_group_display_cursor(0, target));
    assert_eq!(model.app.libs[0].nav_stack[1].resting().cursor(), order[1]);
}

#[test]
fn shell_executes_grouped_music_image_paint() {
    let mut model = Model::new(make_music_group_app());
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    model.app.image_protocol_enabled = true;
    model.app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();

    assert!(model.app.card_image_loading.contains("album-1:P"));
}

#[test]
fn narrow_music_workspace_emits_selected_album_image_request() {
    let mut model = Model::new(make_music_group_app());
    model.app.image_protocol_enabled = true;
    model.app.layout.main.left_area = ratatui::layout::Rect::new(0, 0, 81, 20);
    model.sync_music_workspace();

    let mut terminal = Terminal::new(TestBackend::new(81, 20)).unwrap();
    terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();

    assert!(
        model.app.card_image_loading.contains("album-1:P"),
        "narrow selected album must emit a typed image-loading request"
    );
}

#[test]
fn shell_mounts_music_workspace_in_narrow_mode() {
    let mut model = Model::new(make_music_group_app());
    assert!(model.app.is_music_group_view(0));
    assert!(model.app.is_viewing_album_folders(0));
    assert!(!model.app.layout.main.is_wide_music_active());

    let wide_area = model.app.layout.main.wide_music_area;
    assert_eq!(wide_area.width, 0);
    assert_eq!(wide_area.height, 0);
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("narrow Music workspace mounted");
    assert!(model.application.mounted(&id));
    assert_eq!(model.app.layout.main.wide_music_area, wide_area);
}

#[test]
fn music_resize_push_uses_current_frame_geometry() {
    let mut model = Model::new(make_music_group_app());
    let mut track = crate::app::tests::make_item("Track 1", "Audio");
    track.id = "track-1".into();
    model
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![track]);
    model.sync_music_workspace();

    let mut wide_terminal = Terminal::new(TestBackend::new(160, 30)).unwrap();
    wide_terminal
        .draw(|frame| {
            model.app.compose_base_frame(frame, None);
            model.render_music_workspace_component(frame);
        })
        .unwrap();
    model.push_music_workspace_content();
    let id = model.music_workspace_id.clone().unwrap();
    {
        let wide = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MusicWorkspaceComponent>()
            .unwrap();
        wide.enter_track_focus();
        assert!(model.app.layout.main.is_wide_music_active());
        assert_eq!(wide.track_cursor(), Some(0));
    }
    let hitmap_before = model.app.layout.main.wide_music_track_hitmap.len();
    assert!(
        hitmap_before > 0,
        "wide music render did not publish track hitmap: area={:?}, id={id:?}",
        model.app.layout.main.wide_music_area
    );
    wide_terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();
    assert_eq!(
        model.app.layout.main.wide_music_track_hitmap.len(),
        hitmap_before
    );

    let mut narrow_terminal = Terminal::new(TestBackend::new(60, 30)).unwrap();
    narrow_terminal
        .draw(|frame| {
            model.app.compose_base_frame(frame, None);
            model.render_music_workspace_component(frame);
        })
        .unwrap();
    model.push_music_workspace_content();
    let narrow = model
        .application
        .get_component(&id)
        .unwrap()
        .as_any()
        .downcast_ref::<MusicWorkspaceComponent>()
        .unwrap();
    assert!(!model.app.layout.main.is_wide_music_active());
    assert_eq!(narrow.track_cursor(), None);
}

#[test]
fn narrow_music_workspace_requests_album_activation() {
    let mut model = Model::new(make_music_group_app());
    assert!(!model.app.layout.main.is_wide_music_active());
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("narrow Music workspace mounted");
    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
    assert_eq!(message, Some(Msg::Shell(ShellRequest::MusicAlbumActivate)));
    let mut music_resize = false;
    let mut tv_resize = false;
    model.handle_terminal_message(
        message.expect("album activation request"),
        Some(&id),
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(model.app.pending_overlay.is_some());
}

#[test]
fn wide_music_workspace_allows_enter_for_inline_track_focus() {
    let mut model = Model::new(make_music_group_app());
    let mut track = crate::app::tests::make_item("Track One", "Audio");
    track.id = "track-1".into();
    model
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![track]);
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    assert!(model.app.layout.main.is_wide_music_active());
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("wide Music workspace mounted");
    model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MusicWorkspaceComponent>()
        .unwrap();
    assert_eq!(component.track_cursor(), Some(0));
}

#[test]
fn recursive_album_activation_enters_track_focus_only_in_wide() {
    // Recursive album activation used to write
    // `Some(0)` on the deleted inline track-focus field; the shell now delivers a
    // one-shot enter request consumed at the next content push -- wide only, so
    // narrow stays explicitly unfocused.
    let mut model = Model::new(make_music_group_app());
    let mut track = crate::app::tests::make_item("Track One", "Audio");
    track.id = "track-1".into();
    model
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![track]);
    model.sync_music_workspace();
    assert!(!model.app.layout.main.is_wide_music_active());
    let id = model
        .music_workspace_id
        .clone()
        .expect("narrow Music workspace mounted");

    model.music_track_focus_request = Some(true);
    model.push_music_workspace_content();
    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MusicWorkspaceComponent>()
        .unwrap();
    assert_eq!(
        component.track_cursor(),
        None,
        "narrow keeps inline track focus explicitly off"
    );

    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    model.music_track_focus_request = Some(true);
    model.push_music_workspace_content();
    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MusicWorkspaceComponent>()
        .unwrap();
    assert_eq!(
        component.track_cursor(),
        Some(0),
        "wide recursive activation enters track focus"
    );
}

#[test]
fn position_restore_request_clears_track_focus_at_next_sync() {
    let mut model = Model::new(make_music_group_app());
    let mut track = crate::app::tests::make_item("Track One", "Audio");
    track.id = "track-1".into();
    model
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![track]);
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("wide Music workspace mounted");
    model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));

    // The deleted track-focus-clear rehome: a position-restore request
    // clears the component's inline track focus at the next content push.
    model.music_track_focus_request = Some(false);
    model.sync_music_workspace();
    assert_eq!(model.music_track_focus_request, Some(false));
    model.push_music_workspace_content();
    let component = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MusicWorkspaceComponent>()
        .unwrap();
    assert_eq!(component.track_cursor(), None);
}

/// keep-destination-components-mounted task 3.2: the Music workspace
/// stays mounted across a drill into a track list and back (keep-mounted,
/// D1). Viewing album folders, moving the album cursor, drilling into a
/// track list (`is_viewing_album_folders` → false, pointer → `None`), and
/// going back must leave the `MusicWorkspaceComponent` still mounted with
/// the album cursor where it was — not reset by a drill-time
/// unmount/remount.
///
/// The component-local cursor is deliberately made to DIVERGE from the
/// App/library cursor: a key event moves the component cursor to 1 while
/// App's nav cursor stays 0 (the emitted request is deliberately not
/// applied). A remount would re-sync the component cursor from App (0),
/// so preserving the divergent value 1 across drill→return proves the
/// component was not remounted and its private state survived.
#[test]
fn music_workspace_stays_mounted_and_preserves_album_cursor_across_drill() {
    let mut app = crate::app::tests::make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.music_levels = vec!["group".into(), "album".into()];
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();
    let mut group = make_item("Alpha", "MusicArtist");
    group.id = "group-0".into();
    group.is_folder = true;
    let albums: Vec<_> = (0..3)
        .map(|i| {
            let mut album = make_item(&format!("Album {i}"), "MusicAlbum");
            album.id = format!("album-{i}");
            album.artist = "Alpha".into();
            album.is_folder = true;
            album
        })
        .collect();
    app.libs.push(LibraryTab {
        nav_stack: vec![
            BrowseLevel {
                parent_id: "lib-music".into(),
                title: "Music".into(),
                items: vec![group],
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
            BrowseLevel {
                parent_id: "group-0".into(),
                title: "Alpha".into(),
                items: albums,
                total_count: 3,
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
        ],
        ..LibraryTab::new(library)
    });
    let mut model = Model::new(app);
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("Music workspace mounted");
    let album_cursor = |model: &Model| {
        model
            .application
            .get_component(
                &model
                    .music_workspace_id
                    .clone()
                    .expect("Music workspace mounted"),
            )
            .and_then(|comp| comp.as_any().downcast_ref::<MusicWorkspaceComponent>())
            .map(MusicWorkspaceComponent::album_cursor)
            .expect("album cursor")
    };
    // App and component cursors both start at 0.
    assert_eq!(model.app.libs[0].nav_stack[1].resting().cursor(), 0);
    assert_eq!(album_cursor(&model), 0);

    // Move ONLY the component-local cursor to a divergent value (1): the
    // emitted MusicAlbumCursor request is deliberately not applied to App,
    // so App stays at 0. A remount would re-sync the component from App
    // (0), so this divergence is the discriminating signal.
    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: 1,
            kind: AlbumCursorKind::Move
        }))
    ));
    assert_eq!(album_cursor(&model), 1, "component cursor must diverge");
    assert_eq!(
        model.app.libs[0].nav_stack[1].resting().cursor(),
        0,
        "App cursor must stay put"
    );

    // Drill into a track list: push a third nav level so
    // `is_viewing_album_folders` becomes false (top music_levels entry
    // is no longer "album"). The pointer goes None but the component
    // stays mounted (keep-mounted).
    let mut track = make_item("Track 1", "Audio");
    track.id = "track-1".into();
    model.app.libs[0].nav_stack.push(BrowseLevel {
        parent_id: "album-0".into(),
        title: "Tracks".into(),
        items: vec![track],
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
    });
    model.sync_music_workspace();
    assert!(!model.app.is_viewing_album_folders(0));
    assert_eq!(model.music_workspace_id, None);
    assert!(
        model.application.mounted(&id),
        "the Music workspace must stay mounted across the drill"
    );

    // Go back: the album level returns, the pointer is restored, and the
    // same component is re-pointed (not remounted), preserving the
    // divergent component-local cursor.
    model.app.go_back(0);
    model.sync_music_workspace();
    assert!(model.app.is_viewing_album_folders(0));
    assert_eq!(
        model.music_workspace_id.as_ref(),
        Some(&id),
        "re-point must restore the same component id"
    );
    assert!(model.application.mounted(&id));
    assert_eq!(
        album_cursor(&model),
        1,
        "the divergent component-local album cursor must survive the drill-and-return round trip"
    );
    assert_eq!(
        model.app.libs[0].nav_stack[1].resting().cursor(),
        0,
        "App cursor stays at its own value throughout"
    );
}

/// migrate-narrow-browse-to-components task 2.4 (D6, first half): at a
/// narrow width (no `wide_music_area`), a grouped album-folder Emby Music
/// library's `MusicWorkspaceComponent` is both *rendered* (its `view` is
/// reached via the `left_area` fallback — proven by `render_music_workspace`
/// `_component` publishing geometry into `wide_music_area`, which a
/// early-return would leave zeroed) and *focusable* (the active-destination
/// pass lands TuiRealm focus on its `ComponentId::Browser{..Music}`). It
/// still paints nothing until task 3.6 gives it a narrow branch.
#[test]
fn narrow_grouped_music_workspace_is_rendered_and_focusable() {
    let mut model = Model::new(make_music_group_app());
    model.app.panel_focus = PanelFocus::Library;
    assert!(model.app.is_music_group_view(0));
    assert!(model.app.is_viewing_album_folders(0));
    assert!(!model.app.layout.main.is_wide_music_active());

    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("narrow Music workspace mounted");
    assert!(matches!(
        id,
        ComponentId::Browser(BrowserKey {
            kind: BrowserKind::Music,
            ..
        })
    ));

    // Render at a narrow width: no `wide_music_area`, only a narrow
    // `left_area`, so the component's `view` is reached only via the
    // `left_area` fallback.
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::default();
    model.app.layout.main.left_area = ratatui::layout::Rect::new(0, 0, 50, 28);
    let mut terminal = Terminal::new(TestBackend::new(60, 30)).unwrap();
    terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();
    assert!(
        model.app.layout.main.wide_music_area.width > 0
            && model.app.layout.main.wide_music_area.height > 0,
        "render_music_workspace_component must reach the component view via the \
         left_area fallback and publish geometry, not early-return at narrow"
    );
    assert!(
        !model.app.layout.main.is_wide_music_active(),
        "the narrow fallback must not mark the wide Music layout active"
    );

    // Focus: the active-destination pass lands on the mounted component.
    model.sync_active_destination();
    assert_eq!(model.application.focus(), Some(&id));
}

fn music_album_cursor(model: &Model, id: &ComponentId) -> usize {
    model
        .application
        .get_component(id)
        .and_then(|comp| comp.as_any().downcast_ref::<MusicWorkspaceComponent>())
        .map(MusicWorkspaceComponent::album_cursor)
        .expect("album cursor")
}

fn music_group_app_two_albums() -> crate::app::App {
    let mut app = make_music_group_app();
    let mut second = make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(second);
    app.libs[0].nav_stack[1].total_count = 2;
    app
}

#[test]
fn music_workspace_first_mount_adopts_restored_album_cursor() {
    // A saved position restored into the nav stack before the workspace
    // mounts: the first projection re-anchors the component to it (D1's
    // third re-anchor site), rather than starting at 0.
    let mut model = Model::new(music_group_app_two_albums());
    model.app.libs[0].nav_stack[1].set_resting_cursor(1);
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("Music workspace mounted");
    assert_eq!(music_album_cursor(&model, &id), 1);
}

#[test]
fn music_workspace_reanchor_lands_regardless_of_prior_local_move() {
    let mut model = Model::new(music_group_app_two_albums());
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("Music workspace mounted");

    // Local move: component cursor diverges to 1; the emitted request is
    // deliberately not applied to App, so the nav level stays at 0.
    model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert_eq!(music_album_cursor(&model, &id), 1);

    // A genuine shell re-anchor at a navigation event lands anyway.
    model.music_workspace_reanchor = true;
    model.push_music_workspace_content();
    assert_eq!(music_album_cursor(&model, &id), 0);
}

#[test]
fn music_workspace_ordinary_push_does_not_touch_album_cursor() {
    let mut model = Model::new(music_group_app_two_albums());
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
    model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
    model.sync_music_workspace();
    let id = model
        .music_workspace_id
        .clone()
        .expect("Music workspace mounted");

    model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert_eq!(music_album_cursor(&model, &id), 1);

    // No re-anchor: an ordinary content push leaves the local cursor put
    // even though the nav level's cursor is 0.
    model.push_music_workspace_content();
    assert_eq!(music_album_cursor(&model, &id), 1);
}
