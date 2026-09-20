//! Task 2.1 focused cases for the Grouped Music tree model: the
//! destination-local node arena (monotonic non-reused ids, settled ordering,
//! atomic model revision, and node-to-domain translation), refresh retention,
//! deletion tombstoning, equal-name separation, and destination reset. The
//! browser's selection/viewport ownership is task 2.2 scope.

use tui_treelistview::TreeRevision;

use super::{MusicNodeKey, MusicTreeEntry, MusicTreeModel};
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
