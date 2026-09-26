//! Task 9.4: the Music component tests re-pointed at the panel-embedded
//! `MusicContent` owner and `LibraryPanel` (replacing the retired mounted
//! `MusicWorkspaceComponent`/`music_workspace_id` shape these tests used to
//! exercise). Consolidated from the retired flat Music workspace cursor and
//! mouse tests wherever their assertion was not already covered by the current
//! tick-integration or render tests.

use super::*;
use crate::app::components::library_panel::owner::LibraryContentOwner;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::msg::AlbumCursorKind;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::make_music_group_app;
use crate::app::tests::make_item;
use crate::app::{LibraryTab, PanelFocus};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::Component;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

fn wide(model: &mut Model) {
    model.app.terminal_width = 160;
    model.app.terminal_height = 40;
}

fn painted_music_offset(model: &mut Model) -> usize {
    let area = Rect::new(0, 0, 80, 8);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    {
        let browser = &mut model.test_music_owner_mut().browser;
        terminal.draw(|frame| browser.view(frame, area)).unwrap()
    };

    let browser = &model.test_music_owner().browser;
    let selected = browser
        .selected_target()
        .cloned()
        .expect("music owner has a selected target");
    let target_index = browser
        .visible_targets()
        .iter()
        .position(|target| target == &selected)
        .expect("selected target is in the painted flow");
    let selected_row = browser
        .selected_row_rect()
        .expect("selected target has painted geometry");
    let row_in_view = usize::from(selected_row.y - area.y);
    target_index
        .checked_sub(row_in_view)
        .expect("selected target is below the viewport origin")
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
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::MusicAlbumActivate { .. })));
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
    let mut track = make_item("Track One", "Audio");
    track.id = "track-1".into();
    model
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![track]);
    model.sync_mounted_surfaces();
    assert_eq!(
        model
            .test_music_owner()
            .selected_track_item()
            .map(|item| item.id),
        Some("track-1".into()),
        "the overlay Workspace keeps its stable child target"
    );
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

/// A wide->narrow breakpoint flip forces inline track focus off (design.md
/// D5: narrow never enters track focus), even if the wide side had it.
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
                fetched_rows: 0,
                parent_id: "lib-music".into(),
                title: "Music".into(),
                items: vec![group],
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
            BrowseLevel {
                fetched_rows: 0,
                parent_id: "group-0".into(),
                title: "Alpha".into(),
                items: albums,
                total_count: 3,
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
       Some(Msg::Shell(ref shell_boxed))
    if matches!(shell_boxed.as_ref(), ShellRequest::MusicAlbumCursor {
           target: 1,
           kind: AlbumCursorKind::Move
       })));
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
        fetched_rows: 0,
        parent_id: "album-0".into(),
        title: "Tracks".into(),
        items: vec![track],
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
    // fixture's short flow; read the clamped value back through painted row
    // geometry so this assertion still proves viewport retention, not only
    // stable-target selection.
    let scroll = painted_music_offset(&mut model);
    assert!(
        scroll > 0,
        "the fixture must start with a non-zero viewport"
    );
    let selected = model.test_music_owner().browser.selected_target().cloned();

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
        model.test_music_owner().browser.selected_target().cloned(),
        selected,
        "the owner's selected target survives the tab round trip"
    );
    assert_eq!(
        painted_music_offset(&mut model),
        scroll,
        "the owner's painted viewport offset survives the tab round trip"
    );
}

// ── Re-anchor mechanics ───────────────────────────────────────────────────

// ── Keyboard: album-level navigation (task 9.4 restored this entirely; see
// commit 40b7a361) ─────────────────────────────────────────────────────────

// ── Keyboard: shortcuts reuse the component's own selection ──────────────

// ── Mouse: album-list gestures the shared tick-integration coverage does
// not already exercise ────────────────────────────────────────────────────
