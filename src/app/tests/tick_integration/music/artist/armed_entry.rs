use super::super::landing::{draw_music_frame, mounted_music_app_at, music_panel, tick_key};
use super::super::*;
use super::{arrive_album_tracks, two_artist_app};

/// Task 6.4: a Wide artist-Workspace entry armed before the rows arrived
/// takes the cursor when its own root's rows land — and only then.
#[test]
fn armed_artist_workspace_entry_takes_the_cursor_when_its_own_rows_arrive() {
    let (mut harness, _id) = mounted_music_app_at(two_artist_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    assert!(harness.model().test_music_owner().selected_is_artist());

    // Right on the expanded root with no rows: the entry arms, the cursor
    // stays with the tree.
    tick_key(&mut harness, Key::Right);
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "an armed entry does not take the cursor before its rows arrive"
    );
    assert!(
        harness
            .model()
            .test_music_owner()
            .pending_artist_workspace_focus_for_test(),
        "the entry is armed for the focused root"
    );

    // The armed root's own rows land: the entry takes the cursor once.
    arrive_album_tracks(&mut harness, "album-1", "alpha-track-1");
    assert!(
        harness.model().test_music_owner().track_focused(),
        "the armed entry takes the cursor when its own root's rows arrive"
    );
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        2,
        "the focused Workspace holds the armed root's own heading and track"
    );
}

/// Task 6.4: the armed entry carries the root it was armed on. A push for
/// another root never takes the cursor, returning to the armed root does not
/// resurrect the voided entry, and a fresh Right enters the Workspace again.
#[test]
fn armed_artist_workspace_entry_does_not_seize_focus_for_another_root() {
    let (mut harness, _id) = mounted_music_app_at(two_artist_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    tick_key(&mut harness, Key::Right);
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "the entry is armed on the Alpha root with no rows"
    );

    // The selection leaves Alpha for the Beta root; the entry voids with it,
    // and Beta's rows then land.
    tick_key(&mut harness, Key::End);
    tick_key(&mut harness, Key::Left);
    assert!(
        !harness
            .model()
            .test_music_owner()
            .pending_artist_workspace_focus_for_test(),
        "leaving the armed root voids the entry"
    );
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .artist_detail_target()
            .map(|target| target.artist_name),
        Some("Beta".to_string()),
        "the selection moved to the Beta root"
    );
    arrive_album_tracks(&mut harness, "album-beta-1", "beta-track-1");
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "another root's arriving rows never take the cursor"
    );

    // Back on Alpha, its rows arrive too: the voided entry stays dead.
    tick_key(&mut harness, Key::Home);
    assert!(harness.model().test_music_owner().selected_is_artist());
    arrive_album_tracks(&mut harness, "album-1", "alpha-track-1");
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "returning to the armed root does not resurrect the voided entry"
    );

    // A fresh Right on the same root enters its now-resident Workspace.
    tick_key(&mut harness, Key::Right);
    assert!(
        harness.model().test_music_owner().track_focused(),
        "a fresh Right enters the artist Workspace"
    );
}

/// Task 6.4: the armed entry belongs to the Wide inline pane. A Narrow
/// transition voids it: arriving rows never take the cursor on the narrow
/// pane nothing paints, and the entry stays dead when the Wide pane returns
/// — a fresh Right is required to enter the Workspace again.
#[test]
fn armed_artist_workspace_entry_is_voided_by_a_narrow_transition() {
    let (mut harness, _id) = mounted_music_app_at(two_artist_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    tick_key(&mut harness, Key::Right);
    assert!(!harness.model().test_music_owner().track_focused());

    // Wide -> Narrow: the same owner survives, the inline pane does not, and
    // the armed entry is cleared with it.
    harness.model_mut().app.terminal_width = 60;
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel(&harness).test_narrow_geometry().is_some(),
        "the transition painted the narrow skeleton"
    );
    assert!(
        !harness
            .model()
            .test_music_owner()
            .pending_artist_workspace_focus_for_test(),
        "the Narrow transition clears the armed Wide entry"
    );

    // The armed root's rows land while Narrow, then a frame draws: neither
    // the push nor the draw may take the cursor on the pane nothing paints.
    arrive_album_tracks(&mut harness, "album-1", "alpha-track-1");
    draw_music_frame(&mut harness);
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "a voided entry never re-seizes the focus on the narrow pane"
    );

    // Back to Wide: the entry was voided by the transition, so even its own
    // now-resident rows do not take the cursor without a fresh Right.
    harness.model_mut().app.terminal_width = 160;
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel(&harness).test_wide_geometry().is_some(),
        "the return painted the wide skeleton"
    );
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "the entry voided by the Narrow transition stays dead in Wide"
    );
    tick_key(&mut harness, Key::Right);
    assert!(
        harness.model().test_music_owner().track_focused(),
        "a fresh Right still enters the now-resident artist Workspace"
    );
}
