//! Grouped Music artist-Workspace tests: the mounted artist root's inline
//! Wide pane, its track activation/context arms, and expansion/focus entry.

use super::tree_tests::{press, selected_row, tree_owner};
use super::*;

/// Task 6.3 correction: an artist root's Workspace rows come from the
/// shell-projected `artist_detail` groups (the artist push clears
/// `selected_album`/`album_tracks`), so row behaviours must resolve the track
/// and its owning album from that projection, not the empty album snapshot.
pub(super) fn artist_workspace_owner() -> MusicContent {
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
        Some(Msg::Shell(ShellRequest::MusicArtistTrackActivate { target, track_id })) => {
            assert_eq!(target.artist_name, "Alpha");
            assert_eq!(target.album_targets, ["a-0", "a-1"]);
            assert_eq!(track_id, "alpha-track-1");
        }
        other => panic!("expected artist track activation, got {other:?}"),
    }
}

#[test]
fn hero_activate_activates_an_artist_workspace_track() {
    let mut owner = artist_workspace_owner();
    owner.set_inline_track_focus_enabled(true);
    owner.enter_track_focus();

    match owner.on_slot_event(LibrarySlotEvent::HeroActivate) {
        Some(Msg::Shell(ShellRequest::MusicArtistTrackActivate { target, track_id })) => {
            assert_eq!(target.artist_name, "Alpha");
            assert_eq!(track_id, "alpha-track-1");
        }
        other => panic!("expected artist Hero activation, got {other:?}"),
    }
}

#[test]
fn enter_activates_a_cached_tree_track_through_the_grouped_resolver() {
    let mut owner = artist_workspace_owner();
    owner.expand_all_tree_roots();
    press(&mut owner, Key::Home);
    press(&mut owner, Key::Down);
    press(&mut owner, Key::Down);

    // A tree track carries only stable identities: the shell-owned grouped
    // resolver picks the cached queue according to the autoload policy.
    match press(&mut owner, Key::Enter) {
        Some(Msg::Shell(ShellRequest::MusicTreeTrackActivate {
            album_target,
            track_id,
        })) => {
            assert_eq!(album_target, "a-0");
            assert_eq!(track_id, "alpha-track-1");
        }
        other => panic!("expected grouped tree track activation, got {other:?}"),
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
        Some(Msg::Shell(ShellRequest::MusicArtistTrackActivate { target, track_id })) => {
            assert_eq!(target.artist_name, "Alpha");
            assert_eq!(target.album_targets, ["a-0", "a-1"]);
            assert_eq!(track_id, "alpha-track-1");
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
        Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
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
        Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
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
fn artist_workspace_root_enter_is_handled_by_the_non_wide_panel_gate() {
    let mut owner = artist_workspace_owner();
    let root = owner
        .browser
        .selected_target()
        .expect("artist root selected");
    assert!(owner.browser.root_is_expanded(&root));
    assert!(!owner.track_focused(), "the rail owns the focus");

    // The mounted non-Wide panel intercepts this chord before the owner and
    // opens the Library Hero overlay itself. A direct owner therefore has no
    // request to emit on this geometry.
    assert_eq!(press(&mut owner, Key::Enter), None);
    assert!(
        owner.browser.root_is_expanded(&root),
        "unfiltered Enter preserves expansion"
    );
}

#[test]
fn left_collapses_an_expanded_root_and_returns_a_leaf_to_its_parent() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    press(&mut owner, Key::Home);
    let root = owner
        .browser
        .selected_target()
        .expect("artist root selected");
    // The initial album adoption expanded the first root's path.
    assert!(owner.browser.root_is_expanded(&root));
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
    assert_eq!(owner.browser.selected_target(), Some(root.clone()));
    assert!(
        owner.browser.root_is_expanded(&root),
        "moving to the parent must not collapse it"
    );

    // Left on the expanded root collapses it in place.
    assert_eq!(press(&mut owner, Key::Left), None);
    assert!(!owner.browser.root_is_expanded(&root));
    assert_eq!(owner.browser.selected_target(), Some(root.clone()));
}

#[test]
fn right_expands_a_collapsed_artist_root_then_enters_its_workspace() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"]), ("Beta", &["b-0"])]);
    // Home selects Alpha's (already expanded) root; Down twice reaches Beta's
    // collapsed root.
    press(&mut owner, Key::Home);
    press(&mut owner, Key::Down);
    press(&mut owner, Key::Down);
    let root = owner
        .browser
        .selected_target()
        .expect("artist root selected");
    assert!(owner.browser.selected_is_artist());
    assert!(!owner.browser.root_is_expanded(&root));

    // The first Right on the collapsed root expands it and nothing else
    // (task 6.4): the artist Workspace is entered only by a later Right.
    assert_eq!(
        press(&mut owner, Key::Right),
        None,
        "expand emits no request"
    );
    assert!(
        owner.browser.root_is_expanded(&root),
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
    assert!(owner.browser.root_is_expanded(&root));
    assert_eq!(owner.browser.selected_target(), Some(root.clone()));
}

/// Task 6.4: in Wide geometry the later Right on an expanded artist root takes
/// the inline artist-track Workspace's focus locally, with no shell request.
#[test]
fn wide_right_on_an_expanded_artist_root_takes_the_inline_workspace_focus() {
    let mut owner = artist_workspace_owner();
    owner.set_inline_track_focus_enabled(true);
    let root = owner
        .browser
        .selected_target()
        .expect("artist root selected");
    assert!(owner.browser.root_is_expanded(&root));
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
