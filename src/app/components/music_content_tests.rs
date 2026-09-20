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
fn tree_entries_ignore_played_album_state_but_keep_live_progress() {
    let mut played = make_item("Album", "Folder");
    played.played = true;
    let mut owner = MusicContent::new();
    owner.set_content(context(played, ""));
    assert_eq!(
        owner.tree_entries()[0].semantic_state,
        MediaSemanticState::Ordinary,
        "music tree album rows never inherit stored played state"
    );

    let mut active = make_item("Album", "Folder");
    active.playback_position_ticks = 500;
    active.runtime_ticks = 1000;
    owner.set_content(context(active, ""));
    assert_eq!(
        owner.tree_entries()[0].semantic_state,
        MediaSemanticState::active(Some(50)),
        "music tree retains live playback progress"
    );
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
fn artist_tracks_project_heading_rows_into_the_same_workspace_carrier() {
    use crate::app::music_artist_detail::{
        ArtistDetailProjection, ArtistSummary, ArtistTrackGroup,
    };

    let mut album = make_item("Album", "MusicAlbum");
    album.id = "album-1".into();
    let mut first = make_item("Same Title", "Audio");
    first.id = "track-1".into();
    first.album_id = album.id.clone();
    let mut second = make_item("Same Title", "Audio");
    second.id = "track-2".into();
    second.album_id = album.id.clone();
    let mut owner = MusicContent::new();
    owner.set_content(context(album, "overview"));
    // Artist rows belong to the focused artist root (task 6.4): the detail
    // projection must carry the tree-resolved target, so the fixture focuses
    // the root first and binds the projection to it.
    owner.browser.select_first_visible();
    let target = owner.artist_detail_target().expect("artist root selected");
    let detail = ArtistDetailProjection {
        target,
        summary: ArtistSummary {
            name: "Artist".into(),
            album_count: 1,
            year_start: Some(2001),
            year_end: Some(2001),
        },
        track_groups: vec![ArtistTrackGroup {
            album_id: "album-1".into(),
            album_title: "Album".into(),
            tracks: vec![first.clone(), second.clone()],
        }],
        artwork: HeroImageState::None,
        artwork_cache_key: None,
    };
    let mut ctx = owner.context.clone();
    ctx.selected_album = None;
    ctx.album_tracks = None;
    ctx.artist_detail = Some(detail);
    owner.set_content(ctx);

    assert!(matches!(
        owner.track_list.rows().first(),
        Some(MediaListRow::Heading { text }) if text == "Album"
    ));
    let items: Vec<&str> = owner
        .track_list
        .rows()
        .iter()
        .filter_map(MediaListRow::selectable_target)
        .map(String::as_str)
        .collect();
    assert_eq!(items, ["track-1", "track-2"]);
}

/// Task 6.4 (design D7): an artist root's Hero content is the shell-projected
/// summary and artwork — name, in-scope album count, year span, and the
/// stable-ID artwork source — with the projected groups as its Workspace.
#[test]
fn album_tracks_survive_an_artist_detail_push_for_another_album() {
    use crate::app::music_artist_detail::{
        ArtistDetailProjection, ArtistSummary, ArtistTrackGroup,
    };

    let mut album_track = make_item("Fetched Album Track", "Audio");
    album_track.id = "album-track".into();
    album_track.album_id = "a-0".into();
    let mut owner = tree_owner_with_tracks(
        &[("Alpha", &["a-0", "a-1"])],
        Some(vec![album_track.clone()]),
    );
    owner.browser.select_first_visible();
    let artist_target = owner.artist_detail_target().expect("artist root selected");
    let mut artist_track = make_item("Artist Detail Track", "Audio");
    artist_track.id = "artist-track".into();
    artist_track.album_id = "a-1".into();
    let mut ctx = owner.context.clone();
    ctx.selected_album = None;
    ctx.album_tracks = None;
    ctx.artist_detail = Some(ArtistDetailProjection {
        target: artist_target,
        summary: ArtistSummary {
            name: "Alpha".into(),
            album_count: 2,
            year_start: None,
            year_end: None,
        },
        track_groups: vec![ArtistTrackGroup {
            album_id: "a-1".into(),
            album_title: "a-1".into(),
            tracks: vec![artist_track],
        }],
        artwork: HeroImageState::None,
        artwork_cache_key: None,
    });

    owner.set_content(ctx);

    assert_eq!(
        owner.tree_tracks.get("a-0").map(Vec::as_slice),
        Some([album_track].as_slice()),
        "the prior per-album fetch remains projected"
    );
    assert_eq!(
        owner
            .tree_tracks
            .get("a-1")
            .map(Vec::as_slice)
            .map(|tracks| tracks[0].id.as_str()),
        Some("artist-track"),
        "artist detail tracks are merged into the existing projection"
    );
}

#[test]
fn artist_root_hero_uses_the_projected_summary_and_artwork() {
    use crate::app::components::library_panel::content::ArtworkSource;
    use crate::app::music_artist_detail::{
        ArtistDetailProjection, ArtistSummary, ArtistTrackGroup,
    };

    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    press(&mut owner, Key::Home);
    let target = owner.artist_detail_target().expect("artist root selected");
    let mut track = make_item("Track One", "Audio");
    track.id = "alpha-track-1".into();
    track.album_id = "a-0".into();
    let cache_key = "artist:1:music-library:artist-Alpha:Primary".to_string();
    let mut ctx = owner.context.clone();
    ctx.selected_album = None;
    ctx.album_tracks = None;
    ctx.artist_detail = Some(ArtistDetailProjection {
        target,
        summary: ArtistSummary {
            name: "Alpha".into(),
            album_count: 2,
            year_start: Some(2001),
            year_end: Some(2003),
        },
        track_groups: vec![ArtistTrackGroup {
            album_id: "a-0".into(),
            album_title: "a-0".into(),
            tracks: vec![track],
        }],
        artwork: HeroImageState::Loading,
        artwork_cache_key: Some(cache_key.clone()),
    });
    owner.set_content(ctx);

    {
        let content = owner.content();
        let hero = content.hero.expect("artist hero");
        assert_eq!(hero.facts.title, "Alpha");
        assert_eq!(
            hero.facts.meta_rows,
            vec!["2 albums".to_string(), "2001\u{2013}2003".to_string()]
        );
        assert_eq!(
            hero.facts.artwork.shape,
            super::super::library_panel::ArtworkShape::Square
        );
        assert_eq!(hero.facts.artwork.image, HeroImageState::Loading);
        match hero.facts.artwork.source.as_ref() {
            Some(ArtworkSource::Emby {
                item_id,
                image_types,
                cache_key: key,
                ..
            }) => {
                assert_eq!(item_id, "artist-Alpha");
                assert_eq!(image_types, &vec!["Primary".to_string()]);
                assert_eq!(key, &cache_key);
            }
            other => panic!("expected the artist artwork source, got {other:?}"),
        }
        assert!(
            hero.workspace.is_some(),
            "the artist Hero carries its Workspace"
        );
    }
    assert!(matches!(
        owner.track_list.rows().first(),
        Some(MediaListRow::Heading { text }) if text == "a-0"
    ));
}

/// Task 6.4 (design D7): a fallback artist root has no stable Service ID, so
/// its Hero keeps the summary facts with the explicit no-artwork
/// presentation (no borrowed album image) and its projected rows.
#[test]
fn fallback_artist_hero_uses_the_no_artwork_presentation() {
    use crate::app::music_artist_detail::{ArtistDetailProjection, ArtistSummary};

    let mut owner = MusicContent::new();
    owner.set_content(context(make_item("Album", "MusicAlbum"), ""));
    owner.browser.select_first_visible();
    let target = owner.artist_detail_target().expect("fallback artist root");
    assert_eq!(target.artist_id, None, "the fixture root has no Service ID");
    let mut ctx = owner.context.clone();
    ctx.selected_album = None;
    ctx.album_tracks = None;
    ctx.artist_detail = Some(ArtistDetailProjection {
        target,
        summary: ArtistSummary {
            name: "Artist".into(),
            album_count: 1,
            year_start: Some(2024),
            year_end: Some(2024),
        },
        track_groups: Vec::new(),
        artwork: HeroImageState::None,
        artwork_cache_key: None,
    });
    owner.set_content(ctx);

    let content = owner.content();
    let hero = content.hero.expect("fallback artist hero");
    assert_eq!(hero.facts.title, "Artist");
    assert_eq!(
        hero.facts.meta_rows,
        vec!["1 album".to_string(), "2024".to_string()]
    );
    assert!(
        hero.facts.artwork.source.is_none(),
        "no borrowed artwork source"
    );
    assert_eq!(hero.facts.artwork.image, HeroImageState::None);
}

/// Task 6.4: a projected artist detail that no longer belongs to the tree's
/// current root never paints its summary, artwork, or groups — the Hero is
/// absent rather than stale.
#[test]
fn a_stale_artist_detail_never_paints_under_the_new_root() {
    use crate::app::music_artist_detail::{
        ArtistDetailProjection, ArtistSummary, ArtistTrackGroup,
    };

    let mut owner = tree_owner(&[("Alpha", &["a-0"]), ("Beta", &["b-0"])]);
    press(&mut owner, Key::Home);
    // The projection belongs to Beta while the tree focuses Alpha.
    let beta_target = MusicArtistTarget {
        artist_id: Some("artist-Beta".into()),
        artist_name: "Beta".into(),
        album_targets: vec!["b-0".into()],
        revision: owner.context.catalog_revision,
    };
    let mut track = make_item("Beta Track", "Audio");
    track.id = "beta-track".into();
    track.album_id = "b-0".into();
    let mut ctx = owner.context.clone();
    ctx.artist_detail = Some(ArtistDetailProjection {
        target: beta_target,
        summary: ArtistSummary {
            name: "Beta".into(),
            album_count: 1,
            year_start: None,
            year_end: None,
        },
        track_groups: vec![ArtistTrackGroup {
            album_id: "b-0".into(),
            album_title: "b-0".into(),
            tracks: vec![track.clone()],
        }],
        artwork: HeroImageState::None,
        artwork_cache_key: None,
    });
    owner.set_content(ctx);

    {
        let content = owner.content();
        assert!(
            content.hero.is_none(),
            "no stale Beta Hero paints under Alpha"
        );
    }
    assert!(
        owner.track_list.rows().is_empty(),
        "no stale Beta group rows paint under Alpha"
    );
    assert!(
        owner.workspace_track_item("beta-track").is_none(),
        "the stale track is not resolvable for activation"
    );
}

/// Task 6.4: a local album move between pushes resolves no rows until the new
/// album's snapshot arrives — the prior album's tracks never paint under the
/// new title.
#[test]
fn a_local_album_move_never_paints_the_prior_albums_tracks() {
    let mut owner = tree_owner_with_tracks(
        &[("Alpha", &["a-0", "a-1"])],
        Some(vec![make_item("Old Track", "Audio")]),
    );
    assert_eq!(owner.track_list.rows().len(), 1, "fixture rows for a-0");

    press(&mut owner, Key::Down);
    assert_eq!(owner.browser.selected_album_target(), Some("a-1"));
    {
        let content = owner.content();
        assert!(content.hero.is_some(), "the new leaf's title still paints");
    }
    assert!(
        owner.track_list.rows().is_empty(),
        "the prior album's rows must not paint under a-1"
    );

    // The a-1 push supplies its own snapshot and rows.
    let mut ctx = owner.context.clone();
    ctx.selected_album = owner.selected_item();
    ctx.album_tracks = Some(vec![make_item("New Track", "Audio")]);
    owner.set_content(ctx);
    assert_eq!(owner.track_list.rows().len(), 1);
}

/// Task 6.4: artist roots are hero-bearing rows (double-click/Right own the
/// overlay), while Enter stays their expansion toggle and only an album leaf
/// keeps the Enter overlay entry.
#[test]
fn artist_roots_own_the_overlay_but_enter_stays_their_expansion_toggle() {
    use crate::app::components::library_panel::owner::LibraryContentOwner;

    let mut owner = artist_workspace_owner();
    assert!(
        owner.hero_overlay_target_available(),
        "an artist root is a hero-bearing row"
    );
    assert!(
        !owner.hero_overlay_enter_available(),
        "Enter toggles an artist root, never the overlay"
    );

    let mut album_owner = tree_owner(&[("Alpha", &["a-0"])]);
    assert!(album_owner.hero_overlay_enter_available());
    assert!(album_owner.hero_overlay_target_available());
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
    tree_owner_with_tracks(artists, None)
}

fn tree_owner_with_tracks(
    artists: &[(&str, &[&str])],
    album_tracks: Option<Vec<EmbyItem>>,
) -> MusicContent {
    let mut items: Vec<EmbyItem> = Vec::new();
    let mut album_info: Vec<(String, String, String)> = Vec::new();
    let mut artist_keys: Vec<crate::app::music_grouping::ArtistKey> = Vec::new();
    for (artist, targets) in artists {
        for target in *targets {
            let mut album = make_item(target, "MusicAlbum");
            album.id = (*target).to_string();
            album.artist = (*artist).to_string();
            // Grouped Music album rows are folder targets: shell actions must
            // expand them through the existing per-folder playback effects.
            album.is_folder = true;
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
        album_tracks,
    );
    let mut owner = MusicContent::new();
    owner.set_content(ctx);
    owner.selection_origin = Some(SelectionOrigin::Library(
        crate::app::components::media_list::LibrarySelectionOrigin::Service(
            crate::app::components::library_panel::owner::LibraryKey::Service {
                service: mbv_core::config::ServiceKind::Emby,
                library_id: "music-library".into(),
                kind: crate::app::components::library_panel::owner::LibraryKind::Music,
            },
        ),
    ));
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

fn paint_tree(owner: &mut MusicContent, area: Rect) {
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("tree terminal");
    terminal
        .draw(|frame| owner.browser.view(frame, area))
        .expect("tree frame");
}

fn tree_point(owner: &MusicContent, area: Rect, id: usize) -> Position {
    let row = owner
        .browser
        .projected_nodes()
        .iter()
        .position(|node| node.id() == id)
        .expect("node is projected");
    let visible_row = row
        .checked_sub(owner.browser.offset())
        .expect("node is inside the painted viewport");
    Position::new(area.x, area.y.saturating_add(visible_row as u16))
}

#[test]
fn tree_pointer_gestures_resolve_latest_artist_and_album_rows() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    owner.expand_all_tree_roots();
    let area = Rect::new(0, 0, 48, 8);
    paint_tree(&mut owner, area);

    let root = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()).is_none())
        .expect("artist root")
        .id();
    let album_0 = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()) == Some("a-0"))
        .expect("first album leaf")
        .id();
    let album_1 = owner
        .browser
        .projected_nodes()
        .iter()
        .find(|node| owner.browser.target_of(node.id()) == Some("a-1"))
        .expect("second album leaf")
        .id();
    let root_at = tree_point(&owner, area, root);
    let album_0_at = tree_point(&owner, area, album_0);

    // A click resolves the painted artist row and changes local selection;
    // the grouping root manufactures no album request, but its resolved
    // focus crosses as the typed artist-track request (design D7).
    match owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
        root_at,
    ))) {
        Some(Msg::Shell(ShellRequest::MusicArtistTracks { target })) => {
            assert_eq!(target.artist_name, "Alpha");
        }
        other => panic!("expected the typed artist-track request, got {other:?}"),
    }
    assert!(owner.browser.selected_is_artist());

    // The same completed frame resolves a leaf click to its stable album
    // target, then wheel keeps the existing one-visible-row step semantics.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
            album_0_at
        ))),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target: 0, .. }))
    ));
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));

    // A modified click resolves the current painted row first, toggles only
    // the album leaf, and never emits a playback or Queue request.
    assert!(owner
        .on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ToggleClick(
            album_0_at,
        )))
        .is_none());
    assert_eq!(
        owner.browser.selected_album_targets(),
        vec!["a-0".to_string()]
    );
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
            at: album_0_at,
            delta: 1,
        })),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target: 1, .. }))
    ));
    assert_eq!(owner.browser.selected_id(), Some(album_1));

    // Double-click and right-click resolve the row under the latest retained
    // geometry, rather than the previously focused node.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            album_0_at
        ))),
        Some(Msg::Shell(ShellRequest::MusicAlbumActivate { item })) if item.id == "a-0"
    ));
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ContextClick(
            album_0_at
        ))),
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            Some((x, y)),
        ))) if items.len() == 1 && items[0].id == "a-0" && (x, y) == (album_0_at.x, album_0_at.y)
    ));

    // A right-click on an artist root resolves its ordered album descendants,
    // never the grouping root itself.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ContextClick(
            root_at
        ))),
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            Some((x, y)),
        ))) if items.len() == 2
            && items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>()
                == vec!["a-0", "a-1"]
            && (x, y) == (root_at.x, root_at.y)
    ));
    assert!(owner.browser.selected_is_artist());

    // Artist modified-click scopes the operation to its currently visible
    // album descendants, not to an artist or Queue identity.
    let _ = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ToggleClick(
        root_at,
    )));
    assert_eq!(
        owner.browser.selected_album_targets_in_display_order(),
        vec!["a-0".to_string(), "a-1".to_string()]
    );
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

