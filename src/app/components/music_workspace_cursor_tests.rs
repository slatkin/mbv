use super::music_workspace::MusicWorkspaceComponent;
use crate::app::components::msg::{AlbumCursorKind, ShellRequest};
use crate::app::components::Msg;
use crate::app::palette::{surface_colors_for_column_focus, Surface};
use crate::app::render::{LibraryListRenderCtx, MusicWideRenderCtx};

fn surface_fill(surface: Surface, focused: bool) -> ratatui::style::Color {
    surface_colors_for_column_focus(surface, focused).fill
}
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

fn context(track_focused: bool) -> MusicWideRenderCtx {
    let album = make_item("First Album", "MusicAlbum");
    let mut track = make_item("Track One", "Audio");
    track.index_number = 1;
    let mut second_track = make_item("Track Two", "Audio");
    second_track.index_number = 2;
    MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![album.clone()], 0, 0),
        Some(album),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        vec![("Artist".into(), "2024".into(), "First Album".into())],
        vec![0],
        true,
        Some(vec![track, second_track]),
        false,
        track_focused,
    )
}

fn grouped_context(cursor: usize, order: Vec<usize>, track_focused: bool) -> MusicWideRenderCtx {
    let albums: Vec<_> = (0..4)
        .map(|index| make_item(&format!("Album {index}"), "MusicAlbum"))
        .collect();
    MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(albums.clone(), cursor, 0),
        Some(albums[cursor].clone()),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        (0..4)
            .map(|index| ("Artist".into(), "2024".into(), format!("Album {index}")))
            .collect(),
        order,
        true,
        None,
        false,
        track_focused,
    )
}

#[test]
fn music_workspace_keeps_track_focus_local_between_syncs() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    component.set_inline_track_focus_enabled(true);
    // Enter inline track focus locally, then move within it.
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    // An ordinary content push (same album) never touches the local track
    // cursor.
    component.set_content(context(false));
    assert!(component.track_focused());
    assert_eq!(component.track_selected_row(), Some(1));
}

#[test]
fn music_workspace_vertical_move_follows_album_display_order() {
    let albums = vec![
        make_item("Album 0", "MusicAlbum"),
        make_item("Album 1", "MusicAlbum"),
        make_item("Album 2", "MusicAlbum"),
        make_item("Album 3", "MusicAlbum"),
    ];
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(albums.clone(), 2, 0),
        Some(albums[2].clone()),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        vec![
            ("Artist".into(), "2024".into(), "Album 0".into()),
            ("Artist".into(), "2023".into(), "Album 1".into()),
            ("Artist".into(), "2022".into(), "Album 2".into()),
            ("Artist".into(), "2021".into(), "Album 3".into()),
        ],
        vec![2, 0, 3, 1],
        true,
        None,
        false,
        false,
    ));
    // The shell re-anchors the album cursor at the navigation event; an
    // ordinary push no longer carries it.
    component.re_anchor(2, 0);
    component.set_album_columns(1);
    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.album_cursor(), 0);
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: 0,
            kind: AlbumCursorKind::Move,
        }))
    ));
}

#[test]
fn music_workspace_wheel_moves_one_painted_album_row_and_reuses_cursor_request() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(grouped_context(1, vec![2, 0, 3, 1], false));
    component.re_anchor(1, 0);
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let area = component.layout().wide_music_browser_area;
    let message = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: 1,
            kind: AlbumCursorKind::Move,
        }))
    ));
    assert_eq!(component.album_cursor(), 1);
}

#[test]
fn music_workspace_narrow_enter_requests_album_activation() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(!component.track_focused());
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumActivate { .. }))
    ));
}

#[test]
fn music_workspace_enter_sets_track_focus_when_inline_track_focus_enabled() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    component.set_inline_track_focus_enabled(true);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(component.track_focused());
    assert_eq!(component.track_selected_row(), Some(0));
    component.set_inline_track_focus_enabled(false);
    assert!(!component.track_focused());
}

#[test]
fn music_workspace_selection_follows_shared_hero_gate_boundaries() {
    for (width, height, wide) in [(81, 7, false), (82, 7, true), (82, 6, false)] {
        let mut component = MusicWorkspaceComponent::new();
        component.set_focused(true);
        component.set_content(context(true));
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| component.view(frame, Rect::new(0, 0, width, height)))
            .unwrap();
        assert_eq!(
            component.layout().wide_music_right_area.width > 0
                && component.layout().wide_music_right_area.height > 0,
            wide,
            "component layout branch at {width}x{height}"
        );
    }
}

