//! Focused cases for the Grouped Music tree model and its one state owner,
//! addressed through the stable `MusicTreeTarget` surface: settled grouping and
//! ordering, equal-name separation, fallback grouping, refresh retention and
//! rebuild behaviour, target-to-domain translation, latest-render hit
//! invalidation, selection/expansion/mark retention, absent-target no-ops, and
//! viewport continuity. The arena `usize` and its intern bookkeeping are private
//! to the tree module and are never asserted here.

use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use tui_treelistview::TreeMarkState;

use super::{MusicTreeBrowser, MusicTreeEntry, MusicTreeHit, MusicTreeModel, MusicTreeTarget};
use crate::app::components::media_list::MediaSemanticState;
use crate::app::music_grouping::ArtistKey;

pub(super) fn entry(
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

pub(super) fn alpha(target: &str, title: &str) -> MusicTreeEntry {
    entry(
        "Alpha",
        ArtistKey::Service("artist-alpha".into()),
        title,
        target,
        "2001",
    )
}

pub(super) fn beta(target: &str, title: &str) -> MusicTreeEntry {
    entry(
        "Beta",
        ArtistKey::Service("artist-beta".into()),
        title,
        target,
        "2003",
    )
}

/// The base settled catalog: Alpha with two albums, Beta with one.
pub(super) fn base_entries() -> Vec<MusicTreeEntry> {
    vec![
        alpha("album-1", "First Album"),
        alpha("album-2", "Second Album"),
        beta("album-3", "Beta Session"),
    ]
}

/// A settled Service artist target.
pub(super) fn artist(key: &str) -> MusicTreeTarget {
    MusicTreeTarget::Artist(ArtistKey::Service(key.into()))
}

/// A settled album-leaf target.
pub(super) fn album(target: &str) -> MusicTreeTarget {
    MusicTreeTarget::Album(target.to_string())
}

#[test]
fn settled_ordering_and_grouping_follow_the_entries() {
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&base_entries()));
    let alpha_root = artist("artist-alpha");
    let beta_root = artist("artist-beta");
    browser.expand_root(&alpha_root);
    browser.expand_root(&beta_root);

    assert_eq!(
        browser.projected_node_targets(),
        vec![
            alpha_root.clone(),
            album("album-1"),
            album("album-2"),
            beta_root.clone(),
            album("album-3"),
        ]
    );
    assert_eq!(browser.title_of(&alpha_root), Some("Alpha"));
    assert_eq!(browser.title_of(&beta_root), Some("Beta"));
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
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries));
    let root_1 = artist("artist-1");
    let root_2 = artist("artist-2");
    browser.expand_root(&root_1);
    browser.expand_root(&root_2);

    assert_ne!(root_1, root_2, "equal display names, distinct identities");
    assert_eq!(
        browser.projected_node_targets(),
        vec![
            root_1.clone(),
            album("album-1"),
            root_2.clone(),
            album("album-2"),
        ]
    );
    assert_eq!(browser.title_of(&root_1), Some("Alpha"));
    assert_eq!(browser.title_of(&root_2), Some("Alpha"));
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
    let root = MusicTreeTarget::Artist(ArtistKey::Fallback("Unknown Artist".into()));
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries));
    browser.expand_root(&root);
    assert_eq!(
        browser.projected_node_targets(),
        vec![root.clone(), album("album-1"), album("album-2")]
    );

    assert!(
        !browser.reconcile(&entries),
        "an identical fallback refresh holds the projection"
    );
    assert!(browser.root_is_expanded(&root));
    assert_eq!(browser.projected_node_targets().len(), 3);
}

#[test]
fn refresh_retains_surviving_targets_and_drops_removed_ones() {
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&base_entries()));
    let alpha_root = artist("artist-alpha");
    let beta_root = artist("artist-beta");
    browser.expand_all_roots();
    assert!(browser.root_is_expanded(&alpha_root));

    // Beta's only album leaves the settled catalog: its root and leaf leave the
    // projection while Alpha's surviving targets stay put and expanded.
    let mut replacement = base_entries();
    replacement.retain(|entry| entry.target != "album-3");
    assert!(browser.reconcile(&replacement));
    assert_eq!(
        browser.projected_node_targets(),
        vec![alpha_root.clone(), album("album-1"), album("album-2")]
    );
    assert!(browser.root_is_expanded(&alpha_root));

    // New keys and albums project once their roots are expanded; the removed
    // album's target never returns.
    let mut grown = replacement;
    grown.push(beta("album-4", "New Beta"));
    grown.push(entry(
        "Gamma",
        ArtistKey::Service("artist-gamma".into()),
        "Gamma Album",
        "album-5",
        "",
    ));
    browser.reconcile(&grown);
    browser.expand_all_roots();
    let targets = browser.projected_node_targets();
    assert!(
        targets.contains(&beta_root),
        "the re-grown Beta root projects"
    );
    assert!(targets.contains(&album("album-4")));
    assert!(targets.contains(&artist("artist-gamma")));
    assert!(!targets.contains(&album("album-3")));
}