/// Task 2.4 correction: an album-leaf Enter focuses the Wide inline track
/// pane, but moving the tree selection onto an artist root must not let the
/// stale pane focus swallow the root's Enter toggle. Before the fix the
/// `track_focused` Enter arm matched first and short-circuited through
/// `selected_item()?` (which is `None` for a root), so Enter did nothing.
#[test]
fn enter_toggles_a_root_reached_while_the_track_pane_holds_focus() {
    let mut owner = tree_owner_with_tracks(
        &[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])],
        Some(vec![make_item("t-0", "Audio"), make_item("t-1", "Audio")]),
    );
    owner.set_inline_track_focus_enabled(true);

    // Album-leaf Enter focuses the inline track pane (Wide).
    assert_eq!(owner.browser.selected_album_target(), Some("a-0"));
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(
        owner.track_focused(),
        "album-leaf Enter focuses the track pane"
    );

    // Wide `Home` is not track-focus gated and moves the tree selection onto
    // the Alpha artist root while the pane still holds focus.
    press(&mut owner, Key::Home);
    assert!(owner.browser.selected_is_artist());
    let root = owner.browser.selected_id().expect("artist root selected");
    assert!(owner.browser.root_is_expanded(root));

    // The stale pane must not swallow Enter: the root toggles instead.
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(
        !owner.browser.root_is_expanded(root),
        "Enter collapses the focused root"
    );
    // An artist root resolves no album, so no Hero/Workspace is projected and
    // the previous album's tracks cannot paint under the root.
    assert!(
        owner.panel_content().hero.is_none(),
        "no stale album Workspace paints under an artist root"
    );
}

