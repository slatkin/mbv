//! Task 2.1 focused cases for the Grouped Music tree model and task 2.2
//! focused cases for the one state owner over it: the destination-local node
//! arena (monotonic non-reused ids, settled ordering, atomic model revision,
//! and node-to-domain translation), refresh retention, deletion tombstoning,
//! equal-name separation, and destination reset; then selected-node and
//! expansion retention, viewport continuity/clamping, missing-node fallback,
//! multi-selection survival, projection-cache invalidation, and the
//! album-selection persistence guard.

use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use std::collections::HashMap;
use tui_treelistview::{TreeMarkState, TreeModel, TreeRevision};

use super::{MusicNodeKey, MusicTreeBrowser, MusicTreeEntry, MusicTreeModel, MusicTreeTrack};
use crate::app::components::media_list::MediaSemanticState;
use crate::app::music_grouping::ArtistKey;

fn entry(
    artist: &str,
    artist_key: ArtistKey,
    title: &str,
    target: &str,
    year: &str,
) -> MusicTreeEntry {
    MusicTreeEntry {
        artist: artist.to_string(),
        artist_key,
        title: title.to_string(),
        year: (!year.is_empty()).then(|| year.to_string()),
        target: target.to_string(),
        semantic_state: MediaSemanticState::Ordinary,
    }
}

fn alpha(target: &str, title: &str) -> MusicTreeEntry {
    entry(
        "Alpha",
        ArtistKey::Service("artist-alpha".into()),
        title,
        target,
        "2001",
    )
}

fn beta(target: &str, title: &str) -> MusicTreeEntry {
    entry(
        "Beta",
        ArtistKey::Service("artist-beta".into()),
        title,
        target,
        "2003",
    )
}

/// The base settled catalog: Alpha with two albums, Beta with one.
fn base_entries() -> Vec<MusicTreeEntry> {
    vec![
        alpha("album-1", "First Album"),
        alpha("album-2", "Second Album"),
        beta("album-3", "Beta Session"),
    ]
}

fn artist_id(model: &MusicTreeModel, key: ArtistKey) -> Option<usize> {
    model.node_id(&MusicNodeKey::Artist(key))
}

fn album_id(model: &MusicTreeModel, target: &str) -> Option<usize> {
    model.node_id(&MusicNodeKey::Album(target.to_string()))
}

#[test]
fn settled_ordering_and_grouping_follow_the_entries() {
    let model = MusicTreeModel::from_entries(&base_entries());
    let alpha_root =
        artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("alpha root");
    let beta_root = artist_id(&model, ArtistKey::Service("artist-beta".into())).expect("beta root");

    assert_eq!(model.root_ids(), vec![alpha_root, beta_root]);
    assert_eq!(model.title_of(alpha_root), "Alpha");
    assert_eq!(model.title_of(beta_root), "Beta");
    assert_eq!(
        model.children_of(alpha_root),
        vec![
            album_id(&model, "album-1").expect("album-1 interned"),
            album_id(&model, "album-2").expect("album-2 interned"),
        ]
    );
    assert_eq!(
        model.children_of(beta_root),
        vec![album_id(&model, "album-3").expect("album-3 interned")]
    );
}

#[test]
fn equal_display_names_with_distinct_identities_stay_separate_roots() {
    let entries = vec![
        entry(
            "Alpha",
            ArtistKey::Service("artist-1".into()),
            "Greatest Hits",
            "album-1",
            "",
        ),
        entry(
            "Alpha",
            ArtistKey::Service("artist-2".into()),
            "Other Hits",
            "album-2",
            "",
        ),
    ];
    let model = MusicTreeModel::from_entries(&entries);
    let root_1 = artist_id(&model, ArtistKey::Service("artist-1".into())).expect("root 1");
    let root_2 = artist_id(&model, ArtistKey::Service("artist-2".into())).expect("root 2");

    assert_ne!(root_1, root_2, "equal display names, distinct identities");
    assert_eq!(model.root_ids(), vec![root_1, root_2]);
    assert_eq!(model.children_of(root_1).len(), 1);
    assert_eq!(model.children_of(root_2).len(), 1);
    assert_eq!(
        model.target_of(model.children_of(root_1)[0]),
        Some("album-1")
    );
    assert_eq!(
        model.target_of(model.children_of(root_2)[0]),
        Some("album-2")
    );
}

