//! Task 9.4: the Music component tests re-pointed at the panel-embedded
//! `MusicContent` owner and `LibraryPanel` (replacing the retired mounted
//! `MusicWorkspaceComponent`/`music_workspace_id` shape these tests used to
//! exercise). Ported from the orphaned `shell_music_workspace_cursor_tests.rs`
//! / `shell_music_workspace_mouse_tests.rs` / `shell_music_workspace_tests.rs`
//! (deleted alongside this file's introduction) wherever their assertion was
//! not already covered by `tests_tick_integration_music_mouse.rs`,
//! `tests_tick_integration.rs`, `shell_destination_mounts.rs`, or
//! `render/tests_music_narrow.rs`.

use super::*;
use crate::app::components::library_panel::owner::LibraryContentOwner;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::msg::AlbumCursorKind;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::make_music_group_app;
use crate::app::tests::make_item;
use crate::app::{BrowseLevel, LibraryTab, PanelFocus};
use mbv_core::config::ServiceKind;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn music_group_app_two_albums() -> crate::app::App {
    let mut app = make_music_group_app();
    let mut second = make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(second);
    app.libs[0].nav_stack[1].total_count = 2;
    app
}

fn wide(model: &mut Model) {
    model.app.terminal_width = 160;
    model.app.terminal_height = 40;
}

// ── Keyboard: activation / inline track focus ────────────────────────────

#[test]
fn narrow_enter_requests_album_activation() {
    let mut model = Model::new(make_music_group_app());
    model.app.panel_focus = PanelFocus::Library;
    model.sync_mounted_surfaces();
    assert!(!model.app.is_right_panel_wide());

    let message = model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumActivate { .. }))
    ));
    let (mut music_resize, mut tv_resize) = (false, false);
    model.handle_terminal_message(
        message.expect("activation request"),
        &mut music_resize,
        &mut tv_resize,
    );
    let panel = model
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel");
    assert!(panel.test_hero_overlay_open());
}

#[test]
fn wide_enter_enters_inline_track_focus() {
    let mut model = Model::new(make_music_group_app());
    let mut track = make_item("Track One", "Audio");
    track.id = "track-1".into();
    model
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![track]);
    model.app.panel_focus = PanelFocus::Library;
    wide(&mut model);
    model.sync_mounted_surfaces();
    assert!(model.app.is_right_panel_wide());

    model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    });
    assert!(model.test_music_owner().track_focused());
}

#[test]
fn recursive_album_activation_enters_track_focus_only_in_wide() {
    let mut model = Model::new(make_music_group_app());
    let mut track = make_item("Track One", "Audio");
    track.id = "track-1".into();
    model
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![track]);
    model.app.panel_focus = PanelFocus::Library;
    model.sync_mounted_surfaces();
    assert!(!model.app.is_right_panel_wide());

    model.music_track_focus_request = Some(MusicTrackFocusRequest::Enter {
        album_id: "album-1".into(),
    });
    model.push_music_workspace_content();
    assert!(
        !model.test_music_owner().track_focused(),
        "narrow keeps inline track focus explicitly off"
    );

    wide(&mut model);
    model.music_track_focus_request = Some(MusicTrackFocusRequest::Enter {
        album_id: "album-1".into(),
    });
    model.push_music_workspace_content();
    assert!(
        model.test_music_owner().track_focused(),
        "wide recursive activation enters track focus"
    );
}

#[test]
fn wide_enter_request_defers_until_the_activated_album_tracks_arrive() {
    let mut model = Model::new(make_music_group_app());
    model.app.panel_focus = PanelFocus::Library;
    wide(&mut model);
    model.sync_mounted_surfaces();

    model.music_workspace_reanchor = true;
    model.music_track_focus_request = Some(MusicTrackFocusRequest::Enter {
        album_id: "album-1".into(),
    });
    model.push_music_workspace_content();
    assert!(
        !model.test_music_owner().track_focused(),
        "tracks not cached yet, so track focus cannot be entered"
    );
    assert_eq!(
        model.music_track_focus_request,
        Some(MusicTrackFocusRequest::Enter {
            album_id: "album-1".into()
        }),
        "the request stays armed for the activated album"
    );

    let mut track = make_item("Track One", "Audio");
    track.id = "track-1".into();
    model
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![track]);
    model.push_music_workspace_content();
    assert!(
        model.test_music_owner().track_focused(),
        "the tracks re-push honors the deferred request"
    );
    assert_eq!(model.music_track_focus_request, None, "request consumed");
}