/// Task 6.3 correction: an artist root's Workspace rows come from the
/// shell-projected `artist_detail` groups (the artist push clears
/// `selected_album`/`album_tracks`), so row behaviours must resolve the track
/// and its owning album from that projection, not the empty album snapshot.
fn artist_workspace_owner() -> MusicContent {
    use crate::app::music_artist_detail::{
        ArtistDetailProjection, ArtistSummary, ArtistTrackGroup,
    };

    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    press(&mut owner, Key::Home);
    assert!(owner.browser.selected_is_artist(), "fixture focuses a root");
    let target = owner.artist_detail_target().expect("artist target");
    let mut first = make_item("Track One", "Audio");
    first.id = "alpha-track-1".into();
    first.album_id = "a-0".into();
    let mut second = make_item("Track Two", "Audio");
    second.id = "alpha-track-2".into();
    second.album_id = "a-1".into();
    let detail = ArtistDetailProjection {
        target: target.clone(),
        summary: ArtistSummary {
            name: target.artist_name.clone(),
            album_count: 2,
            year_start: Some(2001),
            year_end: Some(2001),
        },
        track_groups: vec![
            ArtistTrackGroup {
                album_id: "a-0".into(),
                album_title: "a-0".into(),
                tracks: vec![first],
            },
            ArtistTrackGroup {
                album_id: "a-1".into(),
                album_title: "a-1".into(),
                tracks: vec![second],
            },
        ],
        artwork: HeroImageState::None,
        artwork_cache_key: None,
    };
    let mut ctx = owner.context.clone();
    ctx.selected_album = None;
    ctx.album_tracks = None;
    ctx.artist_detail = Some(detail);
    owner.set_content(ctx);
    owner.expand_all_tree_roots();
    owner
}