#[test]
fn fallback_identity_groups_by_display_identity_and_survives_refresh() {
    let entries = vec![
        entry(
            "Unknown Artist",
            ArtistKey::Fallback("Unknown Artist".into()),
            "One",
            "album-1",
            "",
        ),
        entry(
            "Unknown Artist",
            ArtistKey::Fallback("Unknown Artist".into()),
            "Two",
            "album-2",
            "",
        ),
    ];
    let key = ArtistKey::Fallback("Unknown Artist".into());
    let mut model = MusicTreeModel::from_entries(&entries);
    let root = artist_id(&model, key.clone()).expect("fallback root");
    assert_eq!(model.children_of(root).len(), 2);

    let revision = model.revision_value();
    model.reconcile(&entries);
    assert_eq!(artist_id(&model, key), Some(root));
    assert_eq!(
        model.revision_value(),
        revision,
        "an identical fallback refresh holds the revision"
    );
}

#[test]
fn refresh_retains_surviving_ids_and_tombstones_removed_keys() {
    let mut model = MusicTreeModel::from_entries(&base_entries());
    let alpha_root =
        artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("alpha root");
    let beta_root = artist_id(&model, ArtistKey::Service("artist-beta".into())).expect("beta root");
    let alpha_1 = album_id(&model, "album-1").expect("album-1 interned");
    let beta_3 = album_id(&model, "album-3").expect("album-3 interned");

    // An ordinary refresh with the same settled keys changes nothing.
    let revision = model.revision_value();
    model.reconcile(&base_entries());
    assert_eq!(
        artist_id(&model, ArtistKey::Service("artist-alpha".into())),
        Some(alpha_root)
    );
    assert_eq!(album_id(&model, "album-1"), Some(alpha_1));
    assert_eq!(model.revision_value(), revision);

    // Beta's only album is deleted: its root leaves the projection while the
    // interned mapping is retained (tombstoned).
    let mut replacement = base_entries();
    replacement.retain(|entry| entry.target != "album-3");
    model.reconcile(&replacement);
    assert_eq!(model.root_ids(), vec![alpha_root]);
    assert_eq!(
        album_id(&model, "album-1"),
        Some(alpha_1),
        "survivor keeps its id"
    );
    assert_eq!(
        album_id(&model, "album-3"),
        Some(beta_3),
        "a removed key stays interned"
    );
    assert_eq!(
        artist_id(&model, ArtistKey::Service("artist-beta".into())),
        Some(beta_root)
    );

    // New keys intern fresh ids; a removed id is never reused for a
    // different key.
    let mut grown = replacement;
    grown.push(beta("album-4", "New Beta"));
    grown.push(entry(
        "Gamma",
        ArtistKey::Service("artist-gamma".into()),
        "Gamma Album",
        "album-5",
        "",
    ));
    model.reconcile(&grown);
    let beta_4 = album_id(&model, "album-4").expect("album-4 interned");
    let gamma_root =
        artist_id(&model, ArtistKey::Service("artist-gamma".into())).expect("gamma root");
    assert!(beta_4 > beta_3, "a new key interns a fresh id");
    assert_ne!(gamma_root, beta_root);
    assert_ne!(gamma_root, beta_3);
    assert_eq!(
        artist_id(&model, ArtistKey::Service("artist-beta".into())),
        Some(beta_root),
        "the surviving Beta mapping is reused for the same key"
    );
    assert_eq!(model.root_ids(), vec![alpha_root, beta_root, gamma_root]);
}

#[test]
fn destination_reset_clears_the_arena_and_restarts_ids() {
    let mut model = MusicTreeModel::from_entries(&base_entries());
    assert!(album_id(&model, "album-1").is_some());

    model.reset();
    assert!(
        album_id(&model, "album-1").is_none(),
        "no stale mapping survives a destination reset"
    );
    assert!(model.root_ids().is_empty());

    // A fresh arena interns from zero again in settled order: Alpha root,
    // its two leaves, then the Beta root and its leaf.
    model.reconcile(&base_entries());
    assert_eq!(album_id(&model, "album-1"), Some(1));
    assert_eq!(album_id(&model, "album-2"), Some(2));
    assert_eq!(album_id(&model, "album-3"), Some(4));
}