#[test]
fn destination_reset_clears_every_interned_target() {
    let mut model = MusicTreeModel::from_entries(&base_entries());
    assert!(model.id_of(&album("album-1")).is_some());

    model.reset();
    assert!(
        model.id_of(&album("album-1")).is_none(),
        "no stale mapping survives a destination reset"
    );
}

#[test]
fn target_to_domain_translation_maps_artist_keys_and_album_targets() {
    let key = ArtistKey::Service("artist-alpha".into());
    let root = MusicTreeTarget::Artist(key.clone());
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&base_entries()));

    assert!(browser.select_id(&album("album-2")));
    assert_eq!(browser.selected_target(), Some(album("album-2")));
    assert_eq!(browser.selected_album_target(), Some("album-2"));
    assert_eq!(browser.selected_artist_key(), None);

    assert!(browser.select_id(&root));
    assert_eq!(browser.selected_artist_key(), Some(&key));
    assert_eq!(browser.selected_artist_name(), Some("Alpha"));
    assert_eq!(
        browser.selected_album_target(),
        None,
        "an artist focus resolves to no album"
    );
    assert!(browser.model_is_artist(&root));
    assert!(!browser.model_is_artist(&album("album-2")));
}

#[test]
fn browser_launch_identity_uses_tree_target_and_reports_artist_root_absence() {
    let model = MusicTreeModel::from_entries(&base_entries());
    let mut browser = MusicTreeBrowser::new(model);

    assert_eq!(browser.selected_album_target(), None);
    assert!(browser.select_album_target("album-1"));
    assert_eq!(browser.selected_album_target(), Some("album-1"));
}