/// Paints the artist Workspace track list into `area`: row 0 is the `a-0`
/// heading (not selectable), row 1 the first `alpha-track-1` item row.
fn paint_artist_workspace(owner: &mut MusicContent, area: Rect) {
    owner.track_list.wide_mut().set_geometry(area, area);
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("workspace terminal");
    terminal
        .draw(|frame| owner.track_list.wide_mut().view(frame, area))
        .expect("workspace frame");
}

#[test]
fn enter_activates_an_artist_workspace_track_from_its_group() {
    let mut owner = artist_workspace_owner();
    owner.set_inline_track_focus_enabled(true);
    owner.enter_track_focus();
    assert!(owner.track_focused(), "the artist Workspace holds focus");

    match press(&mut owner, Key::Enter) {
        Some(Msg::Shell(ShellRequest::MusicTrackActivate { album_id, track })) => {
            assert_eq!(
                album_id, "a-0",
                "the projected group's settled album identity crosses"
            );
            assert_eq!(track.id, "alpha-track-1");
        }
        other => panic!("expected artist track activation, got {other:?}"),
    }
}

#[test]
fn enter_activates_a_cached_tree_track_through_the_existing_arm() {
    let mut owner = artist_workspace_owner();
    owner.expand_all_tree_roots();
    press(&mut owner, Key::Home);
    press(&mut owner, Key::Down);
    press(&mut owner, Key::Down);

    match press(&mut owner, Key::Enter) {
        Some(Msg::Shell(ShellRequest::MusicTrackActivate { album_id, track })) => {
            assert_eq!(album_id, "a-0");
            assert_eq!(track.id, "alpha-track-1");
        }
        other => panic!("expected cached tree track activation, got {other:?}"),
    }
}

