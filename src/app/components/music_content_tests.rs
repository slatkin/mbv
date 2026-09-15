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
        LibraryListRenderCtx::from_items(vec![album.clone()], 0, 0),
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
    assert_eq!(header, Some("Tracks"));
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
fn album_wheel_emits_cursor_for_owner_target_and_noop_for_unknown_target() {
    let mut owner = MusicContent::new();
    owner.set_content(context(make_item("Album", "MusicAlbum"), "overview"));

    let area = Rect::new(0, 0, 30, 1);
    owner.carrier.inline_mut().set_geometry(area, area);
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|frame| owner.carrier.inline_mut().view(frame, area))
        .unwrap();

    let event = LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at: Position { x: 0, y: 0 },
        delta: 1,
    });
    assert!(matches!(
        owner.on_slot_event(event),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: 0,
            kind: AlbumCursorKind::Move,
        }))
    ));

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
