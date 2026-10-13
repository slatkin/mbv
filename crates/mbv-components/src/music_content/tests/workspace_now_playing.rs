//! Grouped Music Workspace now-playing override tests (task 3.1 of
//! `hero-workspace-now-playing`): the playing track's row lights in both the
//! album and the artist Workspace, and the rows guard dedups an identical
//! re-projection.

use self::tree_fixtures::tree_owner_with_tracks;
use super::*;
use mbv_queue::QueueItemContentId;
use mbv_render::components::media_list::ActiveProgress;

fn track(id: &str) -> EmbyItem {
    let mut track = make_item(id, "Audio");
    track.id = id.to_string();
    track
}

fn row_states(rows: &[MediaListRow<String>]) -> Vec<(String, MediaSemanticState)> {
    rows.iter()
        .filter_map(|row| match row {
            MediaListRow::Item {
                target,
                semantic_state,
                ..
            } => Some((target.clone(), semantic_state.clone())),
            _ => None,
        })
        .collect()
}

fn heading_texts(rows: &[MediaListRow<String>]) -> Vec<String> {
    rows.iter()
        .filter_map(|row| match row {
            MediaListRow::Heading { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Contract (hero-workspace-now-playing task 3.1, design D2/D4): the playing
/// track's row carries `NowPlaying { progress }` in the album and the artist
/// Workspace (both variants share `track_row`), every other row and every
/// heading keeps its derived state, and an identical re-pushed projection
/// skips `set_content` (the reconcile rows guard), so the painted frame stays
/// claimable.
#[test]
fn set_now_playing_marks_the_playing_track_row_in_both_workspaces() {
    // Album Workspace: the selected album leaf's own track rows. The tree
    // selection is anchored onto the album leaf so `resolved_workspace`
    // resolves that leaf's pushed snapshot.
    let mut owner = tree_owner_with_tracks(
        &[("Alpha", &["a-0"])],
        Some(vec![track("track-1"), track("track-2")]),
    );
    owner.expand_all_tree_roots();
    owner.select_tree_target(MusicTreeTarget::Album("a-0".into()));
    owner.set_now_playing(Some(WorkspaceNowPlaying::Item {
        content_id: QueueItemContentId::Emby("track-2".into()),
        progress: Some(37),
    }));
    owner.reconcile_workspace_rows();
    assert_eq!(
        row_states(owner.track_list.rows()),
        vec![
            ("track-1".into(), MediaSemanticState::Ordinary),
            (
                "track-2".into(),
                MediaSemanticState::NowPlaying {
                    progress: Some(ActiveProgress::new(37)),
                },
            ),
        ],
        "the playing row lights with progress; the other row keeps its derived state"
    );
    assert_eq!(heading_texts(owner.track_list.rows()), [] as [String; 0]);

    // Observable set_content skip: a completed paint is retained until the
    // carrier's set_content invalidates it.
    let list = Rect::new(0, 0, 40, 3);
    let geometry = owner.track_list.wide().row_geometry(3);
    owner
        .track_list
        .wide_mut()
        .finish_view(list, list, &geometry, None);
    assert!(owner.track_list.current_content_rect().is_some());

    // Identical input skips set_content (painted frame stays claimable).
    owner.set_now_playing(Some(WorkspaceNowPlaying::Item {
        content_id: QueueItemContentId::Emby("track-2".into()),
        progress: Some(37),
    }));
    owner.reconcile_workspace_rows();
    assert!(
        owner.track_list.current_content_rect().is_some(),
        "an identical projection must not re-run set_content"
    );

    // Control: changed input rebuilds (and invalidates the paint).
    owner.set_now_playing(Some(WorkspaceNowPlaying::Item {
        content_id: QueueItemContentId::Emby("track-2".into()),
        progress: Some(38),
    }));
    owner.reconcile_workspace_rows();
    assert!(owner.track_list.current_content_rect().is_none());

    // Artist Workspace: the projected detail groups' track rows, headings
    // included and unchanged.
    let mut owner = artist_workspace_owner();
    owner.set_now_playing(Some(WorkspaceNowPlaying::Item {
        content_id: QueueItemContentId::Emby("alpha-track-2".into()),
        progress: Some(55),
    }));
    owner.reconcile_workspace_rows();
    assert_eq!(
        heading_texts(owner.track_list.rows()),
        vec!["a-0".to_string(), "a-1".to_string()],
        "headings keep their derived state"
    );
    assert_eq!(
        row_states(owner.track_list.rows()),
        vec![
            ("alpha-track-1".into(), MediaSemanticState::Ordinary),
            (
                "alpha-track-2".into(),
                MediaSemanticState::NowPlaying {
                    progress: Some(ActiveProgress::new(55)),
                },
            ),
        ],
        "the artist Workspace's playing row lights; the other keeps its derived state"
    );
}
