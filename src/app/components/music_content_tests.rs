use self::artist_workspace_tests::artist_workspace_owner;
use self::tree_tests::{press, tree_owner, tree_owner_with_tracks};
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
fn grouped_music_filter_keeps_the_tree_panel_owner_and_uses_the_shared_query_editor() {
    let mut first = make_item("First Album", "Folder");
    first.id = "album-1".into();
    let mut second = make_item("Second Album", "Folder");
    second.id = "album-2".into();
    let mut owner = MusicContent::new();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![first, second], 0),
        None,
        String::new(),
        Vec::new(),
        0,
        vec![
            ("Artist".into(), "2001".into(), "First Album".into()),
            ("Artist".into(), "2002".into(), "Second Album".into()),
        ],
        vec![
            crate::app::music_grouping::ArtistKey::Fallback("Artist".into()),
            crate::app::music_grouping::ArtistKey::Fallback("Artist".into()),
        ],
        vec![0, 1],
        None,
    ));

    let slash = KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    };
    assert!(owner.on_key(&slash).is_some());
    assert!(owner.inline_search.is_active());
    assert!(owner.browser.filter_active());
    owner.inline_search.restore_query("Second".into());
    owner
        .browser
        .apply_filter_query(owner.inline_search.query());
    let content = owner.content();
    assert!(matches!(content.list, ListSlot::Media(_)));
    drop(content);
    let titles: Vec<&str> = owner
        .browser
        .projected_nodes()
        .iter()
        .map(|node| owner.browser.title_of(node.id()))
        .collect();
    assert_eq!(titles, ["Artist", "Second Album"]);
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
    assert_eq!(
        header,
        Some(crate::app::components::library_panel::content::WorkspaceHeader::Tracklist)
    );
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
/// summary and album artwork — name, in-scope album count, year span, and the
/// first settled album's existing artwork source — with the projected groups
/// as its Workspace.
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
    let cache_key = "a-0:P".to_string();
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
    });
    owner.set_content(ctx);
    owner.set_hero_image(HeroImageState::Loading);

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
                assert_eq!(item_id, "a-0");
                assert_eq!(image_types, &vec!["AudioChild".to_string()]);
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

#[test]
fn artist_hero_switches_to_the_selected_track_album_artwork() {
    use crate::app::components::library_panel::content::ArtworkSource;
    use crate::app::music_artist_detail::{
        ArtistDetailProjection, ArtistSummary, ArtistTrackGroup,
    };

    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    press(&mut owner, Key::Home);
    let target = owner.artist_detail_target().expect("artist root selected");
    let mut first = make_item("First Track", "Audio");
    first.id = "track-a-0".into();
    first.album_id = "a-0".into();
    let mut second = make_item("Second Track", "Audio");
    second.id = "track-a-1".into();
    second.album_id = "a-1".into();
    let mut ctx = owner.context.clone();
    ctx.selected_album = None;
    ctx.album_tracks = None;
    ctx.artist_detail = Some(ArtistDetailProjection {
        target,
        summary: ArtistSummary {
            name: "Alpha".into(),
            album_count: 2,
            year_start: None,
            year_end: None,
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
    });
    owner.set_content(ctx);
    owner.expand_all_tree_roots();
    owner.set_hero_image(HeroImageState::Loading);

    let album_a0 = owner.browser.projected_nodes()[1].id();
    owner.browser.expand_node(album_a0);
    owner.browser.select_index(2);
    assert_eq!(
        owner.browser.selected_track_identity(),
        Some(("a-0", "track-a-0"))
    );
    let first_source = owner.hero_data().expect("artist hero").facts.artwork.source;
    assert!(matches!(
        first_source,
        Some(ArtworkSource::Emby { item_id, cache_key, .. })
            if item_id == "a-0" && cache_key == "a-0:P"
    ));

    let album_a1 = owner.browser.projected_nodes()[3].id();
    owner.browser.expand_node(album_a1);
    owner.browser.select_index(4);
    assert_eq!(
        owner.browser.selected_track_identity(),
        Some(("a-1", "track-a-1"))
    );
    let second_source = owner.hero_data().expect("artist hero").facts.artwork.source;
    assert!(matches!(
        second_source,
        Some(ArtworkSource::Emby { item_id, cache_key, .. })
            if item_id == "a-1" && cache_key == "a-1:P"
    ));
}

/// An artist root without a Service ID still uses the settled album's
/// artwork. The image source is album-scoped, so no artist-ID artwork request
/// is needed for either identity form.
#[test]
fn fallback_artist_hero_uses_the_first_album_artwork() {
    use crate::app::components::library_panel::content::ArtworkSource;
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
    });
    owner.set_content(ctx);

    let content = owner.content();
    let hero = content.hero.expect("fallback artist hero");
    assert_eq!(hero.facts.title, "Artist");
    assert_eq!(
        hero.facts.meta_rows,
        vec!["1 album".to_string(), "2024".to_string()]
    );
    match hero.facts.artwork.source.as_ref() {
        Some(ArtworkSource::Emby {
            item_id,
            image_types,
            cache_key,
            ..
        }) => {
            assert_eq!(item_id, "id");
            assert_eq!(image_types, &vec!["AudioChild".to_string()]);
            assert_eq!(cache_key, "id:P");
        }
        other => panic!("expected the first album artwork source, got {other:?}"),
    }
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

#[cfg(test)]
#[path = "music_content_tree_tests.rs"]
mod tree_tests;

#[cfg(test)]
#[path = "music_content_artist_workspace_tests.rs"]
mod artist_workspace_tests;

#[cfg(test)]
#[path = "music_content_artist_actions_tests.rs"]
mod artist_actions_tests;
