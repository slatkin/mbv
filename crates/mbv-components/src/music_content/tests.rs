use self::artist_workspace::artist_workspace_owner;
use self::tree_fixtures::{press, tree_owner};
use super::*;
use crate::list::tree_browser::TreeConsumed;
use mbv_emby_model::test_support::make_item;
use mbv_render::LibraryListRenderCtx;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use tuirealm::component::Component;

/// Complete one painted tree frame so the wheel resolves its claim and its
/// reach from retained geometry, mirroring the shared tree-owner paint seam.
fn paint_tree(owner: &mut MusicContent, height: u16) {
    let mut terminal = Terminal::new(TestBackend::new(20, height)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut owner.browser, frame, Rect::new(0, 0, 18, height)))
        .unwrap();
}

/// Wheel-scrolls-viewport task 4.2 (D4): the Grouped Music wheel scrolls the
/// tree viewport, never the selection, and reports the album display index
/// of the last painted album-or-later row as the shell reach request.
#[test]
fn wheel_scrolls_the_viewport_and_reports_the_last_painted_album_reach() {
    // The uniform wheel step is three rows (mouse-input wheel contract).
    const WHEEL_STEP: i64 = 3;
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1", "a-2", "a-3"])]);
    // Expand the single artist root: the settled flow is the root plus four
    // album leaves, and the selection rests on the first visible node.
    owner.browser.apply(TreeOperation::ToggleExpansionTarget(
        MusicTreeTarget::Artist(mbv_ui_model::music_grouping::ArtistKey::Service(
            "artist-Alpha".into(),
        )),
    ));
    let selection_before = owner.browser.selected_target().cloned();
    // A three-row viewport: before the wheel the window holds the root and
    // the first two albums.
    paint_tree(&mut owner, 3);

    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at: Position::new(1, 1),
        delta: WHEEL_STEP,
    }));

    // After the step the painted window holds a-2 and a-3: the last painted
    // album-or-later row is a-3, album display index 3 — not a cursor move.
    assert_eq!(
        message,
        Some(Msg::Shell(Box::new(ShellRequest::LibraryViewportReach {
            index: 3
        })))
    );
    assert_eq!(
        owner.browser.selected_target().cloned(),
        selection_before,
        "the wheel never moves the selected node"
    );
}

/// Wheel-scrolls-viewport task 4.2: the tree spec's claimed-wheel focus
/// contract holds for a painted window whose rows carry no album-or-later
/// node (a collapsed artist root also clamps the wheel at the boundary) — the
/// focus-only request takes the reach report's place.
#[test]
fn wheel_over_a_painted_artist_root_still_focuses_library() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"])]);
    // The artist root is collapsed: the only painted node is the root,
    // which resolves to no album display index.
    let selection_before = owner.browser.selected_target().cloned();
    paint_tree(&mut owner, 2);

    let message = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at: Position::new(1, 0),
        delta: 3,
    }));

    assert_eq!(
        message,
        Some(Msg::Shell(Box::new(ShellRequest::LibraryPanelFocus)))
    );
    assert_eq!(
        owner.browser.selected_target().cloned(),
        selection_before,
        "the wheel never moves the selected node"
    );
}

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

    // The group pill persists; the selected tree node never does.
    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(mbv_config::SelectorIdentity::Emby {
                key: mbv_config::EmbySelectorKey::Group("group-stable".into()),
            }),
            None,
        )
    );
}

// Companion restore side of the music-tree-reverts-on-restart regression:
// restore reselects the artist root itself (collapsed, no album expanded),
// instead of the pre-fix behaviour of falling through to the default first
// album. A legacy snapshot's saved artist item decodes but is ignored (spec:
// every restart lands on the first row).
#[test]
fn reanchor_launch_state_selects_the_first_artist_root_without_expanding() {
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
// for this library), selected but collapsed. Restore selects the first
// artist root and still collapsed, and nothing in the group is expanded --
// not the first artist's own first album, and not any other artist's. A
// legacy snapshot's saved artist item decodes but is ignored (spec: every
// restart lands on the first row).
#[test]
fn launch_restore_selects_the_first_artist_collapsed_and_expands_nothing() {
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

    assert!(
        owner.selected_is_artist(),
        "the first artist root is selected"
    );
    assert_eq!(
        owner.browser.selected_target().cloned(),
        Some(MusicTreeTarget::Artist(
            mbv_ui_model::music_grouping::ArtistKey::Fallback("First Artist".into())
        ))
    );
    assert!(
        owner
            .browser
            .roots()
            .into_iter()
            .all(|root| !owner.browser.is_expanded(root)),
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
    // rule: a programmatic selection never changes expansion). The snapshot
    // carries the group pill and never an item.
    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(mbv_config::SelectorIdentity::Emby {
                key: mbv_config::EmbySelectorKey::Group("group-stable".into()),
            }),
            None,
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