#[test]
fn model_revision_advances_only_on_real_catalog_change() {
    let mut model = MusicTreeModel::from_entries(&base_entries());
    assert!(
        model.revision_value() > TreeRevision::INITIAL.get(),
        "a populated model is not at the initial revision"
    );

    // Identical replacement: no revision change (the projection cache holds).
    let revision = model.revision_value();
    model.reconcile(&base_entries());
    assert_eq!(model.revision_value(), revision);

    // A settled catalog change (deletion) bumps the revision.
    model.reconcile(&[alpha("album-1", "First Album")]);
    assert!(model.revision_value() > revision);

    // A display-only settled change also bumps it.
    let revision = model.revision_value();
    model.reconcile(&[alpha("album-1", "Renamed Album")]);
    assert!(model.revision_value() > revision);
}

#[test]
fn node_to_domain_translation_maps_artist_keys_and_album_targets() {
    let model = MusicTreeModel::from_entries(&base_entries());
    let alpha_root =
        artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("alpha root");
    let album_2 = album_id(&model, "album-2").expect("album-2 interned");

    assert_eq!(
        model.artist_key_of(alpha_root),
        Some(&ArtistKey::Service("artist-alpha".into()))
    );
    assert_eq!(model.artist_key_of(album_2), None);
    assert_eq!(model.target_of(album_2), Some("album-2"));
    assert_eq!(model.target_of(alpha_root), None);
    assert!(model.is_artist(alpha_root));
    assert!(!model.is_artist(album_2));
}

/// A two-artist corpus large enough to overflow a five-row viewport: Alpha
/// and Beta with twelve albums each. Arena ids follow settled order (Alpha
/// root, its leaves, Beta root, its leaves).
fn viewport_entries() -> Vec<MusicTreeEntry> {
    let mut entries = Vec::new();
    for (artist, key, target_prefix) in [
        ("Alpha", "artist-alpha", "alpha"),
        ("Beta", "artist-beta", "beta"),
    ] {
        for i in 0..12 {
            entries.push(entry(
                artist,
                ArtistKey::Service(key.into()),
                &format!("{artist} {i:02}"),
                &format!("{target_prefix}-{i}"),
                "2001",
            ));
        }
    }
    entries
}

const VIEW_WIDTH: u16 = 40;

fn expand_all_roots(browser: &mut MusicTreeBrowser) {
    for root in 0..browser.projection_len() {
        let id = browser.projected_nodes()[root].id();
        if browser.target_of(id).is_none() {
            browser.expand_root(id);
        }
    }
}

/// One component-level frame at a `VIEW_WIDTH` × `height` browser rect. The
/// owner's viewport, mark aggregate, and hit map settle exactly as they do
/// when the mounted view paints.
fn frame(browser: &mut MusicTreeBrowser, height: u16) {
    let area = Rect::new(0, 0, VIEW_WIDTH, height);
    let mut term = Terminal::new(TestBackend::new(area.width, area.height)).expect("test terminal");
    term.draw(|f| browser.view(f, area)).expect("tree frame");
}

fn projection_row(browser: &MusicTreeBrowser, id: usize) -> usize {
    browser
        .projected_nodes()
        .iter()
        .position(|node| node.id() == id)
        .expect("node is projected")
}

fn assert_selection_visible(browser: &MusicTreeBrowser, height: usize) {
    let selected = projection_row(browser, browser.selected_id().expect("a node is selected"));
    let offset = browser.offset();
    assert!(
        selected >= offset && selected < offset + height,
        "selection row {selected} outside the viewport {offset}..{}",
        offset + height
    );
}

