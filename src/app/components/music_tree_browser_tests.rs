//! Grouped Music browser-owner focused cases: viewport/geometry continuity,
//! missing-node fallback, neighbour prefetch, modified-selection tri-state,
//! filtered projection and mark semantics, and cached track projection.

use super::tests::{
    album_id, alpha, artist_id, assert_selection_visible, base_entries, entry, expand_all_roots,
    frame, projection_row, viewport_entries,
};
use super::{MusicTreeBrowser, MusicTreeEntry, MusicTreeModel, MusicTreeTrack};
use crate::app::music_grouping::ArtistKey;
use ratatui::layout::Position;
use std::collections::HashMap;
use tui_treelistview::{TreeMarkState, TreeModel};

#[test]
fn a_geometry_change_keeps_the_selection_visible_within_bounds() {
    let entries = viewport_entries();
    let model = MusicTreeModel::from_entries(&entries);
    let beta_11 = album_id(&model, "beta-11").expect("beta-11 interned");
    let mut browser = MusicTreeBrowser::new(model);
    expand_all_roots(&mut browser);
    browser.select_id(beta_11);

    frame(&mut browser, 30);
    assert_eq!(browser.offset(), 0, "the tall viewport needs no scroll");

    // Shrinking the same owner clamps around the selection instead of leaving
    // it outside a viewport clamped to the top.
    frame(&mut browser, 5);
    assert_eq!(browser.offset(), browser.projection_len() - 5);
    assert_selection_visible(&browser, 5);

    // Growing back re-clamps at the projection bounds.
    frame(&mut browser, 30);
    assert_eq!(browser.offset(), 0);
    assert_selection_visible(&browser, 30);
}

#[test]
fn a_geometry_change_does_not_persist_filter_forced_expansion() {
    let entries = base_entries();
    let model = MusicTreeModel::from_entries(&entries);
    let alpha_root =
        artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("alpha root");
    let album_2 = album_id(&model, "album-2").expect("album-2 interned");
    let mut browser = MusicTreeBrowser::new(model);

    // A filter policy force-expands matching paths in the projection while
    // leaving persistent expansion untouched (design D5). Matching every
    // current node projects the whole tree under forced expansion.
    let matching: Vec<usize> = (0..browser.model.size_hint()).collect();
    browser.set_filter_matches(Some(&matching));
    frame(&mut browser, 10);
    assert!(
        !browser.root_is_expanded(alpha_root),
        "filter-forced expansion is not persistent"
    );
    assert_eq!(browser.target_of(album_2), Some("album-2"));

    // Select the deep leaf by projection row only, so nothing persists its
    // ancestor path on its behalf.
    let row = projection_row(&browser, album_2);
    browser.select_index(row);
    assert_eq!(browser.selected_id(), Some(album_2));

    // A geometry change re-arms the same owner's viewport visibility without
    // promoting the filter-forced branch into persistent expansion.
    frame(&mut browser, 4);
    assert!(
        !browser.root_is_expanded(alpha_root),
        "a resize must not persist filter-forced expansion"
    );
    assert_eq!(browser.selected_id(), Some(album_2));
    assert_selection_visible(&browser, 4);
}

#[test]
fn a_removed_selected_album_falls_back_to_a_visible_node() {
    let entries = base_entries();
    let model = MusicTreeModel::from_entries(&entries);
    let alpha_root =
        artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("alpha root");
    let album_1 = album_id(&model, "album-1").expect("album-1 interned");
    let mut browser = MusicTreeBrowser::new(model);
    expand_all_roots(&mut browser);
    browser.select_id(album_1);
    frame(&mut browser, 5);

    let replacement: Vec<MusicTreeEntry> = entries
        .iter()
        .filter(|entry| entry.target != "album-1")
        .cloned()
        .collect();
    assert!(browser.reconcile(&replacement));
    assert_eq!(
        browser.selected_id(),
        Some(alpha_root),
        "the missing node's fallback is its artist parent"
    );
    frame(&mut browser, 5);
    assert_selection_visible(&browser, 5);
}

