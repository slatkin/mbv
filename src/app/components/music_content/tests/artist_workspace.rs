//! Grouped Music artist-Workspace tests: the mounted artist root's inline
//! Wide pane, its track activation/context arms, and expansion/focus entry.

use super::*;

/// Task 6.3 correction: an artist root's Workspace rows come from the
/// shell-projected `artist_detail` groups (the artist push clears
/// `selected_album`/`album_tracks`), so row behaviours must resolve the track
/// and its owning album from that projection, not the empty album snapshot.
pub(super) fn artist_workspace_owner() -> MusicContent {
    use crate::app::state::music_artist_detail::{
        ArtistDetailProjection, ArtistSummary, ArtistTrackGroup,
    };

    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    press(&mut owner, Key::Home);
    assert!(owner.selected_is_artist(), "fixture focuses a root");
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
        Some(Msg::Shell(shell_boxed)) => {
            let ShellRequest::MusicArtistTrackActivate { target, track_id } = *shell_boxed else {
                panic!("expected artist track activation, got {shell_boxed:?}")
            };
            assert_eq!(target.artist_name, "Alpha");
            assert_eq!(target.album_targets, ["a-0", "a-1"]);
            assert_eq!(track_id, "alpha-track-1");
        }
        other => panic!("expected artist track activation, got {other:?}"),
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
        Some(Msg::Shell(shell_boxed)) => {
            let ShellRequest::MusicTreeTrackActivate {
                album_target,
                track_id,
            } = *shell_boxed
            else {
                panic!("expected grouped tree track activation, got {shell_boxed:?}")
            };
            assert_eq!(album_target, "a-0");
            assert_eq!(track_id, "alpha-track-1");
        }
        other => panic!("expected grouped tree track activation, got {other:?}"),
    }
}

#[test]
fn artist_workspace_track_context_menu_resolves_projected_groups() {
    let mut owner = artist_workspace_owner();
    owner.set_inline_track_focus_enabled(true);
    owner.enter_track_focus();

    match press(&mut owner, Key::Char('.')) {
        Some(Msg::Shell(shell_boxed)) => {
            let ShellRequest::MusicRowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Emby(items),
                None,
            ) = *shell_boxed
            else {
                panic!("expected an artist track context menu, got {shell_boxed:?}")
            };
            assert_eq!(items.len(), 1, "the focused artist track is a real target");
            assert_eq!(items[0].id, "alpha-track-1");
        }
        other => panic!("expected an artist track context menu, got {other:?}"),
    }
}

#[test]
fn left_collapses_an_expanded_root_and_returns_a_leaf_to_its_parent() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    press(&mut owner, Key::Home);
    let root = owner
        .browser
        .selected_target()
        .cloned()
        .expect("artist root selected");
    // The initial album adoption expanded the first root's path.
    assert!(owner.browser.is_expanded(&root));
    press(&mut owner, Key::Down);
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-0"));

    // Left on a leaf returns to its artist parent without collapsing it and
    // without emitting an album-cursor request (an artist never overwrites
    // album persistence); the resolved root focus crosses as the typed
    // artist-track request (design D7).
    match press(&mut owner, Key::Left) {
        Some(Msg::Shell(shell_boxed)) => {
            let ShellRequest::MusicArtistTracks { target } = *shell_boxed else {
                panic!("expected the typed artist-track request, got {shell_boxed:?}")
            };
            assert_eq!(target.artist_name, "Alpha");
        }
        other => panic!("expected the typed artist-track request, got {other:?}"),
    }
    assert_eq!(owner.browser.selected_target(), Some(&root));
    assert!(
        owner.browser.is_expanded(&root),
        "moving to the parent must not collapse it"
    );

    // Left on the expanded root collapses it in place.
    assert_eq!(press(&mut owner, Key::Left), None);
    assert!(!owner.browser.is_expanded(&root));
    assert_eq!(owner.browser.selected_target(), Some(&root));
}