#[test]
fn music_workspace_renders_without_app() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert!(terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .any(|cell| cell.symbol() == "F"));
}

#[test]
fn music_workspace_track_selection_uses_the_shared_focused_row_surface() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    component.set_inline_track_focus_enabled(true);
    component.enter_track_focus();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();

    assert!(component.track_focused());
    assert_eq!(component.track_selected_row(), Some(0));
    assert_eq!(component.track_selected_row(), Some(0));
    let track_panel = component
        .test_track_selected_row_rect()
        .expect("selected track row geometry retained");
    let buffer = terminal.backend().buffer();
    for x in [track_panel.x, track_panel.right().saturating_sub(1)] {
        assert_eq!(
            buffer[(x, track_panel.y)].bg,
            surface_fill(Surface::SelectedRowOnLibraryPane, true),
            "focused track selection reaches track panel edge {x}"
        );
    }
}

#[test]
fn music_workspace_focused_track_box_uses_the_soft_surface_role() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    component.set_inline_track_focus_enabled(true);
    component.enter_track_focus();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();

    let track_content = component
        .test_track_content_rect()
        .expect("track content geometry retained");
    assert!(
        track_content.height > 1,
        "fixture needs an ordinary track row"
    );
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(track_content.x, track_content.y + 1)].bg,
        surface_fill(Surface::MainContentBox, true),
        "focused track-list box uses the semantic soft surface role"
    );
}

/// `unify-surface-colour` 3.2: wide Music's panel fills follow the library
/// column's focus, not the track cursor. Moving the cursor from the album
/// rail into the track list changes no fill; the selected-row highlight stays
/// with the cursor.
#[test]
fn music_workspace_panel_fills_follow_the_library_column_not_the_track_cursor() {
    use crate::app::render::arrangements::library::wide_library_panes;
    use crate::app::render::arrangements::wide_hero::{PANE_PAD_X, PANE_PAD_Y};

    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    component.set_inline_track_focus_enabled(true);
    let area = Rect::new(0, 0, 100, 30);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|frame| component.view(frame, area)).unwrap();

    // Surfaces pinned: the wide Music browser pane's hero fill, the album
    // rail panel body, and the track-list content box.
    let hero_panel = wide_library_panes(area, 0, PANE_PAD_Y, None)
        .expect("wide fits")
        .hero_panel;
    let rail = component.layout().wide_music_browser_area;
    let rail_panel = (
        rail.x.saturating_sub(PANE_PAD_X),
        rail.y.saturating_sub(PANE_PAD_Y),
    );
    let track_content = component
        .test_track_content_rect()
        .expect("track content geometry retained");
    let buffer = terminal.backend().buffer();
    let hero_fill = buffer[(hero_panel.x, hero_panel.y)].bg;
    let rail_fill = buffer[rail_panel].bg;
    let track_fill = buffer[(track_content.x, track_content.y + 1)].bg;
    assert_eq!(
        rail_fill,
        surface_fill(Surface::LibraryPanel, true),
        "album rail body follows the library column's focus"
    );
    assert_eq!(
        track_fill,
        surface_fill(Surface::MainContentBox, true),
        "track-list box follows the library column's focus"
    );

    // The rail holds the cursor: its selected album row carries the
    // punch-through surface, and the track list's selected row does not.
    let rail_selected = component
        .layout()
        .selected_item_rect
        .expect("rail selected row published");
    let track_selected = component
        .test_track_selected_row_rect()
        .expect("track selected row published");
    assert_ne!(
        buffer[(rail_selected.x + 4, rail_selected.y)].bg,
        rail_fill,
        "rail selected-row highlight while the rail holds the cursor"
    );
    assert_eq!(
        buffer[(track_selected.x, track_selected.y)].bg,
        track_fill,
        "track-list selected row is unmarked while the album rail holds the cursor"
    );

    component.enter_track_focus();
    terminal.draw(|frame| component.view(frame, area)).unwrap();
    let track_content = component
        .test_track_content_rect()
        .expect("track content geometry retained");
    let track_selected = component
        .test_track_selected_row_rect()
        .expect("track selected row published");
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[rail_panel].bg, rail_fill,
        "rail body fill is unchanged when the cursor moves into the track list"
    );
    assert_eq!(
        buffer[(track_content.x, track_content.y + 1)].bg,
        track_fill,
        "track-list box fill is unchanged when the cursor moves into it"
    );
    assert_eq!(
        buffer[(hero_panel.x, hero_panel.y)].bg,
        hero_fill,
        "hero pane fill is unchanged when the cursor moves into the track list"
    );
    // The highlight followed the cursor: the rail's selected album row is now
    // indistinguishable from the body, and the track list's selected row takes
    // its owning focused surface -- observable by background, because the
    // track list paints `OwningSurface` against the soft content box.
    assert_eq!(
        buffer[(rail_selected.x + 4, rail_selected.y)].bg,
        rail_fill,
        "rail selected-row highlight is gone while the track list holds the cursor"
    );
    assert_eq!(
        buffer[(track_selected.x, track_selected.y)].bg,
        surface_fill(Surface::SelectedRowOnLibraryPane, true),
        "track-list selected row takes its owning focused surface while it holds the cursor"
    );

    // Queue column holds panel focus: both rest.
    component.set_focused(false);
    terminal.draw(|frame| component.view(frame, area)).unwrap();
    let track_content = component
        .test_track_content_rect()
        .expect("track content geometry retained");
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[rail_panel].bg,
        surface_fill(Surface::LibraryPanel, false),
        "rail body rests when the queue column holds focus"
    );
    assert_eq!(
        buffer[(track_content.x, track_content.y + 1)].bg,
        surface_fill(Surface::MainContentBox, false),
        "track-list box rests when the queue column holds focus"
    );
}

