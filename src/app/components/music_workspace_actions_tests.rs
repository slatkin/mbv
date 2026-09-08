use super::music_workspace::MusicWorkspaceComponent;
use crate::app::components::inline_search::{InlineSearchHost, SearchPool};
use crate::app::components::msg::ShellRequest;
use crate::app::components::Msg;
use crate::app::render::{LibraryListRenderCtx, MusicWideRenderCtx};
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn context(track_cursor: Option<usize>) -> MusicWideRenderCtx {
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
        track_cursor,
    )
}

fn grouped_context(
    cursor: usize,
    order: Vec<usize>,
    track_cursor: Option<usize>,
) -> MusicWideRenderCtx {
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
        track_cursor,
    )
}

#[test]
fn music_workspace_track_targeted_actions_emit_typed_messages() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(None));
    component.set_inline_track_focus_enabled(true);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));

    let enqueue = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('a'),
        modifiers: KeyModifiers::CONTROL,
    }));
    assert!(matches!(
        enqueue,
        Some(Msg::Shell(ShellRequest::MusicTrackEnqueue))
    ));

    let menu = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('.'),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        menu,
        Some(Msg::Shell(ShellRequest::MusicTrackContextMenu))
    ));
}

#[test]
fn ctrl_s_on_album_emits_library_shuffle() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(None));

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('s'),
        modifiers: KeyModifiers::CONTROL,
    }));

    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::EmbyLibraryShuffle { item }))
            if item.id == "id" && item.item_type == "MusicAlbum"
    ));
}

#[test]
fn ctrl_s_with_track_focus_does_not_shuffle() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(None));
    component.set_inline_track_focus_enabled(true);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('s'),
        modifiers: KeyModifiers::CONTROL,
    }));

    assert_eq!(message, None);
    assert_eq!(component.track_cursor(), Some(0));
}

#[test]
fn dot_on_album_emits_library_context_menu() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(None));
    assert!(matches!(
        component.on(&Event::Keyboard(KeyEvent { code: Key::Char('.'), modifiers: KeyModifiers::NONE })),
        Some(Msg::Shell(ShellRequest::EmbyLibraryContextMenu { item }))
            if item.name == "First Album"
    ));
}

#[test]
fn dot_with_track_focus_emits_track_context_menu() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(None));
    component.set_inline_track_focus_enabled(true);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Char('.'),
            modifiers: KeyModifiers::NONE
        })),
        Some(Msg::Shell(ShellRequest::MusicTrackContextMenu))
    );
}

#[test]
fn slash_on_album_emits_open_inline_search() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(None));
    assert_eq!(
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::NONE
        })),
        Some(Msg::Shell(ShellRequest::OpenInlineSearch))
    );
}

#[test]
fn slash_with_track_focus_is_unclaimed() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(context(None));
    component.set_inline_track_focus_enabled(true);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::NONE
        })),
        None
    );
}

#[test]
fn dot_empty_list_is_unclaimed() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(Vec::new(), 0, 0),
        None,
        "Artist".into(),
        Vec::new(),
        0,
        Vec::new(),
        Vec::new(),
        true,
        None,
        false,
        None,
    ));
    assert_eq!(
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Char('.'),
            modifiers: KeyModifiers::NONE
        })),
        None
    );
}

#[test]
fn ctrl_p_empty_list_is_unclaimed() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(Vec::new(), 0, 0),
        None,
        "Artist".into(),
        Vec::new(),
        0,
        Vec::new(),
        Vec::new(),
        true,
        None,
        false,
        None,
    ));

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('p'),
        modifiers: KeyModifiers::CONTROL,
    }));

    assert_eq!(message, None);
}

/// Wide Inline Search suppresses the grouped album rail's artist headers
/// (design.md D3) and paints flat scored results instead, while the Hero
/// pane painted alongside it remains visible.
#[test]
fn music_workspace_wide_search_hides_grouped_rows_and_paints_flat_results() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    // `grouped_context` groups all four albums under a single "Artist"
    // heading; the ordinary wide rail would paint that heading above them.
    component.set_content(grouped_context(0, vec![0, 1, 2, 3], None));

    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(component.inline_search().is_active());
    component
        .inline_search_mut()
        .set_pool(SearchPool::Items(vec![make_item(
            "Zeta Album Match",
            "MusicAlbum",
        )]));

    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();

    let list_area = component.inline_search().layout().left_area;
    assert!(list_area.width > 0 && list_area.height > 0);
    assert!(
        list_area.x > 0,
        "search paints in the browser pane, not over the Hero pane: {list_area:?}"
    );

    let buffer = terminal.backend().buffer();
    let mut found_result = false;
    let mut found_heading = false;
    for y in list_area.y..list_area.y + list_area.height {
        let row: String = (list_area.x..list_area.x + list_area.width)
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect();
        if row.contains("Zeta Album Match") {
            found_result = true;
        }
        if row.contains("Artist") {
            found_heading = true;
        }
    }
    assert!(
        found_result,
        "flat search result painted in the browser pane"
    );
    assert!(
        !found_heading,
        "no artist heading painted in the search list area"
    );
    assert!(
        component.layout().selector_tabs.is_empty(),
        "group-pill hit regions are unavailable while search is active"
    );
    let frame_text: String = (0..buffer.area().height)
        .flat_map(|y| (0..buffer.area().width).map(move |x| buffer.cell((x, y)).unwrap().symbol()))
        .collect();
    assert!(frame_text.contains("SEARCH:"));
    assert!(
        !frame_text.contains("┌"),
        "bordered search input is not painted"
    );

    // The Hero pane (right of the browser pane) remains painted.
    let mut hero_painted = false;
    for y in 0..30 {
        let row: String = (list_area.x + list_area.width..120)
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect();
        if row.contains("Album 0") {
            hero_painted = true;
        }
    }
    assert!(hero_painted, "Hero pane remains visible during search");
}

/// A right click on an Inline Search result row moves the search cursor there
/// and asks the host to open its ordinary item-based context menu for that
/// result (P1: context-menu actions stay available while search is open).
#[test]
fn music_workspace_search_right_click_on_result_opens_context_menu() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(grouped_context(0, vec![0, 1, 2, 3], None));

    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    }));
    component
        .inline_search_mut()
        .set_pool(SearchPool::Items(vec![make_item(
            "Zeta Album Match",
            "MusicAlbum",
        )]));

    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let list_area = component.inline_search().layout().left_area;

    let message = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: list_area.x,
        row: list_area.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(
        matches!(
            message,
            Some(Msg::Shell(ShellRequest::EmbyLibraryContextMenu { ref item }))
                if item.name == "Zeta Album Match"
        ),
        "right click on a result row opens its context menu: {message:?}"
    );
}

/// Dismissing search restores the prior album position (design.md D4):
/// Inline Search's cursor is local to the control, so the component's own
/// `album_cursor` -- never touched while search is open -- is unchanged.
#[test]
fn music_workspace_dismiss_restores_prior_album_position() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_focused(true);
    component.set_content(grouped_context(0, vec![0, 1, 2, 3], None));
    component.re_anchor(2, 0);
    assert_eq!(component.album_cursor(), 2);

    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(component.inline_search().is_active());

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Esc,
        modifiers: KeyModifiers::NONE,
    }));

    assert_eq!(
        message, None,
        "Escape dismisses locally with no shell effect"
    );
    assert!(!component.inline_search().is_active());
    assert_eq!(
        component.album_cursor(),
        2,
        "the prior album position is unchanged by the local search session"
    );
}