#[test]
fn tree_hit_geometry_is_claimable_only_after_the_latest_view() {
    let entries = base_entries();
    let model = MusicTreeModel::from_entries(&entries);
    let alpha_root =
        artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("alpha root");
    let mut browser = MusicTreeBrowser::new(model);
    browser.expand_root(alpha_root);

    let point = Position::new(1, 0);
    frame(&mut browser, 5);
    assert!(browser.claims_point(point));
    assert!(browser.hit_node(point).is_some());
    assert!(browser.hit_test(point).is_some());

    // An explicit invalidation models a content/area configuration that has
    // happened after the last completed frame. Every pointer-resolution seam
    // must reject the old row until a new view completes.
    browser.invalidate();
    assert!(!browser.claims_point(point));
    assert!(browser.hit_node(point).is_none());
    assert!(browser.hit_test(point).is_none());

    frame(&mut browser, 5);
    assert!(browser.claims_point(point));

    // A settled content replacement invalidates the completed hit map too,
    // even when the surviving root still paints at the same row.
    assert!(browser.reconcile(&[alpha("album-1", "Renamed Album")]));
    assert!(!browser.claims_point(point));
    assert!(browser.hit_test(point).is_none());

    frame(&mut browser, 5);
    assert!(browser.claims_point(point));

    // The panel's viewport/geometry configuration is another invalidation
    // boundary; the old frame cannot claim while the replacement is pending.
    browser.clamp_viewport_to(2);
    assert!(!browser.claims_point(point));
    assert!(browser.hit_test(point).is_none());
}

#[test]
fn settled_replacement_retains_the_selected_node_expansion_and_marks() {
    let entries = base_entries();
    let model = MusicTreeModel::from_entries(&entries);
    let alpha_root =
        artist_id(&model, ArtistKey::Service("artist-alpha".into())).expect("alpha root");
    let beta_root = artist_id(&model, ArtistKey::Service("artist-beta".into())).expect("beta root");
    let album_1 = album_id(&model, "album-1").expect("album-1 interned");
    let album_2 = album_id(&model, "album-2").expect("album-2 interned");
    let mut browser = MusicTreeBrowser::new(model);
    browser.expand_root(alpha_root);
    browser.expand_root(beta_root);
    browser.select_id(album_1);

    // Aggregate root state is derived: an artist root is never a stored
    // mark target (design D6).
    assert!(
        !browser.set_marked(alpha_root, true),
        "artist roots are not stored mark targets"
    );
    assert_eq!(browser.mark_state(alpha_root), TreeMarkState::Unmarked);
    browser.set_marked(album_1, true);
    assert_eq!(browser.mark_state(alpha_root), TreeMarkState::Partial);

    // A settled replacement that still contains the selected leaf and both
    // expanded roots rebuilds the projection but keeps the owner's state.
    let mut grown = base_entries();
    grown.push(beta("album-4", "New Beta"));
    assert!(
        browser.reconcile(&grown),
        "a settled change rebuilds the projection"
    );
    assert_eq!(browser.selected_id(), Some(album_1));
    assert!(browser.root_is_expanded(alpha_root));
    assert!(browser.root_is_expanded(beta_root));
    assert_eq!(browser.projection_len(), 6);
    assert_eq!(browser.mark_state(album_1), TreeMarkState::Marked);
    assert_eq!(browser.mark_state(alpha_root), TreeMarkState::Partial);
    assert_eq!(browser.mark_state(beta_root), TreeMarkState::Unmarked);

    // The surviving mark lifts to Marked once the second leaf is marked.
    browser.set_marked(album_2, true);
    assert_eq!(browser.mark_state(alpha_root), TreeMarkState::Marked);
}

#[test]
fn marking_an_out_of_range_node_id_is_a_no_op() {
    let model = MusicTreeModel::from_entries(&base_entries());
    let mut browser = MusicTreeBrowser::new(model);
    // An id interned by a larger arena is foreign here: `set_marked` is the
    // seam later hit-test/stale ids reach, so it must no-op rather than index
    // this arena out of range.
    let foreign_model = MusicTreeModel::from_entries(&viewport_entries());
    let foreign = album_id(&foreign_model, "alpha-11").expect("foreign id");

    assert!(
        !browser.set_marked(foreign, true),
        "a foreign node id is never a mark target"
    );
    assert_eq!(browser.mark_state(foreign), TreeMarkState::Unmarked);
    assert!(
        !browser.set_marked(usize::MAX, true),
        "an out-of-range node id is never a mark target"
    );
    assert_eq!(browser.mark_state(usize::MAX), TreeMarkState::Unmarked);
}

