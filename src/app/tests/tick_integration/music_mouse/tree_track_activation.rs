use super::*;

/// The painted point of the `make_music_group_app` album leaf's cached track
/// row, resolved from the tree's completed frame through the shared read-only
/// stable-target row geometry.
fn track_point(harness: &TickHarness, track: &str) -> (u16, u16) {
    let music = harness.model().test_music_owner();
    let node = music
        .browser
        .visible_targets()
        .into_iter()
        .find(|candidate| {
            matches!(candidate,
                MusicTreeTarget::Track { album, track: track_target }
                    if album == "album-1" && track_target == track)
        })
        .expect("painted track node");
    let row = music
        .browser
        .row_rect_for(&node)
        .expect("painted track row");
    (row.x, row.y)
}

/// Rows 5.2/5.3 end to end through the mounted composition: the tree's track
/// Enter chord and its track double-click both cross as the stable-identity
/// play-now intent, and the shell's one grouped-track resolver feeds the album
/// queue through the existing executor with the selected start index.
#[test]
fn music_tree_track_activation_plays_through_the_grouped_resolver() {
    let mut harness = expanded_music_tree_track_harness(true);

    // Enter on the selected tree track: the router reaches the owner's
    // track arm, whose request the shell resolves.
    let first_track = track_point(&harness, "track-1");
    harness.inject(left_click(first_track.0, first_track.1));
    dispatch_step(&mut harness);
    draw_frame(&mut harness);
    inject_key(&mut harness, Key::Enter);
    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["track-1", "track-2"],
        "autoload on queues the cached album from the selected tree track"
    );
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 0);

    // Double-click on the second track: the same resolver starts at that
    // track's own index. The run loop repaints between the two presses, as
    // the real event loop does. The previous activation left a populated
    // target queue, so row 5.4's gate raises the replacement confirmation and
    // only the confirmed action executes.
    let second_track = track_point(&harness, "track-2");
    harness.inject(left_click(second_track.0, second_track.1));
    dispatch_step(&mut harness);
    draw_frame(&mut harness);
    harness.inject(left_click(second_track.0, second_track.1));
    dispatch_step(&mut harness);
    assert!(
        confirm_mounted(&harness),
        "a populated target queue asks before the replacement"
    );
    assert_eq!(
        harness.model().app.playback_queue().queue_cursor,
        0,
        "the queue is unchanged before the confirmation"
    );
    inject_key(&mut harness, Key::Char('y'));
    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["track-1", "track-2"]
    );
    assert_eq!(
        harness.model().app.playback_queue().queue_cursor,
        1,
        "the double-clicked track is the start index after confirmation"
    );
}

/// Row 5.5: tree-track Enter feeds the one grouped resolver, whose autoload
/// policy decides whether the replacement queue is the whole cached album
/// (starting at the selected track) or only the selected Audio item.
#[rstest]
#[case::autoload_on(true, &["track-1", "track-2"], 1)]
#[case::autoload_off(false, &["track-2"], 0)]
fn tree_track_enter_follows_the_autoload_policy(
    #[case] autoload: bool,
    #[case] expected: &[&str],
    #[case] start: usize,
) {
    let mut harness = expanded_music_tree_track_harness(autoload);
    assert_eq!(
        harness.model().app.playback_queue().total_queue_len(),
        0,
        "the target queue starts empty, so the replacement needs no prompt"
    );

    let at = track_point(&harness, "track-2");
    activate_track(&mut harness, at, TrackActivation::Enter);

    assert!(!confirm_mounted(&harness));
    assert_eq!(playback_queue_ids(&harness), expected);
    assert_eq!(harness.model().app.playback_queue().queue_cursor, start);
}

/// Row 5.5: with an empty target queue both tree-track activation routes play
/// immediately, with no confirmation prompt.
#[rstest]
#[case::enter(TrackActivation::Enter)]
#[case::double_click(TrackActivation::DoubleClick)]
fn tree_track_activation_with_an_empty_queue_plays_immediately(#[case] kind: TrackActivation) {
    let mut harness = expanded_music_tree_track_harness(true);
    let at = track_point(&harness, "track-2");

    activate_track(&mut harness, at, kind);

    assert!(
        !confirm_mounted(&harness),
        "{kind:?}: an empty queue starts playback without a prompt"
    );
    assert_eq!(playback_queue_ids(&harness), ["track-1", "track-2"]);
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 1);
}

/// Row 5.5: with a populated target queue both tree-track activation routes
/// raise the replacement confirmation, change nothing before it, and only
/// play after the complete confirmation sequence.
#[rstest]
#[case::enter(TrackActivation::Enter)]
#[case::double_click(TrackActivation::DoubleClick)]
fn tree_track_activation_with_a_populated_queue_confirms_before_replacing(
    #[case] kind: TrackActivation,
) {
    let mut app = music_tree_track_app(true);
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    let mut harness = expanded_track_harness(app);

    let at = track_point(&harness, "track-2");
    activate_track(&mut harness, at, kind);

    assert!(
        confirm_mounted(&harness),
        "{kind:?}: a populated target queue asks before the replacement"
    );
    assert_eq!(
        playback_queue_ids(&harness),
        ["existing"],
        "{kind:?}: the prompt changes no queue"
    );
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 0);
    assert!(harness.model().app.pending_queue_replacement.is_some());

    inject_key(&mut harness, Key::Char('y'));

    assert!(!confirm_mounted(&harness));
    assert!(harness.model().app.pending_queue_replacement.is_none());
    assert_eq!(playback_queue_ids(&harness), ["track-1", "track-2"]);
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 1);
}

/// Rows 5.4/5.5: cancelling the populated-queue confirmation leaves the queue
/// and playback unchanged, clears the pending payload, and a later sync pass
/// does not resurrect it.
#[test]
fn cancelling_the_replace_queue_confirmation_leaves_the_queue_unchanged() {
    let mut app = music_tree_track_app(true);
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    let mut harness = expanded_track_harness(app);

    let at = track_point(&harness, "track-2");
    activate_track(&mut harness, at, TrackActivation::Enter);
    assert!(confirm_mounted(&harness));

    inject_key(&mut harness, Key::Esc);

    assert!(!confirm_mounted(&harness));
    assert!(
        harness.model().app.pending_queue_replacement.is_none(),
        "cancellation leaves no executable payload behind"
    );
    assert_eq!(playback_queue_ids(&harness), ["existing"]);
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 0);

    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(playback_queue_ids(&harness), ["existing"]);
    assert_eq!(harness.model().app.playback_queue().queue_cursor, 0);
}
