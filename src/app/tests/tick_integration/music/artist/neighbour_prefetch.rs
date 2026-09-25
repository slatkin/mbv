use super::super::landing::{draw_music_frame, mounted_music_app_at, music_panel_mut};
use super::super::*;
use super::mounted_neighbour_app;
use crate::app::components::list::tree_browser::{TreeConsumed, TreeOperation};

/// Task 6.5 (design D4): the tree resolves the neighbour window from its
/// completed paint and emits the typed payload in visible order in both
/// presentations; the shell re-resolves no cursor.
#[test]
fn neighbour_prefetch_payload_is_the_painted_trees_order_in_both_presentations() {
    for (width, height) in [(160u16, 40u16), (81, 30)] {
        let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), width, height);
        // The production loop drains the panel's post-paint message before the
        // next frame; clear the adopt-frame request so this test observes the
        // payload for the selection it makes.
        let (mut music_resize, mut tv_resize) = (false, false);
        harness
            .model_mut()
            .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
        assert_eq!(
            harness
                .model_mut()
                .test_music_owner_mut()
                .browser
                .apply(TreeOperation::AnchorSelection {
                    target: crate::app::components::music_tree_target::MusicTreeTarget::Album(
                        "album-3".into(),
                    ),
                    flow_offset: 0,
                })
                .disposition,
            TreeConsumed::Consumed,
            "{width}x{height}: the fixture interns album-3"
        );
        harness.model_mut().sync_mounted_surfaces();
        assert_eq!(
            harness
                .model()
                .test_music_owner()
                .browser
                .selected_target()
                .and_then(|target| target.album_leaf_target()),
            Some("album-3"),
            "{width}x{height}: the selection survives the sync"
        );
        draw_music_frame(&mut harness);

        match music_panel_mut(&mut harness).take_deferred_msg() {
            Some(Msg::Shell(shell_boxed)) => {
                let ShellRequest::MusicNeighbourPrefetch { targets, .. } = *shell_boxed else {
                    panic!("{width}x{height}: expected the neighbour request, got {shell_boxed:?}")
                };
                assert_eq!(
                    targets,
                    vec![
                        "album-2".to_string(),
                        "album-4".to_string(),
                        "album-5".to_string(),
                    ],
                    "{width}x{height}: one behind and three ahead over the visible leaves"
                );
            }
            other => panic!("{width}x{height}: expected the neighbour request, got {other:?}"),
        }
    }
}

/// Task 6.5: the shell applies the existing idle gate to the typed targets —
/// a closed gate suppresses every fetch — and an artist-root focus ships no
/// request at all.
#[test]
fn neighbour_prefetch_is_idle_gated_and_suppressed_on_an_artist_root() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 160, 40);
    let (mut music_resize, mut tv_resize) = (false, false);
    // Clear the adopt-frame request (the production loop drains between
    // frames), then select the album whose window this test asserts.
    harness
        .model_mut()
        .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
    assert_eq!(
        harness
            .model_mut()
            .test_music_owner_mut()
            .browser
            .apply(TreeOperation::AnchorSelection {
                target: crate::app::components::music_tree_target::MusicTreeTarget::Album(
                    "album-3".into(),
                ),
                flow_offset: 0,
            })
            .disposition,
        TreeConsumed::Consumed
    );
    harness.model_mut().sync_mounted_surfaces();
    // Drop the selected hero's own non-idle fetch so the assertions isolate
    // the neighbour window's reservations.
    harness.model_mut().app.card_image_loading.clear();
    draw_music_frame(&mut harness);
    harness
        .model_mut()
        .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
    for key in ["album-2:P", "album-4:P", "album-5:P"] {
        assert!(
            harness.model().app.card_image_loading.contains(key),
            "{key} prefetches while the idle gate is open"
        );
    }
    assert!(
        !harness.model().app.card_image_loading.contains("album-1:P"),
        "a leaf outside the window is not prefetched"
    );

    // A fresh navigation closes the idle gate: the tree still resolves and
    // emits the window, but the shell makes no reservation for it.
    harness.model_mut().app.card_image_loading.clear();
    harness.model_mut().app.last_nav_at = std::time::Instant::now();
    draw_music_frame(&mut harness);
    let before = harness.model().app.card_image_fetch_calls;
    let targets = match music_panel_mut(&mut harness).take_deferred_msg() {
        Some(Msg::Shell(shell_boxed)) => match *shell_boxed {
            ShellRequest::MusicNeighbourPrefetch { targets, .. } => targets,
            other => panic!("expected the neighbour request, got {other:?}"),
        },
        other => panic!("expected the neighbour request, got {other:?}"),
    };
    harness.model_mut().handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::MusicNeighbourPrefetch { targets })),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(
        harness.model().app.card_image_fetch_calls,
        before,
        "the shell's idle gate makes no fetch reservation"
    );
    assert!(harness.model().app.card_image_loading.is_empty());

    // An artist-root focus suppresses the artwork window entirely, so the
    // tree posts no post-paint payload at all (source pagination for this
    // album level is unconditional and no longer rides this message).
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .apply(crate::app::components::list::tree_browser::TreeOperation::First);
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel_mut(&mut harness).take_deferred_msg().is_none(),
        "an artist-root focus ships no post-paint payload"
    );
}
