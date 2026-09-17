use super::*;
use crate::app::render::LibraryListRenderCtx;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use tuirealm::component::Component;

fn context(album: EmbyItem, overview: &str) -> MusicWideRenderCtx {
    let mut album = album;
    album.overview = overview.into();
    MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![album.clone()], 0),
        Some(album),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        vec![("Artist".into(), "2024".into(), "Album".into())],
        vec![0],
        None,
    )
}

#[test]
fn content_uses_square_artwork_for_an_album() {
    let mut owner = MusicContent::new();
    owner.set_content(context(make_item("Album", "MusicAlbum"), "overview"));
    let content = owner.content();
    assert_eq!(
        content.hero.unwrap().facts.artwork.shape,
        super::super::library_panel::ArtworkShape::Square
    );
}

#[test]
fn content_uses_square_artwork_for_a_folder_album_row() {
    // Folder-view music libraries list album rows as Emby `Folder` items,
    // which the type-based artwork policy cannot recognise as music; the
    // owner must still resolve the album arm (Square, `{id}:P` album
    // chain) or the hero paints a placeholder for every album.
    let mut owner = MusicContent::new();
    owner.set_content(context(make_item("Album", "Folder"), "overview"));
    let content = owner.content();
    let artwork = &content.hero.unwrap().facts.artwork;
    assert_eq!(
        artwork.shape,
        super::super::library_panel::ArtworkShape::Square
    );
    match artwork.source.as_ref() {
        Some(super::super::library_panel::content::ArtworkSource::Emby { cache_key, .. }) => {
            assert_eq!(cache_key, "id:P");
        }
        other => panic!("expected the album art source, got {other:?}"),
    }
}

#[test]
fn content_omits_an_absent_overview() {
    let mut owner = MusicContent::new();
    owner.set_content(context(make_item("Album", "MusicAlbum"), ""));
    let content = owner.content();
    assert!(content.hero.unwrap().overview.is_none());
}

#[test]
fn content_exposes_tracks_as_the_workspace() {
    let mut owner = MusicContent::new();
    let mut ctx = context(make_item("Album", "MusicAlbum"), "overview");
    ctx.album_tracks = Some(vec![make_item("Track", "Audio")]);
    owner.set_content(ctx);
    let has_workspace = owner
        .content()
        .hero
        .as_ref()
        .is_some_and(|hero| hero.workspace.is_some());
    assert!(has_workspace);
    // Grouped Music labels its Workspace with the `Tracks` header.
    let header = owner
        .content()
        .hero
        .as_ref()
        .and_then(|hero| hero.workspace.as_ref())
        .and_then(|workspace| workspace.header);
    assert_eq!(header, Some("Tracklist"));
    assert_eq!(owner.track_list.rows().len(), 1);
}

#[test]
fn hero_double_click_activates_the_selected_track() {
    let album = make_item("Album", "MusicAlbum");
    let track = make_item("Track", "Audio");
    let mut owner = MusicContent::new();
    let mut ctx = context(album.clone(), "overview");
    ctx.album_tracks = Some(vec![track.clone()]);
    owner.set_content(ctx);

    let area = Rect::new(0, 0, 30, 1);
    owner.track_list.wide_mut().set_geometry(area, area);
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|frame| owner.track_list.wide_mut().view(frame, area))
        .unwrap();

    let message = owner.on_slot_event(LibrarySlotEvent::HeroPane(
        MediaListSurfaceInput::DoubleClick(Position { x: 0, y: 0 }),
    ));
    match message {
        Some(Msg::Shell(ShellRequest::MusicTrackActivate {
            album_id,
            track: activated,
        })) => {
            assert_eq!(album_id, album.id);
            assert_eq!(activated.id, track.id);
        }
        other => panic!("expected track activation, got {other:?}"),
    }
}