#[test]
fn position_restore_request_clears_track_focus_at_next_sync() {
    let mut model = Model::new(make_music_group_app());
    let mut track = make_item("Track One", "Audio");
    track.id = "track-1".into();
    model
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![track]);
    model.app.panel_focus = PanelFocus::Library;
    wide(&mut model);
    model.sync_mounted_surfaces();
    model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    });
    assert!(model.test_music_owner().track_focused());

    model.music_track_focus_request = Some(MusicTrackFocusRequest::Clear);
    model.push_music_workspace_content();
    assert!(!model.test_music_owner().track_focused());
}

/// A wide->narrow breakpoint flip forces inline track focus off (design.md
/// D5: narrow never enters track focus), even if the wide side had it.
#[test]
fn narrow_resize_clears_inline_track_focus() {
    let mut model = Model::new(make_music_group_app());
    let mut track = make_item("Track One", "Audio");
    track.id = "track-1".into();
    model
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![track]);
    model.app.panel_focus = PanelFocus::Library;
    wide(&mut model);
    model.sync_mounted_surfaces();
    model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    });
    assert!(model.test_music_owner().track_focused());

    model.app.terminal_width = 60;
    model.app.terminal_height = 24;
    model.sync_mounted_surfaces();
    assert!(!model.test_music_owner().track_focused());
}

// ── Owner retention (design D2): a drill and a tab change both keep the
// inactive Music owner's cursor/scroll, since the panel retains an owner
// while its library stays in the catalog. ─────────────────────────────────

fn music_library_app_with_three_albums() -> crate::app::App {
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
    app
}

fn music_key() -> LibraryKey {
    LibraryKey::Service {
        service: ServiceKind::Emby,
        library_id: "lib-music".into(),
        kind: LibraryKind::Music,
    }
}

/// keep-destination-components-mounted / task 9.4: the Music owner stays
/// installed in the `LibraryPanel`'s map across a drill into a track list and
/// back, with its own local album cursor -- not reset by the drill (design
/// D2's retention rule; the panel retains an owner while its library stays in
/// the catalog, independent of whether `is_viewing_album_folders` currently
/// makes it the *active* owner).
///
/// The component-local cursor is deliberately made to diverge from the
/// App/library cursor: a key event moves the owner cursor to 1 while App's
/// nav cursor stays 0 (the emitted request is deliberately not applied). If
/// the owner were dropped and recreated, its cursor would re-sync from App
/// (0), so preserving 1 across the drill-and-return proves it was retained.
#[test]
fn music_owner_stays_installed_and_preserves_album_cursor_across_drill() {
    let mut model = Model::new(music_library_app_with_three_albums());
    model.sync_mounted_surfaces();
    let key = music_key();
    assert!(model.library_panel_has_owner(&key));

    assert_eq!(model.app.libs[0].nav_stack[1].resting().cursor(), 0);
    assert_eq!(model.test_music_owner().album_cursor(), 0);

    let message = model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: 1,
            kind: AlbumCursorKind::Move
        }))
    ));
    assert_eq!(
        model.test_music_owner().album_cursor(),
        1,
        "owner cursor must diverge"
    );
    assert_eq!(
        model.app.libs[0].nav_stack[1].resting().cursor(),
        0,
        "App cursor must stay put"
    );

    // Drill into a track list: push a third nav level so
    // `is_viewing_album_folders` becomes false. Music is no longer the
    // active owner, but the panel keeps it retained.
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
    model.sync_mounted_surfaces();
    assert!(!model.app.is_viewing_album_folders(0));
    assert!(
        model.music_owner().is_none(),
        "Music is no longer the active owner"
    );
    assert!(
        model.library_panel_has_owner(&key),
        "the Music owner must stay installed across the drill"
    );

    // Go back: the album level returns, Music becomes active again, and the
    // divergent owner cursor survived untouched.
    model.app.go_back(0);
    model.sync_mounted_surfaces();
    assert!(model.app.is_viewing_album_folders(0));
    assert_eq!(
        model.test_music_owner().album_cursor(),
        1,
        "the divergent owner-local album cursor must survive the drill-and-return round trip"
    );
    assert_eq!(
        model.app.libs[0].nav_stack[1].resting().cursor(),
        0,
        "App cursor stays at its own value throughout"
    );
}

