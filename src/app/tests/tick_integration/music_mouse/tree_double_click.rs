use super::*;

/// D5/rows 5.2 and 5.5: a double-click on an expandable tree node — an artist
/// root or an album leaf with cached track children — toggles its persistent
/// expansion and opens no Hero, in Wide and non-Wide and both with and without
/// the local tree filter active. With the filter active the production Grouped
/// Music surface paints the tree and leaves the flat Inline Search result
/// carrier empty, so the gesture must resolve current-frame filtered tree
/// geometry and keep tree semantics.
#[rstest]
#[case::wide_unfiltered_artist(160, 40, false, TreeDoubleClickNode::ArtistRoot)]
#[case::wide_filtered_artist(160, 40, true, TreeDoubleClickNode::ArtistRoot)]
#[case::wide_unfiltered_album(160, 40, false, TreeDoubleClickNode::AlbumLeaf)]
#[case::wide_filtered_album(160, 40, true, TreeDoubleClickNode::AlbumLeaf)]
#[case::narrow_unfiltered_artist(81, 30, false, TreeDoubleClickNode::ArtistRoot)]
#[case::narrow_filtered_artist(81, 30, true, TreeDoubleClickNode::ArtistRoot)]
#[case::narrow_unfiltered_album(81, 30, false, TreeDoubleClickNode::AlbumLeaf)]
#[case::narrow_filtered_album(81, 30, true, TreeDoubleClickNode::AlbumLeaf)]
fn double_click_expands_artist_and_album_nodes_without_a_hero(
    #[case] width: u16,
    #[case] height: u16,
    #[case] filtered: bool,
    #[case] node: TreeDoubleClickNode,
) {
    let mut app = make_music_group_app();
    app.album_tracks_cache
        .insert("album-1".into(), vec![cached_track("track-1", 1)]);
    app.terminal_width = width;
    app.terminal_height = height;
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw_frame(&mut harness);

    let node_id = node.resolve(&harness);
    let was_expanded = harness
        .model()
        .test_music_owner()
        .browser
        .is_expanded(&node_id);

    // Open the production filter through the router, then feed the
    // destination's own query (the panel paints the tree, never a flat result
    // list). Filter-forced visibility never overwrites persistent expansion.
    if filtered {
        inject_key(&mut harness, Key::Char('/'));
        harness
            .model_mut()
            .test_music_owner_mut()
            .browser
            .apply(TreeOperation::EditFilter(node.filter_query().to_string()));
        draw_frame(&mut harness);
        assert!(
            harness
                .model()
                .test_music_owner()
                .inline_search
                .results_len()
                == 0,
            "{width}x{height}: production filtering leaves the flat carrier empty"
        );
    }

    // The first click resolves the row; the run loop paints the next frame
    // before the second press, as the real event loop does.
    let at = tree_node_point(&harness, &node_id);
    harness.inject(left_click(at.0, at.1));
    dispatch_step(&mut harness);
    draw_frame(&mut harness);
    harness.inject(left_click(at.0, at.1));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().all(|message| !matches!(
            message,
            Msg::Shell(ref shell_boxed | ref shell_boxed)
         if matches!(shell_boxed.as_ref(), ShellRequest::MusicAlbumActivate { .. } | ShellRequest::InlineSearchActivate { .. }))),
        "{width}x{height} {node:?} filtered={filtered}: double-click keeps tree semantics: {:?}",
        outcome.raw_messages
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert_ne!(
        harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&node_id),
        was_expanded,
        "{width}x{height} {node:?} filtered={filtered}: double-click toggles persistent expansion"
    );
    if filtered {
        assert!(
            harness.model().test_music_owner().browser.filter_active(),
            "{width}x{height}: the forced filtered projection stays until the filter closes"
        );
    }
    assert!(
        !music_panel(&harness).test_hero_overlay_open(),
        "{width}x{height} {node:?} filtered={filtered}: no Hero opens over the tree"
    );
}

/// Row 5.5: a double-click on an album leaf with no cached track children
/// claims the gesture without changing its expansion, opening a Hero, or
/// starting playback.
#[test]
fn double_click_a_childless_album_claims_the_gesture_unchanged() {
    let mut app = make_music_group_app_with_second_album();
    app.album_tracks_cache
        .insert("album-1".into(), vec![cached_track("track-1", 1)]);
    app.terminal_width = 160;
    app.terminal_height = 40;
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw_frame(&mut harness);

    let childless = album_target(&harness, "album-2");
    assert!(
        !harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&childless),
        "the childless leaf starts collapsed"
    );
    let at = tree_node_point(&harness, &childless);

    harness.inject(left_click(at.0, at.1));
    dispatch_step(&mut harness);
    draw_frame(&mut harness);
    harness.inject(left_click(at.0, at.1));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().all(|message| !matches!(
            message,
            Msg::Shell(ref shell_boxed)
         if matches!(shell_boxed.as_ref(), ShellRequest::MusicTreeTrackActivate { .. }))),
        "a childless album claims the gesture without a track activation: {:?}",
        outcome.raw_messages
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        !harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&childless),
        "the claimed childless leaf keeps its expansion state"
    );
    assert!(
        !music_panel(&harness).test_hero_overlay_open(),
        "a childless album double-click opens no Hero"
    );
    assert_eq!(playback_queue_ids(&harness), Vec::<String>::new());
    assert!(harness.model().app.pending_queue_replacement.is_none());
}
