use super::super::landing::{
    draw_music_frame, mounted_music_app_at, music_panel, music_panel_mut, tick_key,
};
use super::super::*;
use super::mounted_neighbour_app;
use crate::app::components::list::tree_browser::TreeOperation;

/// Task 6.4: Right on a collapsed artist root only expands it; a later Right
/// enters the artist Workspace — non-Wide through the Library Hero overlay,
/// whose artist Hero and grouped Workspace paint in the same push.
#[test]
fn right_on_a_collapsed_artist_root_expands_first_then_opens_its_workspace() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 81, 30);
    // The first Left leaves the adopted album leaf for Alpha's root and
    // requests its detail; the second collapses the expanded root.
    tick_key(&mut harness, Key::Left);
    assert!(
        harness.model().test_music_owner().selected_is_artist(),
        "Left moves the leaf to its artist parent"
    );
    tick_key(&mut harness, Key::Left);
    let root = harness
        .model()
        .test_music_owner()
        .browser
        .selected_target()
        .cloned()
        .expect("artist root selected");
    assert!(
        !harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&root),
        "Left collapses the expanded root"
    );

    // First Right: expansion only, no overlay.
    tick_key(&mut harness, Key::Right);
    assert!(
        harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&root),
        "the first Right expands the root"
    );
    assert!(
        !music_panel(&harness).test_hero_overlay_open(),
        "expanding must not open the overlay"
    );

    // Later Right: the artist Workspace opens in the Library Hero overlay.
    tick_key(&mut harness, Key::Right);
    assert!(
        music_panel(&harness).test_hero_overlay_open(),
        "the later Right opens the artist Library Hero overlay"
    );
    let hero = music_panel_mut(&mut harness)
        .active_hero_data()
        .expect("the overlay paints the artist Hero");
    assert_eq!(hero.facts.title, "Alpha");
    assert_eq!(
        hero.facts.meta_rows,
        vec!["5 albums".to_string(), "2001".to_string()]
    );
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        10,
        "five album headings plus five grouped track rows"
    );
}

/// Task 6.4: in Wide geometry the later Right enters the inline artist-track
/// Workspace locally, with no overlay and no request.
#[test]
fn right_on_an_expanded_artist_root_enters_the_wide_workspace() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    assert!(harness.model().test_music_owner().selected_is_artist());
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "the tree rail still owns the focus"
    );

    tick_key(&mut harness, Key::Right);
    assert!(
        harness.model().test_music_owner().track_focused(),
        "Wide Right takes the inline artist Workspace's cursor"
    );
    assert!(!music_panel(&harness).test_hero_overlay_open());
}

/// Enter on an unfiltered artist root uses the same Wide Workspace entry as
/// Right, without changing the tree expansion or pane placement.
#[test]
fn enter_on_an_unfiltered_artist_root_enters_wide_workspace_without_relayout() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    let root = harness
        .model()
        .test_music_owner()
        .browser
        .selected_target()
        .cloned()
        .expect("artist root selected");
    let expanded = harness
        .model()
        .test_music_owner()
        .browser
        .is_expanded(&root);
    let before = music_panel(&harness)
        .test_wide_geometry()
        .expect("Wide geometry")
        .clone();

    tick_key(&mut harness, Key::Enter);

    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&root),
        expanded,
        "Enter does not toggle the artist root"
    );
    assert!(
        harness.model().test_music_owner().track_focused(),
        "Enter focuses the artist Workspace"
    );
    assert!(!music_panel(&harness).test_hero_overlay_open());
    let after = music_panel(&harness)
        .test_wide_geometry()
        .expect("Wide geometry")
        .clone();
    assert_eq!(
        (
            before.browser,
            before.hero,
            before.list_panel,
            before.list_area
        ),
        (after.browser, after.hero, after.list_panel, after.list_area),
        "Workspace entry preserves pane geometry"
    );
}

