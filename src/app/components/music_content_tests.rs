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
        vec![crate::app::music_grouping::ArtistKey::Fallback(
            "Artist".into(),
        )],
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
    assert_eq!(header, Some("TRACKLIST"));
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
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|frame| owner.browser.view(frame, area))
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

/// Music never tracks played/progress in its rows: a played track and a
/// half-played track both project the ordinary row (mbv never resumes a music
/// track, so its stored position means nothing either).
#[test]
fn played_tracks_project_the_ordinary_state() {
    let mut played = make_item("Finished Track", "Audio");
    played.played = true;
    let mut half_played = make_item("Half-Played Track", "Audio");
    half_played.runtime_ticks = 1000;
    half_played.playback_position_ticks = 500;
    let fresh = make_item("Fresh Track", "Audio");
    let rows = build_track_rows(&[played, half_played, fresh]);
    let states: Vec<&MediaSemanticState> = rows
        .iter()
        .map(|row| match row {
            MediaListRow::Item { semantic_state, .. } => semantic_state,
            _ => panic!("track rows are items"),
        })
        .collect();
    assert_eq!(states[0], &MediaSemanticState::Ordinary);
    assert_eq!(states[1], &MediaSemanticState::Ordinary);
    assert_eq!(states[2], &MediaSemanticState::Ordinary);
}

// ── Task 2.4: tree chord mapping through the component boundary ──────────

/// A multi-artist Grouped Music owner whose tree starts on the first album of
/// the first root (`selected_album` drives the initial adoption). `artists` is
/// `(artist, [album target…])` in settled order; the derived title is the
/// target so assertions can name leaves.
fn tree_owner(artists: &[(&str, &[&str])]) -> MusicContent {
    let mut items: Vec<EmbyItem> = Vec::new();
    let mut album_info: Vec<(String, String, String)> = Vec::new();
    let mut artist_keys: Vec<crate::app::music_grouping::ArtistKey> = Vec::new();
    for (artist, targets) in artists {
        for target in *targets {
            let mut album = make_item(target, "MusicAlbum");
            album.id = (*target).to_string();
            album.artist = (*artist).to_string();
            items.push(album);
            album_info.push((
                (*artist).to_string(),
                "2001".to_string(),
                (*target).to_string(),
            ));
            artist_keys.push(crate::app::music_grouping::ArtistKey::Service(format!(
                "artist-{artist}"
            )));
        }
    }
    let selected = items.first().cloned();
    let order: Vec<usize> = (0..items.len()).collect();
    let ctx = MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(items, 0),
        selected,
        String::new(),
        Vec::new(),
        0,
        album_info,
        artist_keys,
        order,
        None,
    );
    let mut owner = MusicContent::new();
    owner.set_content(ctx);
    owner
}

fn press(owner: &mut MusicContent, code: Key) -> Option<Msg> {
    owner.on_key(&KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

fn selected_row(owner: &MusicContent) -> usize {
    let id = owner.browser.selected_id().expect("a node is selected");
    owner
        .browser
        .projected_nodes()
        .iter()
        .position(|node| node.id() == id)
        .expect("the selected node is projected")
}

#[test]
fn home_end_and_page_move_over_the_tree_visible_nodes() {
    let mut owner = tree_owner(&[
        ("Alpha", &["a-0", "a-1", "a-2"]),
        ("Beta", &["b-0", "b-1", "b-2"]),
    ]);
    owner.expand_all_tree_roots();

    press(&mut owner, Key::Home);
    let first_visible = owner.browser.selected_id();
    assert!(
        owner.browser.selected_is_artist(),
        "Home lands on the first visible node (the Alpha root), not a Heading group"
    );
    press(&mut owner, Key::End);
    assert_eq!(
        owner.browser.selected_album_target(),
        Some("b-2"),
        "End lands on the last visible album"
    );
    press(&mut owner, Key::Home);
    assert_eq!(owner.browser.selected_id(), first_visible);

    // The page stride is the shared visible-node stride (5): it must never
    // inherit a Heading-based group jump.
    press(&mut owner, Key::PageDown);
    assert_eq!(selected_row(&owner), 5, "PageDown moves one page of rows");
    press(&mut owner, Key::PageUp);
    assert_eq!(selected_row(&owner), 0, "PageUp mirrors the page stride");
}

#[test]
fn up_and_down_move_across_artist_roots_and_album_leaves() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    owner.expand_all_tree_roots();
    press(&mut owner, Key::Home);
    let root = owner.browser.selected_id().expect("artist root selected");
    assert!(owner.browser.root_is_expanded(root));

    // Down crosses from the root to its first leaf, then on to the next root;
    // both are visible tree nodes.
    press(&mut owner, Key::Down);
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));
    press(&mut owner, Key::Down);
    assert_eq!(owner.browser.selected_album_target(), Some("a-1"));
    press(&mut owner, Key::Down);
    assert!(
        owner.browser.selected_is_artist(),
        "the next visible node is Beta's artist root"
    );
    press(&mut owner, Key::Up);
    assert_eq!(owner.browser.selected_album_target(), Some("a-1"));
    // `j`/`k` alias the plain arrows.
    press(&mut owner, Key::Char('k'));
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));
    press(&mut owner, Key::Char('j'));
    assert_eq!(owner.browser.selected_album_target(), Some("a-1"));
}