/// Task 9.4's verify bullet: "an inactive Music owner keeps cursor/scroll
/// across a tab change." Move the owner's cursor while Music is active, flip
/// the tab to Home (Music becomes inactive, its owner un-painted but still
/// retained per design D2 since its library stays in the catalog), flip back,
/// and confirm the divergent cursor/scroll survived.
#[test]
fn music_owner_keeps_cursor_and_scroll_across_a_tab_change() {
    let mut app = make_music_group_app();
    for index in 2..20 {
        let mut album = make_item(&format!("Album {index}"), "MusicAlbum");
        album.id = format!("album-{index}");
        album.artist = "Alpha".into();
        app.libs[0].nav_stack[1].items.push(album);
    }
    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    let key = music_key();
    assert!(model.library_panel_has_owner(&key));

    model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(model.test_music_owner().album_cursor(), 1);
    // A non-trivial scroll offset, distinct from the cursor: proves the full
    // viewport position survives, not just the selected target.
    model.test_music_owner_mut().re_anchor(10, 7);
    assert_eq!(model.test_music_owner().album_cursor(), 10);
    // The carrier clamps the requested scroll to its own valid range for the
    // fixture's short flow; read back the clamped value as the ground truth
    // the round trip below must preserve exactly.
    let scroll = model.test_music_owner().album_scroll();

    // Tab away to Home: Music is no longer the active owner, but stays
    // retained (its library is still in the catalog).
    model.app.tab = TabSelection::Home;
    model.sync_mounted_surfaces();
    assert!(model.music_owner().is_none());
    assert!(
        model.library_panel_has_owner(&key),
        "the inactive Music owner must stay installed across the tab change"
    );

    // Tab back: the same owner re-activates with its cursor/scroll intact.
    model.app.tab = TabSelection::EmbyLibrary(0);
    model.sync_mounted_surfaces();
    assert_eq!(
        model.test_music_owner().album_cursor(),
        10,
        "the owner's album cursor survives the tab round trip"
    );
    assert_eq!(
        model.test_music_owner().album_scroll(),
        scroll,
        "the owner's scroll survives the tab round trip"
    );
}

// ── Re-anchor mechanics ───────────────────────────────────────────────────

#[test]
fn music_owner_reanchor_lands_regardless_of_prior_local_move() {
    let mut model = Model::new(music_group_app_two_albums());
    model.sync_mounted_surfaces();

    // Local move: owner cursor diverges to 1; the emitted request is
    // deliberately not applied to App, so the nav level stays at 0.
    model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(model.test_music_owner().album_cursor(), 1);

    // A genuine shell re-anchor at a navigation event lands anyway.
    model.music_workspace_reanchor = true;
    model.push_music_workspace_content();
    assert_eq!(model.test_music_owner().album_cursor(), 0);
}

// ── Keyboard: album-level navigation (task 9.4 restored this entirely; see
// commit 40b7a361) ─────────────────────────────────────────────────────────

