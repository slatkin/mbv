use self::artist_workspace::artist_workspace_owner;
use self::tree_fixtures::{press, tree_owner};
use super::*;
use crate::app::render::LibraryListRenderCtx;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use tuirealm::component::Component;

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

    assert!(owner
        .on_filter_key(&KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        })
        .is_some());
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
        vec![crate::app::state::music_grouping::ArtistKey::Fallback(
            "Artist".into(),
        )],
        vec![0],
        None,
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
            Some(mbv_core::config::SelectorIdentity::Emby {
                key: mbv_core::config::EmbySelectorKey::Group("group-stable".into()),
            }),
            Some(mbv_core::config::LibraryItemIdentity::Emby {
                id: "album-stable".into(),
            }),
        )
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
        vec![crate::app::state::music_grouping::ArtistKey::Fallback(
            "Artist".into(),
        )],
        vec![0],
        None,
    ));
    let state = mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(mbv_core::config::SelectorIdentity::Emby {
            key: mbv_core::config::EmbySelectorKey::Latest,
        }),
        item: Some(mbv_core::config::LibraryItemIdentity::Emby {
            id: "stale-latest-item".into(),
        }),
    };
    assert!(owner.reanchor_launch_state(&state));
    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(mbv_core::config::SelectorIdentity::Emby {
                key: mbv_core::config::EmbySelectorKey::Group("group-stable".into()),
            }),
            Some(mbv_core::config::LibraryItemIdentity::Emby {
                id: "album-stable".into()
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
    use crate::app::state::music_artist_detail::{
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