/// A two-artist corpus large enough to overflow a five-row viewport: Alpha
/// and Beta with twelve albums each.
pub(super) fn viewport_entries() -> Vec<MusicTreeEntry> {
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

/// One component-level frame at a `VIEW_WIDTH` × `height` browser rect. The
/// owner's viewport, mark aggregate, and hit map settle exactly as they do
/// when the mounted view paints.
pub(super) fn frame(browser: &mut MusicTreeBrowser, height: u16) {
    let area = Rect::new(0, 0, VIEW_WIDTH, height);
    let mut term = Terminal::new(TestBackend::new(area.width, area.height)).expect("test terminal");
    term.draw(|f| browser.view(f, area)).expect("tree frame");
}

/// A visible target's projection row.
pub(super) fn projection_row(browser: &MusicTreeBrowser, target: &MusicTreeTarget) -> usize {
    browser
        .projected_node_targets()
        .iter()
        .position(|candidate| candidate == target)
        .expect("target is projected")
}

pub(super) fn assert_selection_visible(browser: &MusicTreeBrowser, height: usize) {
    let selected = browser.selected_target().expect("a node is selected");
    let row = projection_row(browser, &selected);
    let offset = browser.offset();
    assert!(
        row >= offset && row < offset + height,
        "selection row {row} outside the viewport {offset}..{}",
        offset + height
    );
}

#[test]
fn tree_hit_geometry_is_claimable_only_after_the_latest_view() {
    let model = MusicTreeModel::from_entries(&base_entries());
    let root = artist("artist-alpha");
    let mut browser = MusicTreeBrowser::new(model);
    browser.expand_root(&root);

    let point = Position::new(1, 0);
    frame(&mut browser, 5);
    assert!(browser.claims_point(point));
    assert_eq!(
        browser.hit_region(point),
        Some(MusicTreeHit::Row(root.clone()))
    );

    // An explicit invalidation models a content/area configuration that has
    // happened after the last completed frame. Every pointer-resolution seam
    // must reject the old row until a new view completes.
    browser.invalidate();
    assert!(!browser.claims_point(point));
    assert!(browser.hit_node(point).is_none());
    assert!(browser.hit_region(point).is_none());

    frame(&mut browser, 5);
    assert!(browser.claims_point(point));

    // A settled content replacement invalidates the completed hit map too,
    // even when the surviving root still paints at the same row.
    assert!(browser.reconcile(&[alpha("album-1", "Renamed Album")]));
    assert!(!browser.claims_point(point));
    assert!(browser.hit_region(point).is_none());

    frame(&mut browser, 5);
    assert!(browser.claims_point(point));

    // The panel's viewport/geometry configuration is another invalidation
    // boundary; the old frame cannot claim while the replacement is pending.
    browser.clamp_viewport_to(2);
    assert!(!browser.claims_point(point));
    assert!(browser.hit_region(point).is_none());
}

#[test]
fn settled_replacement_retains_the_selected_node_expansion_and_marks() {
    let model = MusicTreeModel::from_entries(&base_entries());
    let mut browser = MusicTreeBrowser::new(model);
    let alpha_root = artist("artist-alpha");
    let beta_root = artist("artist-beta");
    let album_1 = album("album-1");
    let album_2 = album("album-2");
    browser.expand_root(&alpha_root);
    browser.expand_root(&beta_root);
    browser.select_id(&album_1);

    // Aggregate root state is derived: an artist root is never a stored
    // mark target (design D6).
    assert!(
        !browser.set_marked(&alpha_root, true),
        "artist roots are not stored mark targets"
    );
    assert_eq!(browser.mark_state(&alpha_root), TreeMarkState::Unmarked);
    browser.set_marked(&album_1, true);
    assert_eq!(browser.mark_state(&alpha_root), TreeMarkState::Partial);

    // A settled replacement that still contains the selected leaf and both
    // expanded roots rebuilds the projection but keeps the owner's state.
    let mut grown = base_entries();
    grown.push(beta("album-4", "New Beta"));
    assert!(
        browser.reconcile(&grown),
        "a settled change rebuilds the projection"
    );
    assert_eq!(browser.selected_target(), Some(album_1.clone()));
    assert!(browser.root_is_expanded(&alpha_root));
    assert!(browser.root_is_expanded(&beta_root));
    assert_eq!(browser.projected_node_targets().len(), 6);
    assert_eq!(browser.mark_state(&album_1), TreeMarkState::Marked);
    assert_eq!(browser.mark_state(&alpha_root), TreeMarkState::Partial);
    assert_eq!(browser.mark_state(&beta_root), TreeMarkState::Unmarked);

    // The surviving mark lifts to Marked once the second leaf is marked.
    browser.set_marked(&album_2, true);
    assert_eq!(browser.mark_state(&alpha_root), TreeMarkState::Marked);
}

#[test]
fn marking_an_absent_target_is_a_no_op() {
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&base_entries()));
    let foreign = album("foreign-album");

    assert!(
        !browser.set_marked(&foreign, true),
        "an absent target is never a mark target"
    );
    assert_eq!(browser.mark_state(&foreign), TreeMarkState::Unmarked);
}

#[test]
fn a_settled_change_rebuilds_the_projection_but_a_no_op_reconcile_does_not() {
    let entries = base_entries();
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries));
    frame(&mut browser, 10);

    // The crate caches the projection behind a stamp containing the model's
    // revision; an unchanged revision (identical settled content) keeps the
    // stamp equal, so no rebuild is reported and the offset is untouched.
    assert!(
        !browser.reconcile(&entries),
        "a no-op reconcile holds the cached projection"
    );
    assert_eq!(browser.title_of(&album("album-1")), Some("First Album"));

    // A real settled change advances the model revision, the stamp no longer
    // matches, and the projection rebuilds from the new settled content.
    let renamed = vec![alpha("album-1", "Renamed Album")];
    assert!(
        browser.reconcile(&renamed),
        "a changed revision invalidates the cached projection"
    );
    assert_eq!(browser.title_of(&album("album-1")), Some("Renamed Album"));
    assert_eq!(
        browser.projected_node_targets(),
        vec![artist("artist-alpha")],
        "only the collapsed root projects"
    );
}

#[test]
fn viewport_keeps_its_offset_and_scrolls_only_the_minimum() {
    let entries = viewport_entries();
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries));
    browser.expand_all_roots();

    let alpha_5 = album("alpha-5");
    let beta_11 = album("beta-11");

    // Scroll the selected leaf to the viewport's last row.
    browser.select_id(&alpha_5);
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
    let selected_row = projection_row(&browser, &alpha_5);
    assert_eq!(
        browser.offset(),
        selected_row + 1 - 5,
        "only the minimum scroll keeps the selection visible"
    );
    assert_selection_visible(&browser, 5);

    // A jump to the last projection row clamps at the projection bounds.
    browser.select_id(&beta_11);
    frame(&mut browser, 5);
    assert_eq!(browser.offset(), browser.projected_node_targets().len() - 5);
    assert_selection_visible(&browser, 5);
}