#[test]
fn hero_double_click_activates_an_artist_workspace_track() {
    let mut owner = artist_workspace_owner();
    let area = Rect::new(0, 0, 30, 4);
    paint_artist_workspace(&mut owner, area);

    let message = owner.on_slot_event(LibrarySlotEvent::HeroPane(
        MediaListSurfaceInput::DoubleClick(Position { x: 0, y: 1 }),
    ));
    match message {
        Some(Msg::Shell(ShellRequest::MusicTrackActivate { album_id, track })) => {
            assert_eq!(album_id, "a-0");
            assert_eq!(track.id, "alpha-track-1");
        }
        other => panic!("expected artist track activation, got {other:?}"),
    }
}

#[test]
fn artist_workspace_track_context_menu_resolves_projected_groups() {
    let mut owner = artist_workspace_owner();
    owner.set_inline_track_focus_enabled(true);
    owner.enter_track_focus();

    match press(&mut owner, Key::Char('.')) {
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            None,
        ))) => {
            assert_eq!(items.len(), 1, "the focused artist track is a real target");
            assert_eq!(items[0].id, "alpha-track-1");
        }
        other => panic!("expected an artist track context menu, got {other:?}"),
    }
}

#[test]
fn hero_context_click_resolves_an_artist_workspace_track() {
    let mut owner = artist_workspace_owner();
    let area = Rect::new(0, 0, 30, 4);
    paint_artist_workspace(&mut owner, area);

    let message = owner.on_slot_event(LibrarySlotEvent::HeroPane(
        MediaListSurfaceInput::ContextClick(Position { x: 0, y: 1 }),
    ));
    match message {
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            Some((0, 1)),
        ))) => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].id, "alpha-track-1");
        }
        other => panic!("expected an artist track context menu, got {other:?}"),
    }
}

