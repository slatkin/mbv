//! Grouped Music tree pointer gestures: click, wheel, double-click, and
//! context-click all resolve the latest painted frame's retained geometry, and
//! the double-click dispatch is by resolved node kind.

use super::tree_fixtures::{
    find, paint_tree, press, tree_owner, tree_owner_with_tracks, tree_point,
};
use super::*;
use crate::app::components::music_tree_target::MusicTreeTarget;
use rstest::rstest;

#[test]
fn hero_double_click_activates_the_selected_track() {
    let album = make_item("Album", "MusicAlbum");
    let track = make_item("Track", "Audio");
    let mut owner = MusicContent::new();
    let mut ctx = context(album.clone(), "overview");
    ctx.album_tracks = Some(vec![track.clone()]);
    owner.set_content(ctx);

    let area = Rect::new(0, 0, 30, 1);
    owner.track_list.wide_mut().set_geometry(area, area);
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|frame| owner.track_list.wide_mut().view(frame, area))
        .unwrap();

    let message = owner.on_slot_event(LibrarySlotEvent::HeroPane(
        MediaListSurfaceInput::DoubleClick(Position { x: 0, y: 0 }),
    ));
    match message {
        Some(Msg::Shell(ShellRequest::MusicTrackActivate {
            album_id,
            track: activated,
        })) => {
            assert_eq!(album_id, album.id);
            assert_eq!(activated.id, track.id);
        }
        other => panic!("expected track activation, got {other:?}"),
    }
}

#[test]
fn album_wheel_emits_cursor_for_owner_target_and_noop_for_unknown_target() {
    let mut owner = MusicContent::new();
    owner.set_content(context(make_item("Album", "MusicAlbum"), "overview"));

    let area = Rect::new(0, 0, 30, 1);
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|frame| tuirealm::component::Component::view(&mut owner.browser, frame, area))
        .unwrap();

    let event = LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
        at: Position { x: 0, y: 0 },
        delta: 1,
    });
    assert!(matches!(
        owner.on_slot_event(event),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: 0,
            kind: AlbumCursorKind::Move,
        }))
    ));

    owner.context.album_targets.clear();
    // The first wheel's mutation invalidated the completed frame; re-paint so
    // the second gesture resolves the latest frame again.
    let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
    terminal
        .draw(|frame| tuirealm::component::Component::view(&mut owner.browser, frame, area))
        .unwrap();
    assert_eq!(
        owner.on_slot_event(event),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    );
}

#[test]
fn tree_pointer_gestures_resolve_latest_artist_and_album_rows() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"]), ("Beta", &["b-0"])]);
    owner.expand_all_tree_roots();
    let area = Rect::new(0, 0, 48, 8);
    paint_tree(&mut owner, area);

    let root = find(&owner, |target| target.is_artist());
    let album_0 = find(&owner, |target| target.album_leaf_target() == Some("a-0"));
    let album_1 = find(&owner, |target| target.album_leaf_target() == Some("a-1"));
    let root_at = tree_point(&owner, &root);
    let album_0_at = tree_point(&owner, &album_0);

    // A click resolves the painted artist row and changes local selection;
    // the grouping root manufactures no album request, but its resolved
    // focus crosses as the typed artist-track request (design D7).
    match owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
        root_at,
    ))) {
        Some(Msg::Shell(ShellRequest::MusicArtistTracks { target })) => {
            assert_eq!(target.artist_name, "Alpha");
        }
        other => panic!("expected the typed artist-track request, got {other:?}"),
    }
    assert!(owner.selected_is_artist());

    // Each gesture resolves the latest completed frame; a mutation invalidates
    // it, so re-paint before the next gesture.
    paint_tree(&mut owner, area);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
            album_0_at
        ))),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target: 0, .. }))
    ));
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-0"));
    paint_tree(&mut owner, area);

    // A modified click resolves the current painted row first, toggles only
    // the album leaf, and never emits a playback or Queue request.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ToggleClick(
            album_0_at,
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
    assert_eq!(owner.selected_album_targets(), vec!["a-0".to_string()]);
    paint_tree(&mut owner, area);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
            at: album_0_at,
            delta: 1,
        })),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target: 1, .. }))
    ));
    assert_eq!(owner.browser.selected_target(), Some(&album_1));
    paint_tree(&mut owner, area);

    // Double-click resolves the row under the latest retained geometry. The
    // childless leaf is claimed locally and focuses Library once.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            album_0_at
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
    paint_tree(&mut owner, area);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ContextClick(
            album_0_at
        ))),
        Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            Some((x, y)),
        ))) if items.len() == 1 && items[0].id == "a-0" && (x, y) == (album_0_at.x, album_0_at.y)
    ));

    // A right-click on an artist root inside a marked tree selection acts on
    // that selection, not on every descendant of the root. The grouping root
    // itself never crosses the effect boundary.
    paint_tree(&mut owner, area);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ContextClick(
            root_at
        ))),
        Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            Some((x, y)),
        ))) if items.len() == 1
            && items[0].id == "a-0"
            && (x, y) == (root_at.x, root_at.y)
    ));
    assert!(owner.selected_is_artist());
    paint_tree(&mut owner, area);

    // Artist modified-click scopes the operation to its currently visible
    // album descendants, not to an artist or Queue identity.
    let _ = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ToggleClick(
        root_at,
    )));
    assert_eq!(
        owner.selected_album_targets_in_display_order(),
        vec!["a-0".to_string(), "a-1".to_string()]
    );
}