/// Down/PageDown move the owner's album cursor by one row / by the shared
/// carrier's page stride, and the shell applies the emitted requests onto
/// the resting browse position -- not just the owner's own local state.
#[test]
fn down_and_page_down_move_the_album_cursor_and_shell_applies_it() {
    let mut model = Model::new(make_music_group_app());
    for index in 2..8 {
        let mut album = make_item(&format!("Album {index}"), "MusicAlbum");
        album.id = format!("album-{index}");
        album.artist = "Alpha".into();
        model.app.libs[0].nav_stack[1].items.push(album);
    }
    model.app.panel_focus = PanelFocus::Library;
    model.sync_mounted_surfaces();

    let message = model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    let Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target, kind })) = message else {
        panic!("expected MusicAlbumCursor, got {message:?}");
    };
    assert_eq!(target, 1);
    assert_eq!(kind, AlbumCursorKind::Move);
    let (mut music_resize, mut tv_resize) = (false, false);
    model.handle_terminal_message(message.unwrap(), &mut music_resize, &mut tv_resize);
    assert_eq!(
        model.app.libs[0].nav_stack[1].resting().cursor(),
        1,
        "the shell must apply the emitted cursor request to the resting position"
    );

    let message = model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::PageDown,
        modifiers: KeyModifiers::NONE,
    });
    assert!(
        matches!(
            message,
            Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                kind: AlbumCursorKind::Page,
                ..
            }))
        ),
        "PageDown must move by the shared carrier's page stride: {message:?}"
    );
}

/// The album cursor moves by the same sorted display order the panel paints
/// (design D3), never raw insertion order, even before any frame has been
/// drawn.
#[test]
fn down_key_moves_by_display_order_not_raw_insertion_order() {
    let mut model = Model::new(make_music_group_app());
    // Raw insertion order [0 "First Album", 1 "Zebra Album", 2 "Mango
    // Album"] sorts (by artist, stable within an artist) to display order
    // [0, 2, 1] once Bravo/Charlie sort ahead of... no: sorted by artist name
    // "Alpha" < "Bravo" < "Charlie", so Mango (Bravo) sorts before Zebra
    // (Charlie), landing display order [0, 2, 1].
    let mut zebra = make_item("Zebra Album", "MusicAlbum");
    zebra.id = "album-zebra".into();
    zebra.artist = "Charlie".into();
    let mut mango = make_item("Mango Album", "MusicAlbum");
    mango.id = "album-mango".into();
    mango.artist = "Bravo".into();
    model.app.libs[0].nav_stack[1].items.extend([zebra, mango]);
    model.app.panel_focus = PanelFocus::Library;
    model.sync_mounted_surfaces();

    let order = model.app.wide_music_render_ctx(0, None).album_order.clone();
    assert_eq!(order, vec![0, 2, 1], "display order must differ from raw");

    let message = model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    let target = match message {
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target,
            kind: AlbumCursorKind::Move,
        })) => target,
        other => panic!("Down must emit an album cursor intent, got {other:?}"),
    };
    assert_eq!(target, order[1]);
    assert_ne!(target, 1, "must not fall through to raw-index navigation");
}

// ── Keyboard: shortcuts reuse the component's own selection ──────────────

#[test]
fn period_and_slash_keys_use_the_owners_own_selection() {
    let mut model = Model::new(make_music_group_app());
    model.app.panel_focus = PanelFocus::Library;
    model.sync_mounted_surfaces();

    let message = model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Char('.'),
        modifiers: KeyModifiers::NONE,
    });
    let Some(Msg::Shell(ShellRequest::RowContextMenu(
        crate::app::types_context_menu::ContextMenuTargets::Emby(mut items),
        anchor,
    ))) = message
    else {
        panic!("Music '.' must emit a library context-menu request, got {message:?}");
    };
    let item = items.pop().expect("context item");
    assert!(anchor.is_none());
    assert_eq!(item.item_type, "MusicAlbum");
    let (mut music_resize, mut tv_resize) = (false, false);
    model.handle_terminal_message(
        Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(vec![item]),
            None,
        )),
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(matches!(
        model.app.pending_overlay,
        Some(crate::app::types_overlay::OverlayRequest::ContextMenu(_))
    ));

    let message = model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(message, Some(Msg::Shell(ShellRequest::OpenInlineSearch)));
}