#[test]
fn artist_workspace_root_still_toggles_while_the_rail_owns_the_focus() {
    let mut owner = artist_workspace_owner();
    let root = owner.browser.selected_id().expect("artist root selected");
    assert!(owner.browser.root_is_expanded(root));
    assert!(!owner.track_focused(), "the rail owns the focus");

    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(
        !owner.browser.root_is_expanded(root),
        "Enter on the artist root still toggles expansion"
    );
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
    // album persistence); the resolved root focus crosses as the typed
    // artist-track request (design D7).
    match press(&mut owner, Key::Left) {
        Some(Msg::Shell(ShellRequest::MusicArtistTracks { target })) => {
            assert_eq!(target.artist_name, "Alpha");
        }
        other => panic!("expected the typed artist-track request, got {other:?}"),
    }
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
fn right_expands_a_collapsed_artist_root_then_enters_its_workspace() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"]), ("Beta", &["b-0"])]);
    // Home selects Alpha's (already expanded) root; Down twice reaches Beta's
    // collapsed root.
    press(&mut owner, Key::Home);
    press(&mut owner, Key::Down);
    press(&mut owner, Key::Down);
    let root = owner.browser.selected_id().expect("artist root selected");
    assert!(owner.browser.selected_is_artist());
    assert!(!owner.browser.root_is_expanded(root));

    // The first Right on the collapsed root expands it and nothing else
    // (task 6.4): the artist Workspace is entered only by a later Right.
    assert_eq!(
        press(&mut owner, Key::Right),
        None,
        "expand emits no request"
    );
    assert!(
        owner.browser.root_is_expanded(root),
        "Right expands the root"
    );
    assert!(
        !owner.track_focused(),
        "the first Right must not enter the Workspace"
    );

    // The later Right on the already expanded root enters the artist
    // Workspace: non-Wide asks the shell to open its Library Hero overlay for
    // the component-resolved root; it never re-expands or collapses.
    match press(&mut owner, Key::Right) {
        Some(Msg::Shell(ShellRequest::MusicArtistActivate { target })) => {
            assert_eq!(target.artist_name, "Beta");
            assert_eq!(target.album_targets, vec!["b-0".to_string()]);
        }
        other => panic!("expected the artist Workspace entry, got {other:?}"),
    }
    assert!(owner.browser.root_is_expanded(root));
    assert_eq!(owner.browser.selected_id(), Some(root));
}

/// Task 6.4: in Wide geometry the later Right on an expanded artist root takes
/// the inline artist-track Workspace's focus locally, with no shell request.
#[test]
fn wide_right_on_an_expanded_artist_root_takes_the_inline_workspace_focus() {
    let mut owner = artist_workspace_owner();
    owner.set_inline_track_focus_enabled(true);
    let root = owner.browser.selected_id().expect("artist root selected");
    assert!(owner.browser.root_is_expanded(root));
    assert!(!owner.track_focused());

    assert_eq!(press(&mut owner, Key::Right), None);
    assert!(
        owner.track_focused(),
        "Wide Right enters the artist Workspace"
    );
}