#[test]
fn tree_pointer_noop_and_local_expansion_requests_focus_once() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"])]);
    let area = Rect::new(0, 0, 48, 8);
    paint_tree(&mut owner, area);
    let root = find(&owner, |target| target.is_artist());
    let root_at = tree_point(&owner, &root);

    // The first click resolves the root's artist request; repeating the same
    // painted selection has no other effect and emits the single focus request.
    let _ = owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
        root_at,
    )));
    paint_tree(&mut owner, area);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
            root_at
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));

    // Double-click expansion is local and still crosses once for focus.
    let was_expanded = owner.browser.is_expanded(&root);
    paint_tree(&mut owner, area);
    let root_at = tree_point(&owner, &root);
    assert_eq!(owner.browser.resolve_current_point(root_at), Some(&root));
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            root_at
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
    assert_ne!(owner.browser.is_expanded(&root), was_expanded);
    if !owner.browser.is_expanded(&root) {
        owner
            .browser
            .apply(TreeOperation::ToggleExpansionTarget(root.clone()));
    }

    paint_tree(&mut owner, area);
    let album = find(&owner, |target| target.album_leaf_target() == Some("a-0"));
    let album_at = tree_point(&owner, &album);
    // A childless album claims double-click without opening a Hero or changing
    // expansion, and a wheel at the final row focuses even when movement clamps.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            album_at
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
    let _ = owner.take_album_selection_change();
    paint_tree(&mut owner, area);
    let album_at = tree_point(&owner, &album);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
            at: album_at,
            delta: 1,
        })),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
}

#[test]
fn tree_double_click_cached_album_toggles_expansion_and_focuses_once() {
    let mut track = make_item("Track", "Audio");
    track.id = "track-1".into();
    let mut owner = tree_owner_with_tracks(&[("Alpha", &["a-0"])], Some(vec![track]));
    let root = owner
        .browser
        .visible_targets()
        .first()
        .expect("artist root")
        .clone();
    if !owner.browser.is_expanded(&root) {
        owner
            .browser
            .apply(TreeOperation::ToggleExpansionTarget(root.clone()));
    }
    let area = Rect::new(0, 0, 48, 8);
    paint_tree(&mut owner, area);
    let album = find(&owner, |target| target.album_leaf_target() == Some("a-0"));
    let album_at = tree_point(&owner, &album);
    assert!(!owner.browser.is_expanded(&album));
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            album_at
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
    assert!(owner.browser.is_expanded(&album));

    paint_tree(&mut owner, area);
    let album_at = tree_point(&owner, &album);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            album_at
        ))),
        Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
    ));
    assert!(!owner.browser.is_expanded(&album));
}

/// The four Grouped Music tree node kinds one double-click can resolve, so
/// the gesture's dispatch can be asserted both with and without a local tree
/// filter active (design D5).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DoubleClickNode {
    ArtistRoot,
    AlbumWithTracks,
    ChildlessAlbum,
    Track,
}