#[test]
fn album_selection_persistence_changes_only_with_the_resolved_album() {
    let model = MusicTreeModel::from_entries(&base_entries());
    let alpha_root =
        artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("alpha root");
    let album_1 = album_id(&model, "album-1").expect("album-1 interned");
    let album_3 = album_id(&model, "album-3").expect("album-3 interned");
    let mut browser = MusicTreeBrowser::new(model);

    // The initial artist-root selection has no album to persist.
    assert_eq!(browser.selected_id(), Some(alpha_root));
    assert_eq!(browser.take_album_selection_change(), None);

    browser.select_id(album_1);
    assert_eq!(
        browser.take_album_selection_change().as_deref(),
        Some("album-1")
    );
    assert_eq!(
        browser.take_album_selection_change(),
        None,
        "an unchanged album does not re-emit"
    );

    // Focusing an artist root neither emits nor overwrites the retained album.
    browser.select_id(alpha_root);
    assert_eq!(browser.take_album_selection_change(), None);
    browser.select_id(album_1);
    assert_eq!(
        browser.take_album_selection_change(),
        None,
        "returning to the retained album is still not a change"
    );

    browser.select_id(album_3);
    assert_eq!(
        browser.take_album_selection_change().as_deref(),
        Some("album-3")
    );
}

/// Task 6.5 (design D4): the neighbour artwork window resolves from the
/// latest completed paint's visible projection — one album leaf behind the
/// selected leaf, three ahead, in visible order, skipping artist roots.
#[test]
fn neighbour_prefetch_window_is_one_behind_three_ahead_over_visible_leaves() {
    let model = MusicTreeModel::from_entries(&viewport_entries());
    let mut browser = MusicTreeBrowser::new(model);
    browser.expand_all_roots();
    browser.select_album_target("alpha-5");

    // No completed paint means no painted projection to key the window off.
    assert_eq!(browser.neighbour_prefetch_targets(), None);

    frame(&mut browser, 8);
    assert_eq!(
        browser.neighbour_prefetch_targets(),
        Some(vec![
            "alpha-4".to_string(),
            "alpha-6".to_string(),
            "alpha-7".to_string(),
            "alpha-8".to_string(),
        ])
    );
}

/// Task 6.5: the window clamps at the visible edges and crosses artist roots
/// without ever naming one — only album leaf targets cross the boundary.
#[test]
fn neighbour_prefetch_window_clamps_at_edges_and_skips_artist_roots() {
    let model = MusicTreeModel::from_entries(&viewport_entries());
    let mut browser = MusicTreeBrowser::new(model);
    browser.expand_all_roots();

    browser.select_album_target("alpha-0");
    frame(&mut browser, 8);
    assert_eq!(
        browser.neighbour_prefetch_targets(),
        Some(vec![
            "alpha-1".to_string(),
            "alpha-2".to_string(),
            "alpha-3".to_string(),
        ]),
        "the first visible leaf has no behind neighbour"
    );

    browser.select_album_target("alpha-11");
    frame(&mut browser, 8);
    assert_eq!(
        browser.neighbour_prefetch_targets(),
        Some(vec![
            "alpha-10".to_string(),
            "beta-0".to_string(),
            "beta-1".to_string(),
            "beta-2".to_string(),
        ]),
        "the window crosses roots but skips every artist root"
    );
}

/// Task 6.5: an artist-root focus ships no neighbour request, and an
/// invalidated (unpainted) projection cannot key the window.
#[test]
fn neighbour_prefetch_is_suppressed_for_an_artist_root_or_an_unpainted_frame() {
    let model = MusicTreeModel::from_entries(&viewport_entries());
    let alpha_root =
        artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("alpha root");
    let mut browser = MusicTreeBrowser::new(model);
    browser.expand_all_roots();
    frame(&mut browser, 8);

    browser.select_id(alpha_root);
    frame(&mut browser, 8);
    assert_eq!(
        browser.neighbour_prefetch_targets(),
        None,
        "an artist-root focus ships no neighbour request"
    );

    browser.select_album_target("alpha-5");
    frame(&mut browser, 8);
    assert!(browser.neighbour_prefetch_targets().is_some());
    browser.invalidate();
    assert_eq!(
        browser.neighbour_prefetch_targets(),
        None,
        "an invalidated (unpainted) projection cannot key the window"
    );
}

