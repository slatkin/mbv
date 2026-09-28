use self::artist_workspace::artist_workspace_owner;
use self::tree_fixtures::{press, tree_owner};
use super::*;
use crate::list::tree_browser::TreeConsumed;
use mbv_emby_model::test_support::make_item;
use mbv_render::LibraryListRenderCtx;

#[test]
fn filter_escape_closes_search_and_tree_navigation_returns_selection() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    owner.inline_search.open();

    assert_eq!(
        owner.on_filter_key(&KeyEvent {
            code: Key::Esc,
            modifiers: KeyModifiers::NONE,
        }),
        None
    );
    assert!(!owner.inline_search.is_active());

    assert!(
        owner
            .on_filter_key(&KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            })
            .is_some()
    );
}

#[test]
fn grouped_music_launch_snapshot_uses_group_and_tree_target_identities() {
    let mut album = make_item("Album", "Folder");
    album.id = "album-stable".into();
    let mut group = make_item("Artist", "MusicArtist");
    group.id = "group-stable".into();
    let mut owner = MusicContent::new();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![album], 0),
        None,
        String::new(),
        vec![group],
        0,
        vec![("Artist".into(), "2024".into(), "Album".into())],
        vec![mbv_ui_model::music_grouping::ArtistKey::Fallback(
            "Artist".into(),
        )],
        vec![0],
        None,
    ));
    // Anchoring never expands (product rule): expand the album's artist root
    // first, as if the user had already opened it, so the album itself is
    // visible and actually gets selected.
    owner.browser.apply(TreeOperation::ToggleExpansionTarget(
        MusicTreeTarget::Artist(mbv_ui_model::music_grouping::ArtistKey::Fallback(
            "Artist".into(),
        )),
    ));
    assert_eq!(
        owner
            .browser
            .apply(TreeOperation::AnchorSelection {
                target: MusicTreeTarget::Album("album-stable".into()),
                flow_offset: 0,
            })
            .disposition,
        TreeConsumed::Consumed
    );

    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(mbv_config::SelectorIdentity::Emby {
                key: mbv_config::EmbySelectorKey::Group("group-stable".into()),
            }),
            Some(mbv_config::LibraryItemIdentity::Emby {
                id: "album-stable".into(),
            }),
        )
    );
}

// Regression for the music-tree-reverts-on-restart bug: an artist root
// never writes the ordinary album-persistence request (task 2.2), so
// without the artist fallback in `launch_snapshot`, quitting while an
// artist row is focused saved no item at all, and the next launch's
// `reanchor_launch_state` fell back to the group's default first album
// instead of the artist the user actually left selected.
#[test]
fn grouped_music_launch_snapshot_falls_back_to_focused_artist_when_no_album_selected() {
    let mut album = make_item("Album", "Folder");
    album.id = "album-stable".into();
    let mut group = make_item("Artist", "MusicArtist");
    group.id = "group-stable".into();
    let mut owner = MusicContent::new();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![album], 0),
        None,
        String::new(),
        vec![group],
        0,
        vec![("Artist".into(), "2024".into(), "Album".into())],
        vec![mbv_ui_model::music_grouping::ArtistKey::Service(
            "artist-service-id".into(),
        )],
        vec![0],
        None,
    ));
    assert_eq!(
        owner
            .browser
            .apply(TreeOperation::AnchorSelection {
                target: MusicTreeTarget::Artist(mbv_ui_model::music_grouping::ArtistKey::Service(
                    "artist-service-id".into(),
                )),
                flow_offset: 0,
            })
            .disposition,
        TreeConsumed::Consumed
    );

    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(mbv_config::SelectorIdentity::Emby {
                key: mbv_config::EmbySelectorKey::Group("group-stable".into()),
            }),
            Some(mbv_config::LibraryItemIdentity::Emby {
                id: "artist-service-id".into(),
            }),
        )
    );
}

// Companion restore side of the regression above: a saved artist id
// reselects the artist root itself (collapsed, no album expanded), instead
// of the pre-fix behaviour of falling through to the default first album.
#[test]
fn reanchor_launch_state_restores_focused_artist_without_expanding_default_album() {
    let mut album = make_item("Album", "Folder");
    album.id = "album-stable".into();
    let mut group = make_item("Artist", "MusicArtist");
    group.id = "group-stable".into();
    let mut owner = MusicContent::new();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![album], 0),
        None,
        String::new(),
        vec![group],
        0,
        vec![("Artist".into(), "2024".into(), "Album".into())],
        vec![mbv_ui_model::music_grouping::ArtistKey::Service(
            "artist-service-id".into(),
        )],
        vec![0],
        None,
    ));
    let state = mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::Home,
        panel_focus: mbv_config::LaunchPanelFocus::Library,
        selector: Some(mbv_config::SelectorIdentity::Emby {
            key: mbv_config::EmbySelectorKey::Group("group-stable".into()),
        }),
        item: Some(mbv_config::LibraryItemIdentity::Emby {
            id: "artist-service-id".into(),
        }),
    };

    assert!(owner.reanchor_launch_state(&state));

    assert!(owner.selected_is_artist());
    assert_eq!(owner.selected_album_target(), None);
}