#[test]
fn music_workspace_horizontal_move_is_ignored_at_one_column() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(grouped_context(1, vec![0, 1, 2, 3], false));
    component.re_anchor(1, 0);
    component.set_album_columns(1);

    for key in [Key::Left, Key::Right, Key::Char('h'), Key::Char('l')] {
        let message = component.on(&Event::Keyboard(KeyEvent {
            code: key,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(message, None);
        assert_eq!(component.album_cursor(), 1);
    }
}

#[test]
fn music_workspace_page_moves_saturate_at_both_ends() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(grouped_context(0, vec![0, 1, 2, 3], false));
    component.set_album_columns(2);
    component.set_page_rows(2);

    for key in [Key::PageUp, Key::PageDown, Key::PageDown, Key::PageUp] {
        component.on(&Event::Keyboard(KeyEvent {
            code: key,
            modifiers: KeyModifiers::NONE,
        }));
    }
    assert_eq!(component.album_cursor(), 0);
}

#[test]
fn music_workspace_track_keys_are_consumed_locally_and_do_not_move_album_cursor() {
    // With a track focused (wide), Down moves the track cursor only: the
    // component consumes the key locally without emitting an album intent.
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    let albums: Vec<_> = (0..4)
        .map(|index| make_item(&format!("Album {index}"), "MusicAlbum"))
        .collect();
    let tracks: Vec<_> = (0..3)
        .map(|i| {
            let mut t = make_item(&format!("Track {i}"), "Audio");
            t.index_number = i + 1;
            t
        })
        .collect();
    component.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(albums.clone(), 1, 0),
        Some(albums[1].clone()),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        (0..4)
            .map(|index| ("Artist".into(), "2024".into(), format!("Album {index}")))
            .collect(),
        vec![0, 1, 2, 3],
        true,
        Some(tracks),
        false,
        false,
    ));
    component.re_anchor(1, 0);
    component.set_inline_track_focus_enabled(true);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    component.set_album_columns(2);

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(message, None);
    assert!(component.track_focused());
    assert_eq!(component.track_selected_row(), Some(1));
    assert_eq!(component.album_cursor(), 1);
}

#[test]
fn music_workspace_enter_on_focused_track_emits_activation() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    component.set_inline_track_focus_enabled(true);
    // Enter enters track mode; Enter again activates the focused track.
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::MusicTrackActivate { .. }))
    ));
}

#[test]
fn music_workspace_track_esc_exits_locally_without_forwarding() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    component.set_inline_track_focus_enabled(true);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Esc,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(!component.track_focused());
    assert_eq!(message, None);
}