#[test]
fn modified_selection_keeps_added_order_and_derives_artist_tri_state() {
    let model = MusicTreeModel::from_entries(&base_entries());
    let alpha_root = artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("root");
    let album_1 = album_id(&model, "album-1").expect("album-1");
    let album_2 = album_id(&model, "album-2").expect("album-2");
    let mut browser = MusicTreeBrowser::new(model);

    // Membership is album-only and retains the order in which leaves were
    // added, even though the crate's internal mark set is unordered.
    assert!(browser.set_marked(album_2, true));
    assert!(browser.set_marked(album_1, true));
    assert_eq!(
        browser.selected_album_targets(),
        vec!["album-2".to_string(), "album-1".to_string()]
    );
    assert_eq!(browser.mark_state(alpha_root), TreeMarkState::Marked);
    browser.expand_root(alpha_root);
    assert_eq!(
        browser.selected_album_targets_in_display_order(),
        vec!["album-1".to_string(), "album-2".to_string()]
    );

    // Toggling the artist root removes all visible descendants; a second
    // toggle adds them back in settled child order.
    assert!(browser.toggle_mark(alpha_root));
    assert!(browser.selected_album_targets().is_empty());
    assert_eq!(browser.mark_state(alpha_root), TreeMarkState::Unmarked);
    assert!(browser.toggle_mark(alpha_root));
    assert_eq!(browser.mark_state(alpha_root), TreeMarkState::Marked);
    assert_eq!(
        browser.selected_album_targets(),
        vec!["album-1".to_string(), "album-2".to_string()]
    );
}

#[test]
fn filter_matches_each_level_on_its_own_text_and_hides_everything_below_it() {
    // "Devil" is in both the artist's name and one album title: each is its
    // own match at its own level, and neither drags the artist's other albums
    // or the album's tracks in.
    let entries = vec![
        entry(
            "Devil Band",
            ArtistKey::Service("artist-devil".into()),
            "Devil Soup",
            "album-1",
            "2001",
        ),
        entry(
            "Devil Band",
            ArtistKey::Service("artist-devil".into()),
            "Cake",
            "album-2",
            "2002",
        ),
        entry(
            "Other Band",
            ArtistKey::Service("artist-other".into()),
            "Devil Cake",
            "album-3",
            "2003",
        ),
    ];
    let mut model = MusicTreeModel::new();
    let mut tracks = HashMap::new();
    tracks.insert(
        "album-1".into(),
        vec![
            MusicTreeTrack {
                target: "track-1".into(),
                title: "Track One".into(),
                search_title: "Track One".into(),
            },
            MusicTreeTrack {
                target: "track-2".into(),
                title: "2. Devil Track".into(),
                search_title: "Devil Track".into(),
            },
        ],
    );
    model.reconcile_with_tracks(&entries, &tracks);
    let mut browser = MusicTreeBrowser::new(model);
    browser.open_filter();

    let visible = |browser: &MusicTreeBrowser| -> Vec<String> {
        browser
            .projected_nodes()
            .iter()
            .map(|node| browser.title_of(node.id()).to_string())
            .collect()
    };

    // The artist name matches the root alone: the sibling albums that shared
    // the name are gone, and so is every track below the matching album.
    browser.apply_filter_query("Devil Band");
    assert_eq!(visible(&browser), ["Devil Band"]);

    // An album-title match shows the artist path and that album, never the
    // artist's other albums.
    browser.apply_filter_query("Cake");
    assert_eq!(
        visible(&browser),
        ["Devil Band", "Cake", "Other Band", "Devil Cake"]
    );

    browser.apply_filter_query("Devil Soup");
    assert_eq!(visible(&browser), ["Devil Band", "Devil Soup"]);

    // A track-title match shows its own path down to the track, with no
    // sibling track and no sibling album.
    browser.apply_filter_query("Devil Track");
    assert_eq!(
        visible(&browser),
        ["Devil Band", "Devil Soup", "2. Devil Track"]
    );

    // The painted number is not part of the track's searchable title.
    browser.apply_filter_query("2.");
    assert!(visible(&browser).is_empty());

    browser.apply_filter_query("Track One");
    assert_eq!(visible(&browser), ["Devil Band", "Devil Soup", "Track One"]);
}