/// Task 6.4: a Wide artist-Workspace entry armed while the root's rows are
/// still loading takes the focus when the rows arrive, without a second key.
#[test]
fn wide_right_waits_for_the_artist_rows_before_taking_the_pane_focus() {
    use crate::app::music_artist_detail::{
        ArtistDetailProjection, ArtistSummary, ArtistTrackGroup,
    };

    let mut owner = tree_owner(&[("Alpha", &["a-0"])]);
    press(&mut owner, Key::Home);
    let target = owner.artist_detail_target().expect("artist root selected");
    owner.set_inline_track_focus_enabled(true);

    // Loading projection: the root is expanded but its groups are empty.
    let mut ctx = owner.context.clone();
    ctx.selected_album = None;
    ctx.album_tracks = None;
    ctx.artist_detail = Some(ArtistDetailProjection {
        target: target.clone(),
        summary: ArtistSummary {
            name: "Alpha".into(),
            album_count: 1,
            year_start: None,
            year_end: None,
        },
        track_groups: Vec::new(),
        artwork: HeroImageState::None,
        artwork_cache_key: None,
    });
    owner.set_content(ctx);
    assert_eq!(press(&mut owner, Key::Right), None);
    assert!(
        !owner.track_focused(),
        "no rows exist yet, so the entry stays armed"
    );

    // The rows land: the armed entry takes the focus on the same push.
    let mut track = make_item("Track One", "Audio");
    track.id = "alpha-track-1".into();
    track.album_id = "a-0".into();
    let mut ctx = owner.context.clone();
    ctx.artist_detail = Some(ArtistDetailProjection {
        target,
        summary: ArtistSummary {
            name: "Alpha".into(),
            album_count: 1,
            year_start: None,
            year_end: None,
        },
        track_groups: vec![ArtistTrackGroup {
            album_id: "a-0".into(),
            album_title: "a-0".into(),
            tracks: vec![track],
        }],
        artwork: HeroImageState::None,
        artwork_cache_key: None,
    });
    owner.set_content(ctx);
    assert!(owner.track_focused(), "the armed entry takes the focus");
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

/// Music tree album leaves ignore stored played state while retaining the
/// playback-live position as `Active`.
#[test]
fn tree_entries_ignore_played_but_keep_live_progress() {
    let mut album = make_item("Played Album", "MusicAlbum");
    album.id = "album-played".into();
    album.artist = "Alpha".into();
    album.played = true;
    album.playback_position_ticks = 120_000_000;
    album.runtime_ticks = 240_000_000;
    let ctx = MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![album], 0),
        None,
        String::new(),
        Vec::new(),
        0,
        vec![("Alpha".into(), "2001".into(), "Played Album".into())],
        vec![crate::app::music_grouping::ArtistKey::Service(
            "artist-Alpha".into(),
        )],
        vec![0],
        None,
    );
    let mut owner = MusicContent::new();
    owner.set_content(ctx);

    let entries = owner.tree_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Played Album");
    assert_eq!(
        entries[0].semantic_state,
        MediaSemanticState::active(Some(50)),
        "stored played state never suppresses live playback emphasis"
    );
}

fn tree_owner_with_stable_keys(artists: &[(&str, &str, &[&str])]) -> MusicContent {
    let mut items = Vec::new();
    let mut album_info = Vec::new();
    let mut artist_keys = Vec::new();
    for (artist, artist_id, targets) in artists {
        for target in *targets {
            let mut album = make_item(target, "MusicAlbum");
            album.id = (*target).to_string();
            album.artist = (*artist).to_string();
            album.is_folder = true;
            items.push(album);
            album_info.push((
                (*artist).to_string(),
                "2001".to_string(),
                (*target).to_string(),
            ));
            artist_keys.push(crate::app::music_grouping::ArtistKey::Service(
                (*artist_id).to_string(),
            ));
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
    owner.selection_origin = Some(SelectionOrigin::Library(
        crate::app::components::media_list::LibrarySelectionOrigin::Service(
            crate::app::components::library_panel::owner::LibraryKey::Service {
                service: mbv_core::config::ServiceKind::Emby,
                library_id: "music-library".into(),
                kind: crate::app::components::library_panel::owner::LibraryKind::Music,
            },
        ),
    ));
    owner
}

fn artist_action_ids(owner: &mut MusicContent, code: Key) -> Vec<String> {
    let message = owner.on_key(&KeyEvent {
        code,
        modifiers: if matches!(code, Key::Char('p' | 'a' | 's')) {
            KeyModifiers::CONTROL
        } else {
            KeyModifiers::NONE
        },
    });
    let items = match message {
        Some(Msg::Shell(ShellRequest::MusicArtistAction {
            items,
            unresolved_targets,
            ..
        })) => {
            assert!(
                unresolved_targets.is_empty(),
                "settled artist fixture should resolve every album target"
            );
            assert!(
                items.iter().all(|item| item.is_folder),
                "artist play/enqueue/shuffle requests carry folder album targets"
            );
            items
        }
        Some(Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            _,
        ))) => {
            assert!(
                items.iter().all(|item| item.is_folder),
                "artist context requests carry folder album targets"
            );
            items
        }
        other => panic!("expected an artist album action for {code:?}, got {other:?}"),
    };
    items.into_iter().map(|item| item.id).collect()
}