// Startup regression (real repro): quit with the Jazz pill selected and its
// first artist, a Fallback-keyed root (no stable Service id, the common case
// for this library), selected but collapsed. On relaunch the saved pill plus
// saved artist item must restore exactly that: the artist selected and still
// collapsed, and nothing else in the group expanded -- not the saved
// artist's own first album, and not any other artist's.
#[test]
fn launch_restore_selects_the_saved_artist_collapsed_and_expands_nothing() {
    let mut first_album = make_item("First Artist Album", "Folder");
    first_album.id = "first-artist-album".into();
    let mut second_album = make_item("Second Artist Album", "Folder");
    second_album.id = "second-artist-album".into();
    let mut group = make_item("Jazz", "MusicArtist");
    group.id = "jazz-group".into();
    let mut owner = MusicContent::new();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![first_album, second_album], 0),
        None,
        String::new(),
        vec![group],
        0,
        vec![
            (
                "First Artist".into(),
                "2001".into(),
                "First Artist Album".into(),
            ),
            (
                "Second Artist".into(),
                "2002".into(),
                "Second Artist Album".into(),
            ),
        ],
        vec![
            mbv_ui_model::music_grouping::ArtistKey::Fallback("First Artist".into()),
            mbv_ui_model::music_grouping::ArtistKey::Fallback("Second Artist".into()),
        ],
        vec![0, 1],
        None,
    ));
    let state = mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::Home,
        panel_focus: mbv_config::LaunchPanelFocus::Library,
        selector: Some(mbv_config::SelectorIdentity::Emby {
            key: mbv_config::EmbySelectorKey::Group("jazz-group".into()),
        }),
        item: Some(mbv_config::LibraryItemIdentity::Emby {
            id: "First Artist".into(),
        }),
    };

    assert!(owner.reanchor_launch_state(&state));

    assert!(owner.selected_is_artist(), "the saved artist is selected");
    assert_eq!(
        owner.browser.selected_target().cloned(),
        Some(MusicTreeTarget::Artist(
            mbv_ui_model::music_grouping::ArtistKey::Fallback("First Artist".into())
        ))
    );
    assert!(
        owner.browser.expanded.is_empty(),
        "no artist root -- saved or otherwise -- is expanded by a programmatic restore"
    );
}

#[test]
fn saved_music_latest_selector_falls_back_to_normal_default() {
    let mut album = make_item("Album", "Folder");
    album.id = "album-stable".into();
    let mut group = make_item("Artist", "MusicArtist");
    group.id = "group-stable".into();
    let mut owner = MusicContent::new();
    owner.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![album], 0),
        None,
        String::new(),
        vec![group],
        0,
        vec![("Artist".into(), "2024".into(), "Album".into())],
        vec![mbv_ui_model::music_grouping::ArtistKey::Fallback(
            "Artist".into(),
        )],
        vec![0],
        None,
    ));
    let state = mbv_config::TuiLaunchState {
        version: mbv_config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_config::TabIdentity::Home,
        panel_focus: mbv_config::LaunchPanelFocus::Library,
        selector: Some(mbv_config::SelectorIdentity::Emby {
            key: mbv_config::EmbySelectorKey::Latest,
        }),
        item: Some(mbv_config::LibraryItemIdentity::Emby {
            id: "stale-latest-item".into(),
        }),
    };
    assert!(owner.reanchor_launch_state(&state));
    // The default fallback selects the first visible row (the collapsed
    // artist root) rather than expanding into its first album (product
    // rule: a programmatic selection never changes expansion), so the
    // snapshot carries the artist's own fallback-name identity.
    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(mbv_config::SelectorIdentity::Emby {
                key: mbv_config::EmbySelectorKey::Group("group-stable".into()),
            }),
            Some(mbv_config::LibraryItemIdentity::Emby {
                id: "Artist".into(),
            }),
        )
    );
    assert!(owner.launch_selector(&state).is_none());
}

#[test]
fn music_selector_contains_only_groups_and_switches_by_group_index() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"]), ("Beta", &["b-0"])]);
    let mut alpha = make_item("Alpha", "MusicArtist");
    alpha.id = "alpha-group".into();
    let mut beta = make_item("Beta", "MusicArtist");
    beta.id = "beta-group".into();
    owner.context.groups = vec![alpha, beta];
    let content = owner.panel_content();
    let selector = content.selector.as_ref().expect("group selector");
    assert_eq!(selector.pills, vec!["Alpha", "Beta"]);
    assert_eq!(selector.markers, vec![false, false]);
    assert_eq!(selector.active, Some(0));
    assert!(matches!(content.list, ListSlot::Media(_)));
    drop(content);

    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::SelectorPicked(1)),
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::MusicGroupSwitch { delta: 1 })));
}

/// Task 6.4: a projected artist detail that no longer belongs to the tree's
/// current root never paints its summary, artwork, or groups — the Hero is
/// absent rather than stale.
#[test]
fn a_stale_artist_detail_never_paints_under_the_new_root() {
    use mbv_ui_model::music_artist_detail::{
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
    };
    assert!(
        owner.track_list.rows().is_empty(),
        "no stale Beta group rows paint under Alpha"
    );
    assert!(
        owner.workspace_track_item("beta-track").is_none(),
        "the stale track is not resolvable for activation"
    );
}

#[cfg(test)]
mod tree_fixtures;

#[cfg(test)]
mod tree;

#[cfg(test)]
mod artist_workspace;

#[cfg(test)]
mod artist_actions;