/// The reported case, on the tree's own corpus: an album whose name is the
/// whole phrase. No separator is involved, so "devil" could only come from
/// single letters taken from different words of that name. The shared
/// word-local rule rejects it, while the name's real words still find the
/// album.
#[test]
fn filter_rejects_a_scatter_through_one_albums_name() {
    let entries = vec![entry(
        "The Velvet Underground",
        ArtistKey::Service("artist-vu".into()),
        "The Velvet Underground Live With Lou Reed",
        "album-vu",
        "1974",
    )];
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries));
    browser.open_filter();

    browser.apply_filter_query("devil");
    assert!(browser.projected_nodes().is_empty(), "scatter must not hit");

    browser.apply_filter_query("velvet");
    let visible: Vec<String> = browser
        .projected_nodes()
        .iter()
        .map(|node| browser.title_of(node.id()).to_string())
        .collect();
    assert_eq!(
        visible,
        [
            "The Velvet Underground",
            "The Velvet Underground Live With Lou Reed"
        ]
    );
}

#[test]
fn fuzzy_filter_uses_composite_text_and_preserves_settled_order() {
    let entries = vec![
        entry(
            "Alpha Artist",
            ArtistKey::Service("alpha".into()),
            "Quiet Record",
            "album-1",
            "2001",
        ),
        entry(
            "Alpha Artist",
            ArtistKey::Service("alpha".into()),
            "Loud Record",
            "album-2",
            "2002",
        ),
        entry(
            "Beta Artist",
            ArtistKey::Service("beta".into()),
            "Other Record",
            "album-3",
            "2003",
        ),
    ];
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries));
    let alpha = browser.projected_nodes().first().expect("root").id();
    browser.collapse_root(alpha);
    browser.open_filter();
    browser.apply_filter_query("2002");

    let visible: Vec<&str> = browser
        .projected_nodes()
        .iter()
        .map(|node| browser.title_of(node.id()))
        .collect();
    assert_eq!(visible, ["Alpha Artist", "Loud Record"]);

    browser.apply_filter_query("does-not-match");
    assert!(browser.projected_nodes().is_empty());
    browser.apply_filter_query("");
    assert_eq!(
        browser.projected_nodes().len(),
        2,
        "empty query restores the tree"
    );
}

#[test]
fn filter_session_restores_anchor_expansion_and_hidden_marks() {
    let entries = vec![alpha("album-1", "First"), alpha("album-2", "Second")];
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries));
    let root = browser.projected_nodes()[0].id();
    browser.expand_root(root);
    let album_2 = browser.projected_nodes()[2].id();
    browser.select_id(album_2);
    browser.set_marked(album_2, true);
    browser.collapse_root(root);
    browser.select_id(root);

    browser.open_filter();
    browser.apply_filter_query("First");
    assert_eq!(browser.projected_nodes().len(), 2);
    assert!(browser.selected_id().is_some());
    assert!(
        browser.selected_album_targets().is_empty(),
        "hidden marks are masked"
    );

    browser.close_filter();
    assert_eq!(browser.selected_id(), Some(root));
    assert!(
        !browser.root_is_expanded(root),
        "forced expansion is not persistent"
    );
    assert_eq!(browser.selected_album_targets(), vec!["album-2"]);
}

/// A matched album shows down to its own level: its artist path and the album
/// itself, never the cached tracks below it, which did not match.
#[test]
fn matching_album_hides_cached_track_children_in_the_filtered_projection() {
    let entries = vec![alpha("album-1", "First")];
    let mut model = MusicTreeModel::new();
    let mut tracks = HashMap::new();
    tracks.insert(
        "album-1".into(),
        vec![MusicTreeTrack {
            target: "track-1".into(),
            title: "Track".into(),
            search_title: "Track".into(),
        }],
    );
    model.reconcile_with_tracks(&entries, &tracks);
    let mut browser = MusicTreeBrowser::new(model);
    browser.open_filter();
    browser.apply_filter_query("First");
    let titles: Vec<&str> = browser
        .projected_nodes()
        .iter()
        .map(|node| browser.title_of(node.id()))
        .collect();
    assert_eq!(titles, ["Alpha", "First"]);
}