#[test]
fn music_workspace_album_change_clears_track_focus() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    component.set_inline_track_focus_enabled(true);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(component.track_focused());
    assert_eq!(component.track_selected_row(), Some(0));

    // A different selected album (group switch / recursive activation)
    // resets the stale track cursor.
    let mut other = make_item("Other Album", "MusicAlbum");
    other.id = "album-2".into();
    component.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![other.clone()], 0, 0),
        Some(other),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        vec![("Artist".into(), "2024".into(), "Other Album".into())],
        vec![0],
        true,
        Some(vec![make_item("Other Track", "Audio")]),
        false,
        false,
    ));
    assert!(!component.track_focused());
}

#[test]
fn music_workspace_clear_track_focus_keeps_the_track_owner_selection() {
    // D5: the track pane's focus and the track owner's selected row are
    // separate. A focus-only clear (position restore) must not move the
    // selection or scroll the owner back to its first row.
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    component.set_inline_track_focus_enabled(true);
    component.enter_track_focus();
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(component.track_focused());
    assert_eq!(component.track_selected_row(), Some(1));

    component.clear_track_focus();

    assert!(!component.track_focused());
    assert_eq!(
        component.track_selected_row(),
        Some(1),
        "a focus-only clear leaves the track owner's selection untouched"
    );
}

#[test]
fn music_workspace_re_anchor_overrides_prior_local_move() {
    // A shell re-anchor at a navigation event adopts the shell's cursor
    // unconditionally -- the outcome does not depend on whether the user
    // moved the cursor since the previous projection.
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(grouped_context(0, vec![0, 1, 2, 3], false));
    component.re_anchor(0, 0);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert_ne!(
        component.album_cursor(),
        0,
        "local move diverged the cursor"
    );

    component.set_content(grouped_context(2, vec![0, 1, 2, 3], false));
    component.re_anchor(2, 0);
    assert_eq!(component.album_cursor(), 2);
}

#[test]
fn music_workspace_ordinary_push_leaves_album_cursor_alone() {
    // Without a re-anchor, a content push never adopts the shell cursor,
    // and the component holds no stored copy of a previously pushed value.
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(grouped_context(0, vec![0, 1, 2, 3], false));
    component.re_anchor(0, 0);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let moved = component.album_cursor();
    assert_ne!(moved, 0);

    component.set_content(grouped_context(3, vec![0, 1, 2, 3], false));
    assert_eq!(component.album_cursor(), moved);
}

#[test]
fn music_workspace_bracket_keys_request_group_switch() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(grouped_context(1, vec![0, 1, 2, 3], false));

    let prev = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('['),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        prev,
        Some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta: -1 }))
    );

    let next = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char(']'),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        next,
        Some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta: 1 }))
    );
}

#[test]
fn music_workspace_bracket_keys_ignored_with_focused_track() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(false));
    component.set_inline_track_focus_enabled(true);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(component.track_focused());
    assert_eq!(component.track_selected_row(), Some(0));

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('['),
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(message, None);
}

/// `unify-surface-colour` 4.2: the row between the pill bar and the album rail
/// is the chrome-band spacer (design D2's `PillRowGap`), so it keeps the app
/// backdrop in both focus states. The pane container itself is left to the
/// shell's `LibraryColumn`, as TV/Movies/ABS do.
#[test]
fn music_wide_pill_row_spacer_is_the_chrome_band_surface() {
    use crate::app::render::arrangements::library::wide_library_panes;
    use crate::app::render::arrangements::wide_hero::{
        wide_hero_browser_pane, PANE_PAD_X, PANE_PAD_Y,
    };

    let expected = crate::app::palette::surface_colors_for_column_focus(
        crate::app::palette::Surface::PillRowGap,
        false,
    )
    .fill;

    for focused in [true, false] {
        let mut component = MusicWorkspaceComponent::new();
        component.set_focused(focused);
        component.set_content(context(false));
        let area = Rect::new(0, 0, 100, 30);
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal.draw(|frame| component.view(frame, area)).unwrap();
        let panes = wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y, None).expect("wide fits");
        let spacer = wide_hero_browser_pane(panes.browser_panel, panes.browser_area).spacer_area;
        let buffer = terminal.backend().buffer();
        assert_eq!(
            buffer[(spacer.x, spacer.y)].bg,
            expected,
            "spacer does not follow the library column's focus (focused={focused})"
        );
    }
}