#[test]
fn enter_toggles_an_artist_root_and_activates_an_album_leaf() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    press(&mut owner, Key::Home);
    let root = owner.browser.selected_id().expect("artist root selected");
    // The initial album adoption expanded the first root's path.
    assert!(owner.browser.root_is_expanded(root));

    assert_eq!(
        press(&mut owner, Key::Enter),
        None,
        "root toggle emits no request"
    );
    assert!(!owner.browser.root_is_expanded(root), "Enter collapses it");
    press(&mut owner, Key::Enter);
    assert!(
        owner.browser.root_is_expanded(root),
        "Enter expands it again"
    );

    // The existing album activation is preserved on a leaf.
    press(&mut owner, Key::Down);
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));
    match press(&mut owner, Key::Enter) {
        Some(Msg::Shell(ShellRequest::MusicAlbumActivate { item })) => {
            assert_eq!(item.id, "a-0")
        }
        other => panic!("expected album activation, got {other:?}"),
    }
}

#[test]
fn left_collapses_an_expanded_root_and_returns_a_leaf_to_its_parent() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    press(&mut owner, Key::Home);
    let root = owner.browser.selected_id().expect("artist root selected");
    // The initial album adoption expanded the first root's path.
    assert!(owner.browser.root_is_expanded(root));
    press(&mut owner, Key::Down);
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));

    // Left on a leaf returns to its artist parent without collapsing it and
    // without emitting an album-cursor request (an artist never overwrites
    // album persistence).
    assert_eq!(press(&mut owner, Key::Left), None);
    assert_eq!(owner.browser.selected_id(), Some(root));
    assert!(
        owner.browser.root_is_expanded(root),
        "moving to the parent must not collapse it"
    );

    // Left on the expanded root collapses it in place.
    assert_eq!(press(&mut owner, Key::Left), None);
    assert!(!owner.browser.root_is_expanded(root));
    assert_eq!(owner.browser.selected_id(), Some(root));
}

#[test]
fn right_expands_a_collapsed_artist_root_only() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"]), ("Beta", &["b-0"])]);
    // Home selects Alpha's (already expanded) root; Down twice reaches Beta's
    // collapsed root.
    press(&mut owner, Key::Home);
    press(&mut owner, Key::Down);
    press(&mut owner, Key::Down);
    let root = owner.browser.selected_id().expect("artist root selected");
    assert!(owner.browser.selected_is_artist());
    assert!(!owner.browser.root_is_expanded(root));

    assert_eq!(
        press(&mut owner, Key::Right),
        None,
        "expand emits no request"
    );
    assert!(
        owner.browser.root_is_expanded(root),
        "Right expands the root"
    );

    // Right on an already expanded root is the artist-Workspace entry
    // (task 6.4): it must stay a local no-op here, not a second expansion or
    // a destructive collapse.
    assert_eq!(press(&mut owner, Key::Right), None);
    assert!(owner.browser.root_is_expanded(root));
    assert_eq!(owner.browser.selected_id(), Some(root));
}

/// Carry-over correction from task 2.3: the persisted flat-flow offset is a
/// row in the settled flow (artist row plus leaves), while the tree's
/// projection interleaves artist roots with visible leaves. Applying the
/// persisted value as a raw projection row can anchor the viewport to the
/// wrong album; the restored album must land visible regardless of how many
/// roots precede it.
#[test]
fn restored_album_selection_lands_visible_behind_many_artist_roots() {
    // 12 artists × 3 albums. Build the corpus with stable, nameable targets.
    let mut items: Vec<EmbyItem> = Vec::new();
    let mut album_info: Vec<(String, String, String)> = Vec::new();
    let mut artist_keys: Vec<crate::app::music_grouping::ArtistKey> = Vec::new();
    for artist in 0..12 {
        for album in 0..3 {
            let target = format!("a{artist}-{album}");
            let mut item = make_item(&target, "MusicAlbum");
            item.id = target.clone();
            item.artist = format!("Artist {artist:02}");
            items.push(item);
            album_info.push((format!("Artist {artist:02}"), "2001".into(), target.clone()));
            artist_keys.push(crate::app::music_grouping::ArtistKey::Service(format!(
                "artist-{artist}"
            )));
        }
    }
    let selected = items.first().cloned();
    let order: Vec<usize> = (0..items.len()).collect();
    let mut owner = MusicContent::new();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(items, 0),
        selected,
        String::new(),
        Vec::new(),
        0,
        album_info,
        artist_keys,
        order,
        None,
    ));

    // The initial adoption expanded artist 0's path. Restore artist 5's first
    // album (album index 15) with a persisted flat-flow offset of 17: that
    // offset names `a4-0`, but artist 4 stays collapsed, so the tree must round
    // the anchor forward to the next visible node instead of applying 17 as a
    // projection row.
    owner.re_anchor(15, 17);
    let area = Rect::new(0, 0, 40, 5);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| owner.browser.view(frame, area))
        .unwrap();

    assert_eq!(owner.browser.selected_album_target(), Some("a5-0"));
    let row = selected_row(&owner);
    let offset = owner.browser.offset();
    assert!(
        row >= offset && row < offset + 5,
        "restored album row {row} outside the viewport {offset}..{}",
        offset + 5
    );
    assert_eq!(
        row, 9,
        "eleven artist roots precede the restored album's root"
    );
    assert_eq!(
        offset, 8,
        "the raw persisted offset (17) must not anchor the tree; the hidden \
         a4 leaf rounds forward to artist 5's root"
    );
}