#[test]
fn a_settled_change_invalidates_the_projection_but_a_no_op_reconcile_does_not() {
    let entries = base_entries();
    let model = MusicTreeModel::from_entries(&entries);
    let album_1 = album_id(&model, "album-1").expect("album-1 interned");
    let mut browser = MusicTreeBrowser::new(model);
    frame(&mut browser, 10);

    // The crate caches the projection behind a stamp containing the model's
    // `TreeRevision`; an unchanged revision (identical settled content) keeps
    // the stamp equal, so no rebuild is reported and the offset is untouched.
    assert!(
        !browser.reconcile(&entries),
        "a no-op reconcile holds the cached projection"
    );
    assert_eq!(browser.title_of(album_1), "First Album");

    // A real settled change advances the model revision, the stamp no longer
    // matches, and the projection rebuilds from the new settled content.
    let renamed = vec![alpha("album-1", "Renamed Album")];
    assert!(
        browser.reconcile(&renamed),
        "a changed revision invalidates the cached projection"
    );
    assert_eq!(browser.title_of(album_1), "Renamed Album");
    assert_eq!(
        browser.projection_len(),
        1,
        "only the collapsed root projects"
    );
}

#[test]
fn viewport_keeps_its_offset_and_scrolls_only_the_minimum() {
    let entries = viewport_entries();
    let model = MusicTreeModel::from_entries(&entries);
    let alpha_5 = album_id(&model, "alpha-5").expect("alpha-5 interned");
    let beta_11 = album_id(&model, "beta-11").expect("beta-11 interned");
    let mut browser = MusicTreeBrowser::new(model);
    expand_all_roots(&mut browser);

    // Scroll the selected leaf to the viewport's last row.
    browser.select_id(alpha_5);
    frame(&mut browser, 5);
    assert_eq!(browser.offset(), 2, "the leaf sits at the viewport bottom");
    assert_selection_visible(&browser, 5);

    // A settled insertion *after* the selection leaves the offset exactly
    // where it was (bounds still permit).
    let mut appended = entries.clone();
    appended.push(beta("beta-12", "Beta Twelve"));
    assert!(browser.reconcile(&appended));
    frame(&mut browser, 5);
    assert_eq!(browser.offset(), 2, "the prior viewport row is preserved");
    assert_selection_visible(&browser, 5);

    // A settled insertion *before* the selection pushes the leaf out of the
    // window: the viewport scrolls only the minimum needed to keep it
    // visible, not to the top and not centered.
    let mut prepended = vec![entry(
        "Aardvark",
        ArtistKey::Service("artist-aardvark".into()),
        "Aardvark Album",
        "aardvark-1",
        "1999",
    )];
    prepended.extend(entries.iter().cloned());
    assert!(browser.reconcile(&prepended));
    frame(&mut browser, 5);
    let selected_row = projection_row(&browser, alpha_5);
    assert_eq!(
        browser.offset(),
        selected_row + 1 - 5,
        "only the minimum scroll keeps the selection visible"
    );
    assert_selection_visible(&browser, 5);

    // A jump to the last projection row clamps at the projection bounds.
    browser.select_id(beta_11);
    frame(&mut browser, 5);
    assert_eq!(browser.offset(), browser.projection_len() - 5);
    assert_selection_visible(&browser, 5);
}

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

#[test]
fn matching_album_keeps_cached_track_children_in_the_filtered_projection() {
    let entries = vec![alpha("album-1", "First")];
    let mut model = MusicTreeModel::new();
    let mut tracks = HashMap::new();
    tracks.insert(
        "album-1".into(),
        vec![MusicTreeTrack {
            target: "track-1".into(),
            title: "Track".into(),
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
    assert_eq!(titles, ["Alpha", "First", "Track"]);
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
            },
            MusicTreeTrack {
                target: "track-2".into(),
                title: "Track Two".into(),
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