impl DoubleClickNode {
    /// The tree fixture each case needs: two settled leaves under one
    /// expanded artist, with the first album's cached track attached to the
    /// selected album.
    fn owner(self) -> MusicContent {
        let mut track = make_item("Track", "Audio");
        track.id = "track-1".into();
        let mut owner = tree_owner_with_tracks(&[("Alpha", &["a-0", "b-0"])], Some(vec![track]));
        let root = owner
            .browser
            .visible_targets()
            .first()
            .expect("artist root")
            .clone();
        if !owner.browser.is_expanded(&root) {
            owner
                .browser
                .apply(TreeOperation::ToggleExpansionTarget(root.clone()));
        }
        let album = find(&owner, |target| target.album_leaf_target() == Some("a-0"));
        match self {
            // A track row is only projected under an expanded album leaf.
            DoubleClickNode::Track => {
                owner
                    .browser
                    .apply(TreeOperation::ToggleExpansionTarget(album.clone()));
            }
            // The cached-children case starts collapsed so the gesture's
            // toggle direction is the assertion.
            DoubleClickNode::AlbumWithTracks if owner.browser.is_expanded(&album) => {
                owner
                    .browser
                    .apply(TreeOperation::ToggleExpansionTarget(album.clone()));
            }
            _ => {}
        }
        owner
    }

    /// The filter query that keeps this node's own row in the tree's filtered
    /// projection (every level matches its own text alone).
    fn filter_query(self) -> &'static str {
        match self {
            DoubleClickNode::ArtistRoot => "Alpha",
            DoubleClickNode::AlbumWithTracks => "a-0",
            DoubleClickNode::ChildlessAlbum => "b-0",
            DoubleClickNode::Track => "Track",
        }
    }

    fn resolve(self, owner: &MusicContent) -> MusicTreeTarget {
        find(owner, |target| match self {
            DoubleClickNode::ArtistRoot => target.is_artist(),
            DoubleClickNode::AlbumWithTracks => target.album_leaf_target() == Some("a-0"),
            DoubleClickNode::ChildlessAlbum => target.album_leaf_target() == Some("b-0"),
            DoubleClickNode::Track => matches!(target, MusicTreeTarget::Track { .. }),
        })
    }
}

/// D5: double-click dispatch is by resolved node kind, in both the unfiltered
/// tree and the filter-forced projection. An expandable node toggles its
/// persistent expansion and crosses once for panel focus, a childless album
/// claims the gesture without changing expansion or opening a Hero, and a
/// track emits the stable-identity play-now intent for the grouped resolver.
#[rstest]
#[case::artist_root_unfiltered(DoubleClickNode::ArtistRoot, false)]
#[case::artist_root_filtered(DoubleClickNode::ArtistRoot, true)]
#[case::album_with_tracks_unfiltered(DoubleClickNode::AlbumWithTracks, false)]
#[case::album_with_tracks_filtered(DoubleClickNode::AlbumWithTracks, true)]
#[case::childless_album_unfiltered(DoubleClickNode::ChildlessAlbum, false)]
#[case::childless_album_filtered(DoubleClickNode::ChildlessAlbum, true)]
#[case::track_unfiltered(DoubleClickNode::Track, false)]
#[case::track_filtered(DoubleClickNode::Track, true)]
fn tree_double_click_dispatches_by_node_kind_filtered_and_unfiltered(
    #[case] node: DoubleClickNode,
    #[case] filtered: bool,
) {
    let mut owner = node.owner();
    if filtered {
        press(&mut owner, Key::Char('/'));
        owner
            .browser
            .apply(TreeOperation::EditFilter(node.filter_query().to_string()));
        assert!(owner.browser.filter_active());
        assert!(
            owner.inline_search.results_len() == 0 && !owner.inline_search.has_pool_entries(),
            "production filtering leaves the flat result carrier empty"
        );
    }
    // A wide-enough fixture keeps every projected row inside the painted
    // viewport, so the gesture resolves through real retained geometry.
    let area = Rect::new(0, 0, 48, 12);
    paint_tree(&mut owner, area);
    let id = node.resolve(&owner);
    let at = tree_point(&owner, &id);
    let double_click = |owner: &mut MusicContent| {
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
            at,
        )))
    };

    match node {
        DoubleClickNode::ArtistRoot => {
            let was_expanded = owner.browser.is_expanded(&id);
            assert!(matches!(
                double_click(&mut owner),
                Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
            ));
            assert_ne!(owner.browser.is_expanded(&id), was_expanded);
        }
        DoubleClickNode::AlbumWithTracks => {
            assert!(!owner.browser.is_expanded(&id));
            assert!(matches!(
                double_click(&mut owner),
                Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
            ));
            assert!(owner.browser.is_expanded(&id));
        }
        DoubleClickNode::ChildlessAlbum => {
            assert!(!owner.browser.is_expanded(&id));
            assert!(matches!(
                double_click(&mut owner),
                Some(Msg::Shell(ShellRequest::LibraryPanelFocus))
            ));
            assert!(
                !owner.browser.is_expanded(&id),
                "a childless album claims the gesture without a state change"
            );
            assert_eq!(owner.browser.selected_target(), Some(&id));
        }
        DoubleClickNode::Track => {
            assert!(matches!(
                double_click(&mut owner),
                Some(Msg::Shell(ShellRequest::MusicTreeTrackActivate {
                    album_target,
                    track_id,
                })) if album_target == "a-0" && track_id == "track-1"
            ));
            assert_eq!(
                owner.browser.selected_target(),
                Some(&id),
                "the resolved track is selected before its play intent crosses"
            );
        }
    }

    if filtered {
        // Filter-forced visibility survives the persistent-expansion update:
        // the gesture never closes the filter or opens a Hero.
        assert!(owner.browser.filter_active());
        assert!(
            owner.browser.visible_targets().contains(&id),
            "the filtered projection still shows the resolved node"
        );
    }
}