#[test]
fn filtered_artist_toggle_masks_hidden_marks_without_losing_them() {
    let model = MusicTreeModel::from_entries(&base_entries());
    let alpha_root = artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("root");
    let album_1 = album_id(&model, "album-1").expect("album-1");
    let album_2 = album_id(&model, "album-2").expect("album-2");
    let mut browser = MusicTreeBrowser::new(model);

    // Keep a mark on the album hidden by the active filter, then toggle the
    // visible root. The root sees only its matching descendant.
    assert!(browser.set_marked(album_2, true));
    browser.set_filter_matches(Some(&[album_1]));
    assert_eq!(browser.mark_state(alpha_root), TreeMarkState::Unmarked);
    assert!(browser.toggle_mark(alpha_root));
    assert_eq!(
        browser.selected_album_targets(),
        vec!["album-1".to_string()]
    );
    assert_eq!(browser.mark_state(alpha_root), TreeMarkState::Marked);

    // Dismissing the filter reveals the surviving hidden mark without adding
    // any new album membership.
    browser.set_filter_matches(None);
    assert_eq!(
        browser.selected_album_targets(),
        vec!["album-2".to_string(), "album-1".to_string()]
    );
    assert_eq!(browser.mark_state(alpha_root), TreeMarkState::Marked);
}

#[test]
fn cached_tracks_project_as_ordered_depth_two_children() {
    let entries = vec![alpha("album-1", "First Album")];
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries));
    browser.set_track_items(HashMap::from([(
        "album-1".to_string(),
        vec![
            MusicTreeTrack {
                target: "track-1".into(),
                title: "Track One".into(),
                search_title: "Track One".into(),
            },
            MusicTreeTrack {
                target: "track-2".into(),
                title: "Track Two".into(),
                search_title: "Track Two".into(),
            },
        ],
    )]));
    browser.reconcile(&entries);
    let root = browser.selected_id().expect("root selected");
    browser.expand_root(root);
    let album = browser.projected_nodes()[1].id();
    browser.expand_node(album);
    assert_eq!(browser.projection_len(), 4);
    assert_eq!(browser.projected_nodes()[1].level(), 1);
    assert_eq!(browser.projected_nodes()[2].level(), 2);
    assert_eq!(browser.projected_nodes()[3].level(), 2);
    browser.select_index(2);
    assert_eq!(
        browser.selected_track_identity(),
        Some(("album-1", "track-1"))
    );
    browser.select_index(3);
    assert_eq!(
        browser.selected_track_identity(),
        Some(("album-1", "track-2"))
    );
}

/// Row 5.2: every settled content push re-applies the active filter, and the
/// crate advances its filter revision on every write. An unchanged match set
/// must keep the current projection and its completed-frame hit geometry, so a
/// filtered pointer gesture still resolves; a genuinely different match set
/// must drop that geometry until the next paint.
#[test]
fn an_unchanged_filter_reapplication_keeps_completed_frame_hit_geometry() {
    let model = MusicTreeModel::from_entries(&base_entries());
    let album_1 = album_id(&model, "album-1").expect("album-1 interned");
    let album_2 = album_id(&model, "album-2").expect("album-2 interned");
    let mut browser = MusicTreeBrowser::new(model);
    browser.open_filter();
    browser.apply_filter_query("First");
    frame(&mut browser, 6);
    let row = browser
        .row_rect_for(album_1)
        .expect("filtered match painted a row");
    let at = Position { x: row.x, y: row.y };
    assert_eq!(
        browser.hit_node(at).map(|(id, _)| id),
        Some(album_1),
        "the filtered row resolves through the completed frame"
    );

    // The same query re-applied (what a settled content push does) leaves the
    // projection alone, so the painted row still resolves.
    browser.apply_filter_query("First");
    assert_eq!(
        browser.hit_node(at).map(|(id, _)| id),
        Some(album_1),
        "a no-op filter re-application keeps the current-frame hit rows"
    );

    // A different match set rebuilds the projection: the retained row no
    // longer claims input until the replacement frame paints.
    browser.apply_filter_query("Second");
    assert_eq!(
        browser.hit_node(at),
        None,
        "a changed filtered projection invalidates the stale hit map"
    );
    frame(&mut browser, 6);
    let row = browser
        .row_rect_for(album_2)
        .expect("the new match painted a row");
    assert_eq!(
        browser
            .hit_node(Position { x: row.x, y: row.y })
            .map(|(id, _)| id),
        Some(album_2)
    );
}