/// Grouped Music paints one album per row; horizontal keys are unclaimed so
/// the central router keeps precedence over them.
#[test]
fn horizontal_keys_fall_through_unclaimed() {
    let mut model = Model::new(make_music_group_app());
    model.app.panel_focus = PanelFocus::Library;
    model.sync_mounted_surfaces();
    for code in [Key::Left, Key::Right, Key::Char('h'), Key::Char('l')] {
        let message = model.test_music_owner_mut().on_key(&KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(message, None, "unexpected claim for {code:?}");
    }
}

// ── Mouse: album-list gestures the shared tick-integration coverage does
// not already exercise ────────────────────────────────────────────────────

/// A click on a non-selectable structural row (the artist heading) resolves
/// to nothing.
#[test]
fn narrow_heading_click_resolves_to_nothing() {
    let mut model = Model::new(make_music_group_app());
    for index in 0..4 {
        let mut album = make_item(&format!("Album {}", index + 2), "MusicAlbum");
        album.id = format!("album-{}", index + 2);
        album.artist = "Alpha".into();
        model.app.libs[0].nav_stack[1].items.push(album);
    }
    model.app.panel_focus = PanelFocus::Library;
    model.app.terminal_width = 60;
    model.app.terminal_height = 9;
    model.sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(60, 9)).unwrap();
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    model.sync_mounted_surfaces();

    let layout = model.test_painted_library_layout();
    let heading_point = (layout.left_area.x + 1, layout.left_area.y);
    let message = model.test_music_owner_mut().on_slot_event(
        crate::app::components::library_panel::owner::LibrarySlotEvent::List(
            crate::app::components::media_list::MediaListSurfaceInput::Click(
                ratatui::layout::Position::new(heading_point.0, heading_point.1),
            ),
        ),
    );
    assert_eq!(message, None, "the artist heading claims no click");
}

/// Right-click on a painted album row emits the album-level context-menu
/// request (distinct from the track table's own right-click, already
/// covered by `tests_tick_integration_music_mouse.rs`).
#[test]
fn album_row_right_click_requests_the_album_context_menu() {
    let mut model = Model::new(make_music_group_app());
    model.app.panel_focus = PanelFocus::Library;
    model.app.terminal_width = 100;
    model.app.terminal_height = 30;
    model.sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| model.draw_frame(frame, false, false))
        .unwrap();
    model.sync_mounted_surfaces();

    let selected = model
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("LibraryPanel mounted")
        .test_painted_layout()
        .selected_item_rect
        .expect("wide selected album row painted");

    let message = model
        .application
        .get_component_mut(&ComponentId::Library)
        .expect("LibraryPanel mounted")
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: selected.x + 1,
            row: selected.y,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(_),
            Some(_),
        )))
    ));
}

// ── Resize-tick activation-branch timing (unrelated to the mounted-owner
// shape; ported verbatim from the orphaned cursor tests) ──────────────────

/// `activate_album_folder_row` gates on `App::is_right_panel_wide`, a
/// paint-free predicate driven solely by terminal size. Resizing narrow ->
/// wide through the real `Msg::TerminalEvent(Resize)` path must flip the
/// activation branch on that same tick, before any repaint refreshes the
/// rendered panel geometry.
#[test]
fn music_album_folder_activation_branch_flips_on_resize_tick_before_repaint() {
    let mut model = Model::new(make_music_group_app());
    model.app.terminal_width = 60;
    model.app.terminal_height = 24;
    let album = make_item("First Album", "MusicAlbum");
    assert!(!model.app.is_right_panel_wide());

    model.app.panel_focus = PanelFocus::Library;
    model.sync_mounted_surfaces();
    let message = model.test_music_owner_mut().on_key(&KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    });
    let (mut music_resize, mut tv_resize) = (false, false);
    model.handle_terminal_message(
        message.expect("narrow album activation request"),
        &mut music_resize,
        &mut tv_resize,
    );
    let panel = model
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel");
    assert!(panel.test_hero_overlay_open());

    let (mut music_resize, mut tv_resize) = (false, false);
    model.handle_terminal_message(
        Msg::TerminalEvent(crate::app::components::TerminalObserverEvent::Resize {
            width: 160,
            height: 40,
        }),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(model.app.terminal_width, 160);
    assert!(model.app.is_right_panel_wide());

    model.app.activate_album_folder_row(Some(album));
    assert!(
        model.app.pending_overlay.is_none(),
        "wide activation right after the resize tick must not open a constituent modal"
    );
}