/// Production Grouped Music filtering paints the tree and leaves the flat
/// result carrier empty, so click and wheel gestures resolve the current-frame
/// filtered tree geometry instead of the legacy seeded-carrier path.
#[test]
fn filtered_pointer_gestures_resolve_tree_geometry_not_the_flat_carrier() {
    let mut owner = tree_owner(&[("Alpha", &["a-0", "a-1"])]);
    owner.expand_all_tree_roots();
    press(&mut owner, Key::Char('/'));
    owner
        .browser
        .apply(TreeOperation::EditFilter("a-".to_string()));
    assert!(owner.inline_search.results_len() == 0 && !owner.inline_search.has_pool_entries());
    let area = Rect::new(0, 0, 48, 12);
    paint_tree(&mut owner, area);
    let album_0 = find(&owner, |target| target.album_leaf_target() == Some("a-0"));
    let album_0_at = tree_point(&owner, &album_0);

    // A click selects the filtered row the tree painted and reports the
    // resolved album move; the empty carrier resolves nothing.
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
            album_0_at
        ))),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
    ));
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-0"));

    // A wheel inside the painted filtered tree rectangle moves the tree's own
    // viewport selection rather than falling through unclaimed. The click's
    // mutation invalidated the completed frame, so re-paint first.
    paint_tree(&mut owner, area);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Wheel {
            at: album_0_at,
            delta: 1,
        })),
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
    ));
    assert_eq!(owner.selected_album_target().as_deref(), Some("a-1"));
}

#[test]
fn tree_context_click_outside_selection_clears_only_tree_marks() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"]), ("Beta", &["b-0"])]);
    owner.expand_all_tree_roots();
    let area = Rect::new(0, 0, 48, 8);
    paint_tree(&mut owner, area);
    let a0 = find(&owner, |target| target.album_leaf_target() == Some("a-0"));
    let b0 = find(&owner, |target| target.album_leaf_target() == Some("b-0"));
    owner
        .browser
        .apply(TreeOperation::ToggleMarkTarget(a0.clone()));
    // The mark mutation invalidated the completed frame; re-paint before
    // resolving the clicked row against the latest geometry.
    paint_tree(&mut owner, area);
    let b0_at = tree_point(&owner, &b0);
    assert!(matches!(
        owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::ContextClick(b0_at))),
        Some(Msg::Shell(ShellRequest::MusicRowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            _,
        ))) if items.len() == 1 && items[0].id == "b-0"
    ));
    assert!(
        owner.selected_album_targets().is_empty(),
        "a context click outside the marked set clears only this tree"
    );
}