/// Non-Wide Enter opens the Library Hero overlay and focuses the artist
/// Workspace through the panel's composed overlay path.
#[test]
fn enter_on_an_unfiltered_artist_root_opens_the_non_wide_hero_workspace() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 81, 30);
    tick_key(&mut harness, Key::Left);
    let root = harness
        .model()
        .test_music_owner()
        .browser
        .selected_target()
        .cloned()
        .expect("artist root selected");
    let expanded = harness
        .model()
        .test_music_owner()
        .browser
        .is_expanded(&root);

    tick_key(&mut harness, Key::Enter);

    assert!(
        music_panel(&harness).test_hero_overlay_open(),
        "non-Wide Enter opens the artist Hero overlay"
    );
    assert!(
        harness.model().test_music_owner().track_focused(),
        "the artist Workspace receives focus"
    );
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&root),
        expanded,
        "Hero entry does not toggle expansion"
    );
}

#[test]
fn artist_library_hero_track_activation_emits_artist_track_intent() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 81, 30);
    tick_key(&mut harness, Key::Left);
    tick_key(&mut harness, Key::Enter);
    assert!(music_panel(&harness).test_hero_overlay_open());
    assert!(harness.model().test_music_owner().track_focused());

    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| {
        matches!(
            message,
            Msg::Shell(ref shell_boxed)
         if matches!(shell_boxed.as_ref(), ShellRequest::MusicArtistTrackActivate { .. }))
    }));
}

/// A filtered artist root keeps Enter local in both the Wide and non-Wide
/// compositions; the panel must not open an overlay before the owner sees it.
#[test]
fn enter_on_a_filtered_artist_root_toggles_locally_in_wide_and_non_wide() {
    for (width, height) in [(160, 40), (81, 30)] {
        let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), width, height);
        tick_key(&mut harness, Key::Left);
        let root = harness
            .model()
            .test_music_owner()
            .browser
            .selected_target()
            .cloned()
            .expect("artist root selected");
        assert!(harness
            .model()
            .test_music_owner()
            .browser
            .is_expanded(&root));

        tick_key(&mut harness, Key::Char('/'));
        harness
            .model_mut()
            .test_music_owner_mut()
            .browser
            .apply(TreeOperation::EditFilter("Alpha".to_string()));
        draw_music_frame(&mut harness);
        tick_key(&mut harness, Key::Enter);

        assert!(
            !harness
                .model()
                .test_music_owner()
                .browser
                .is_expanded(&root),
            "{width}x{height}: filtered Enter toggles the root locally"
        );
        assert!(
            !music_panel(&harness).test_hero_overlay_open(),
            "{width}x{height}: filtered Enter does not open a Hero"
        );
        assert!(
            !harness.model().test_music_owner().track_focused(),
            "{width}x{height}: filtered Enter does not focus a Workspace"
        );
    }
}

/// Task 6.4: the Wide Hero and its Workspace switch atomically between the
/// album and artist arms — the title, facts, and rows never come from
/// different selections.
#[test]
fn artist_and_album_hero_workspaces_switch_atomically_in_wide() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 160, 40);

    {
        let album = music_panel_mut(&mut harness)
            .active_hero_data()
            .expect("album hero");
        assert_eq!(album.facts.title, "Album 1");
        assert_eq!(
            album.facts.meta_rows,
            vec!["Alpha".to_string(), "2001".to_string()]
        );
    }
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        1,
        "the album Workspace paints that album's track"
    );

    tick_key(&mut harness, Key::Left);
    {
        let artist = music_panel_mut(&mut harness)
            .active_hero_data()
            .expect("artist hero");
        assert_eq!(artist.facts.title, "Alpha");
        assert_eq!(
            artist.facts.meta_rows,
            vec!["5 albums".to_string(), "2001".to_string()]
        );
    }
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        10,
        "the artist Workspace replaces the album rows in the same push"
    );
    assert!(matches!(
        harness.model().test_music_owner().track_list.rows().first(),
        Some(crate::app::components::media_list::MediaListRow::Heading { text }) if text == "Album 1"
    ));

    tick_key(&mut harness, Key::Down);
    {
        let album = music_panel_mut(&mut harness)
            .active_hero_data()
            .expect("album hero returns");
        assert_eq!(album.facts.title, "Album 1");
    }
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        1
    );
}