#[test]
fn album_wheel_steps_the_viewport_and_requests_the_cursor_only_when_dragged() {
    let albums: Vec<EmbyItem> = (0..8)
        .map(|i| {
            let mut album = make_item(&format!("Album {i}"), "MusicAlbum");
            album.id = format!("album-{i}");
            album
        })
        .collect();
    let selected = albums[0].clone();
    let mut owner = MusicContent::new();
    let album_info: Vec<(String, String, String)> = (0..8)
        .map(|i| ("Artist".to_string(), String::new(), format!("Album {i}")))
        .collect();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(albums, 0),
        Some(selected),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        album_info,
        (0..8).collect(),
        None,
    ));

    let area = Rect::new(0, 0, 30, 3);
    owner.carrier.wide_mut().set_geometry(area, area);
    let mut terminal = Terminal::new(TestBackend::new(30, 3)).unwrap();
    terminal
        .draw(|frame| owner.carrier.wide_mut().view(frame, area))
        .unwrap();

    let event = LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at: Position { x: 0, y: 0 },
        delta: 1,
    });
    // The rows are the artist's `Heading` plus eight albums, so "album-2" is
    // display row 3. The window displays [1, 4) around it, so the wheel
    // steps the viewport alone: no album cursor request is emitted (design
    // D8) and the framework-visible claim survives (ADR 0024).
    owner.carrier.select_target(&"album-2".to_string());
    assert!(matches!(
        owner.on_slot_event(event),
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert_eq!(owner.carrier.scroll(), 2);
    assert_eq!(
        owner.carrier.selected_target(),
        Some(&"album-2".to_string())
    );

    // The selection now rides the window's top edge — still inside, so the
    // next step is still a viewport gesture.
    assert!(matches!(
        owner.on_slot_event(event),
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert_eq!(owner.carrier.scroll(), 3);
    // Stepping past it drags the selection with the window: the album cursor
    // request fires only then, resolved against the dragged owner target.
    let dragged = owner.on_slot_event(event);
    assert_eq!(owner.carrier.scroll(), 4);
    assert!(matches!(
        dragged,
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: 3,
            kind: AlbumCursorKind::Move,
        }))
    ));

    // A dragged step whose target the context cannot resolve reports
    // nothing.
    owner.context.album_targets.clear();
    assert_eq!(owner.on_slot_event(event), None);
}

#[test]
fn wide_album_metadata_removes_artist_and_year_prefix() {
    // The old `wide_album_metadata` characterization (rehomed here by task
    // 9.2): a tagged album whose display name still carries the
    // `Artist (Year) Title` folder prefix must present the bare title and
    // the parsed release year, even though `derive_album_display_name`
    // leaves a tagged album's name untouched.
    let mut album = make_item("Bob Dylan (1970) New Morning", "MusicAlbum");
    album.artist = "Bob Dylan".into();
    album.production_year = 1970;

    assert_eq!(
        wide_album_metadata(&album, "Bob Dylan"),
        ("New Morning".to_string(), 1970)
    );
}

#[test]
fn resolved_hero_data_uses_parsed_title_year_and_cached_artist() {
    // The `album_artist_cache` fallback names the artist; the folder-name
    // parse supplies the title/year the Wide hero presents.
    let mut owner = MusicContent::new();
    let mut album = make_item("Folder Artist (2024) First Album", "MusicAlbum");
    album.artist.clear();
    album.production_year = 0;
    let mut ctx = context(album, "overview");
    ctx.album_info = vec![("Folder Artist".into(), "2024".into(), "First Album".into())];
    owner.set_content(ctx);

    let data = owner.hero_data().expect("hero data");
    assert_eq!(data.facts.title, "First Album");
    assert_eq!(data.facts.meta_rows, vec!["Folder Artist", "2024"]);
}