#[test]
fn artist_actions_materialize_all_albums_for_collapsed_and_expanded_roots() {
    let expected = vec!["a-0".to_string(), "a-1".to_string(), "a-2".to_string()];
    let mut collapsed = tree_owner(&[("Alpha", &["a-0", "a-1", "a-2"])]);
    press(&mut collapsed, Key::Home);
    let root = collapsed
        .browser
        .selected_id()
        .expect("artist root selected");
    collapsed.browser.collapse_root(root);
    for code in [
        Key::Char('p'),
        Key::Char('a'),
        Key::Char('s'),
        Key::Char('.'),
    ] {
        assert_eq!(artist_action_ids(&mut collapsed, code), expected);
    }

    let mut expanded = tree_owner(&[("Alpha", &["a-0", "a-1", "a-2"])]);
    press(&mut expanded, Key::Home);
    let root = expanded
        .browser
        .selected_id()
        .expect("artist root selected");
    expanded.browser.expand_root(root);
    for code in [
        Key::Char('p'),
        Key::Char('a'),
        Key::Char('s'),
        Key::Char('.'),
    ] {
        assert_eq!(artist_action_ids(&mut expanded, code), expected);
    }
}

#[test]
fn filtered_artist_actions_materialize_only_matching_leaves_in_settled_order() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1", "a-2", "a-3", "a-4"])]);
    press(&mut owner, Key::Home);
    let matching: Vec<usize> = owner
        .browser
        .projected_nodes()
        .iter()
        .filter_map(|node| {
            matches!(owner.browser.target_of(node.id()), Some("a-1" | "a-3")).then_some(node.id())
        })
        .collect();
    owner.browser.set_filter_matches(Some(&matching));

    assert_eq!(
        artist_action_ids(&mut owner, Key::Char('p')),
        vec!["a-1".to_string(), "a-3".to_string()]
    );
    assert_eq!(
        artist_action_ids(&mut owner, Key::Char('.')),
        vec!["a-1".to_string(), "a-3".to_string()]
    );
}

#[test]
fn partially_unresolved_artist_actions_keep_ordered_targets_and_report_misses() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    press(&mut owner, Key::Home);
    // Simulate a tree/content sync race: the tree still has both leaves, but
    // the latest content snapshot only resolves the first one.
    owner.context.album_targets = vec!["a-0".into(), "stale-target".into()];
    owner.context.list.items.truncate(1);

    for code in [Key::Char('p'), Key::Char('a'), Key::Char('s')] {
        let message = owner.on_key(&KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
        });
        match message {
            Some(Msg::Shell(ShellRequest::MusicArtistAction {
                items,
                unresolved_targets,
                ..
            })) => {
                assert_eq!(
                    items.into_iter().map(|item| item.id).collect::<Vec<_>>(),
                    vec!["a-0"],
                    "a resolved album must survive a stale sibling target"
                );
                assert_eq!(unresolved_targets, vec!["a-1"]);
            }
            other => panic!("expected a partial artist action for {code:?}, got {other:?}"),
        }
    }
}

#[test]
fn fully_unresolved_artist_action_requests_shell_feedback() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    press(&mut owner, Key::Home);
    owner.context.album_targets.clear();
    owner.context.list.items.clear();

    match owner.on_key(&KeyEvent {
        code: Key::Char('p'),
        modifiers: KeyModifiers::CONTROL,
    }) {
        Some(Msg::Shell(ShellRequest::MusicArtistAction {
            items,
            unresolved_targets,
            ..
        })) => {
            assert!(items.is_empty());
            assert_eq!(unresolved_targets, vec!["a-0", "a-1"]);
        }
        other => panic!("expected shell feedback request, got {other:?}"),
    }
}

#[test]
fn empty_visible_artist_emits_no_action_target() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    press(&mut owner, Key::Home);
    owner.browser.set_filter_matches(Some(&[]));

    for (code, modifiers) in [
        (Key::Char('p'), KeyModifiers::CONTROL),
        (Key::Char('a'), KeyModifiers::CONTROL),
        (Key::Char('s'), KeyModifiers::CONTROL),
        (Key::Char('.'), KeyModifiers::NONE),
    ] {
        assert!(
            owner.on_key(&KeyEvent { code, modifiers }).is_none(),
            "an artist with no visible album leaves has no action"
        );
    }
}

#[test]
fn equal_name_artists_resolve_actions_by_stable_root_identity() {
    let mut owner = tree_owner_with_stable_keys(&[
        ("Same Name", "artist-one", &["one-album"]),
        ("Same Name", "artist-two", &["two-album"]),
    ]);
    press(&mut owner, Key::Home);
    press(&mut owner, Key::Down);
    press(&mut owner, Key::Down);
    assert!(owner.browser.selected_is_artist());

    for code in [
        Key::Char('p'),
        Key::Char('a'),
        Key::Char('s'),
        Key::Char('.'),
    ] {
        assert_eq!(
            artist_action_ids(&mut owner, code),
            vec!["two-album".to_string()],
            "the second equal-name root owns only its album"
        );
    }
}