/// The album rail's keyboard viewport chords (task 6.1, design D6/D7):
/// `Ctrl+y`/`Ctrl+e` step the window one display row, `PgDn` pages it by the
/// painted height, and the album-cursor echo reports an actual selection
/// move only (design D8) — a window-only step emits no cursor request but is
/// still claimed, so the panel takes the deferred resting-scroll position
/// report instead of letting the chord fall through to the shell.
#[test]
fn album_viewport_chords_step_the_rail_and_report_only_drags() {
    let albums: Vec<EmbyItem> = (0..8)
        .map(|i| {
            let mut album = make_item(&format!("Album {i}"), "MusicAlbum");
            album.id = format!("album-{i}");
            album
        })
        .collect();
    let selected = albums[0].clone();
    let mut owner = MusicContent::new();
    let album_info: Vec<(String, String, String)> = (0..8)
        .map(|i| ("Artist".to_string(), String::new(), format!("Album {i}")))
        .collect();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(albums, 0),
        Some(selected),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        album_info,
        (0..8).collect(),
        None,
    ));

    // A 3-row painted frame over a `Heading` plus eight albums: "album-2" is
    // display row 3 and the resolved window shows [1, 4).
    let area = Rect::new(0, 0, 30, 3);
    owner.carrier.wide_mut().set_geometry(area, area);
    let mut terminal = Terminal::new(TestBackend::new(30, 3)).unwrap();
    terminal
        .draw(|frame| owner.carrier.wide_mut().view(frame, area))
        .unwrap();
    owner.carrier.select_target(&"album-2".to_string());

    // Ctrl+y steps the window down onto the selection's neighbourhood; the
    // selection stays inside, so no album cursor is reported — but the
    // consumed step is claimed (the position-report path).
    let message = owner.on_key(&KeyEvent {
        code: Key::Char('y'),
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(
        message,
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed)),
        "a window-only chord is claimed without a cursor request"
    );
    assert_eq!(owner.carrier.scroll(), 2);
    assert_eq!(
        owner.carrier.selected_target(),
        Some(&"album-2".to_string())
    );

    // Ctrl+e steps the window back; still a viewport gesture, still
    // claimed with no cursor request.
    let message = owner.on_key(&KeyEvent {
        code: Key::Char('e'),
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(
        message,
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
    );
    assert_eq!(owner.carrier.scroll(), 1);

    // PgDn pages the window a full painted height; the left-behind
    // selection is dragged to the paged window's top and the page-kind
    // album cursor reports the drag (design D8).
    let message = owner.on_key(&KeyEvent {
        code: Key::PageDown,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(owner.carrier.scroll(), 4);
    match message {
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target, kind })) => {
            assert_eq!(target, 3);
            assert_eq!(kind, AlbumCursorKind::Page);
        }
        other => panic!("expected the dragged page's album cursor, got {other:?}"),
    }
}

/// The focused track list's keyboard viewport chords (task 6.1, design
/// D6/D7) at the track pane's own painted height: the window steps/pages
/// and the selection rides only at the window's edge; the track list keeps
/// no shell echo, so a step reports nothing.
#[test]
fn track_viewport_chords_step_the_focused_track_list() {
    let tracks: Vec<EmbyItem> = (0..30)
        .map(|i| {
            let mut track = make_item(&format!("Track {i}"), "Audio");
            track.id = format!("track-{i}");
            track
        })
        .collect();
    let mut owner = MusicContent::new();
    let mut ctx = context(make_item("Album", "MusicAlbum"), "overview");
    ctx.album_tracks = Some(tracks);
    owner.set_content(ctx);
    owner.enter_track_focus();

    let area = Rect::new(0, 0, 20, 4);
    owner.track_list.wide_mut().set_geometry(area, area);
    let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
    terminal
        .draw(|frame| owner.track_list.wide_mut().view(frame, area))
        .unwrap();

    // Ctrl+y steps the window; the top-edge selection is dragged with it.
    let message = owner.on_key(&KeyEvent {
        code: Key::Char('y'),
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(message, None, "the track list reports no step echo");
    assert_eq!(owner.track_list.scroll(), 1);
    assert_eq!(owner.track_list.cursor(), 1);

    // Ctrl+e steps the window back over a now mid-window selection.
    let message = owner.on_key(&KeyEvent {
        code: Key::Char('e'),
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(message, None);
    assert_eq!(owner.track_list.scroll(), 0);
    assert_eq!(owner.track_list.cursor(), 1);

    // PgDn pages the window a full painted height and drags the
    // left-behind selection to the paged window's top.
    let message = owner.on_key(&KeyEvent {
        code: Key::PageDown,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(message, None);
    assert_eq!(owner.track_list.scroll(), 4);
    assert_eq!(owner.track_list.cursor(), 4);
}
