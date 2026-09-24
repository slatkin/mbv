use super::*;

#[test]
fn incomplete_artist_data_uses_item_tag_when_available() {
    let mut album = make_item("Tagged Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = "Known Artist".into();
    let resolved: HashMap<String, String> = HashMap::new();
    assert_eq!(
        derive_album_artist(&album, resolved.get("album-1").map(String::as_str)),
        "Known Artist"
    );
}

#[test]
fn incomplete_artist_data_falls_back_to_resolved_lookup() {
    let mut album = make_item("Unknown Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = String::new();
    let mut resolved: HashMap<String, String> = HashMap::new();
    resolved.insert("album-1".to_string(), "Fetched Artist".to_string());
    assert_eq!(
        derive_album_artist(&album, resolved.get("album-1").map(String::as_str)),
        "Fetched Artist"
    );
}

#[test]
fn incomplete_artist_data_falls_back_to_folder_name_parse() {
    let mut album = make_item("Pink Floyd (1973) The Dark Side of the Moon", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = String::new();
    let resolved: HashMap<String, String> = HashMap::new();
    assert_eq!(
        derive_album_artist(&album, resolved.get("album-1").map(String::as_str)),
        "Pink Floyd"
    );
}

#[test]
fn incomplete_artist_data_falls_back_to_unknown_artist() {
    let mut album = make_item("Mystery Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = String::new();
    let resolved: HashMap<String, String> = HashMap::new();
    assert_eq!(
        derive_album_artist(&album, resolved.get("album-1").map(String::as_str)),
        "Unknown Artist"
    );
}

#[test]
fn terminal_fallback_uses_empty_cache_tombstone() {
    let mut album = make_item("Tombstone Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = String::new();
    let mut resolved: HashMap<String, String> = HashMap::new();
    resolved.insert("album-1".to_string(), String::new());
    assert_eq!(
        derive_album_artist(&album, resolved.get("album-1").map(String::as_str)),
        "Unknown Artist"
    );
}

#[test]
fn catalog_publication_groups_by_artist() {
    let mut a1 = make_item("First Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    let mut a2 = make_item("Second Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = "Beta".into();
    let mut a3 = make_item("Third Album", "MusicAlbum");
    a3.id = "album-3".into();
    a3.artist = "Alpha".into();

    let resolved: HashMap<String, String> = HashMap::new();
    let catalog = build_grouped_album_catalog(&[a1, a2, a3], &resolved);

    assert_eq!(catalog.entries.len(), 3);
    assert_eq!(catalog.entries[0].artist, "Alpha");
    assert_eq!(catalog.entries[1].artist, "Alpha");
    assert_eq!(catalog.entries[2].artist, "Beta");
}

#[test]
fn catalog_id_lookup_returns_correct_position() {
    let mut a1 = make_item("Alpha Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    let mut a2 = make_item("Beta Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = "Beta".into();

    let resolved: HashMap<String, String> = HashMap::new();
    let catalog = build_grouped_album_catalog(&[a1, a2], &resolved);

    let pos_alpha = catalog.id_to_entry.get("album-1").expect("alpha in lookup");
    let pos_beta = catalog.id_to_entry.get("album-2").expect("beta in lookup");
    assert_eq!(catalog.entries[*pos_alpha].album_id, "album-1");
    assert_eq!(catalog.entries[*pos_beta].album_id, "album-2");
}

#[test]
fn catalog_index_lookup_returns_correct_position() {
    let mut a1 = make_item("Alpha Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    let mut a2 = make_item("Beta Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = "Beta".into();

    let resolved: HashMap<String, String> = HashMap::new();
    let catalog = build_grouped_album_catalog(&[a1, a2], &resolved);

    let pos_0 = catalog.index_to_entry.get(&0).expect("index 0 in lookup");
    let pos_1 = catalog.index_to_entry.get(&1).expect("index 1 in lookup");
    assert_eq!(catalog.entries[*pos_0].album_index, 0);
    assert_eq!(catalog.entries[*pos_1].album_index, 1);
}

#[rstest]
#[case::matched_pair_resolves_service_identity(
    json!([{"name": "Alpha", "id": "artist-1"}]),
    "Alpha",
    ArtistKey::Service("artist-1".into())
)]
#[case::absent_artist_items_falls_back_to_display_artist(
    json!(null),
    "Alpha",
    ArtistKey::Fallback("Alpha".into())
)]
#[case::unmatched_pairs_fall_back_to_display_artist(
    json!([{"name": "Beta", "id": "artist-2"}]),
    "Alpha",
    ArtistKey::Fallback("Alpha".into())
)]
#[case::ambiguous_equal_name_pairs_fall_back_to_display_artist(
    json!([{"name": "Alpha", "id": "artist-1"}, {"name": "Alpha", "id": "artist-2"}]),
    "Alpha",
    ArtistKey::Fallback("Alpha".into())
)]
fn catalog_artist_key_cases(
    #[case] artist_items: serde_json::Value,
    #[case] display_artist: &str,
    #[case] expected: ArtistKey,
) {
    let mut album = make_item("Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = display_artist.into();
    if !artist_items.is_null() {
        album.artist_items = serde_json::from_value(artist_items).unwrap();
    }
    let resolved: HashMap<String, String> = HashMap::new();
    let catalog = build_grouped_album_catalog(&[album], &resolved);
    assert_eq!(catalog.entries[0].artist_key, expected);
}

#[test]
fn equal_display_names_with_distinct_ids_stay_separate() {
    let mut a1 = make_item("Greatest Hits", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    a1.artist_items = vec![mbv_core::api::EmbyArtistRef {
        name: "Alpha".into(),
        id: "artist-1".into(),
    }];
    let mut a2 = make_item("Greatest Hits", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = "Alpha".into();
    a2.artist_items = vec![mbv_core::api::EmbyArtistRef {
        name: "Alpha".into(),
        id: "artist-2".into(),
    }];

    let resolved: HashMap<String, String> = HashMap::new();
    let catalog = build_grouped_album_catalog(&[a1, a2], &resolved);

    let keys: Vec<_> = catalog.entries.iter().map(|e| &e.artist_key).collect();
    assert_ne!(
        keys[0], keys[1],
        "equal display names with distinct valid IDs must stay separate"
    );
    assert_eq!(keys[0], &ArtistKey::Service("artist-1".into()));
    assert_eq!(keys[1], &ArtistKey::Service("artist-2".into()));
}

#[test]
fn fallback_keys_are_stable_across_rebuilds_and_input_order() {
    let mut a1 = make_item("Bravo Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Bravo".into();
    let mut a2 = make_item("Alpha Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = "Alpha".into();

    let resolved: HashMap<String, String> = HashMap::new();
    let forward = build_grouped_album_catalog(&[a1.clone(), a2.clone()], &resolved);
    let reversed = build_grouped_album_catalog(&[a2.clone(), a1.clone()], &resolved);
    let again = build_grouped_album_catalog(&[a1, a2], &resolved);

    let key_of = |catalog: &crate::app::state::music_grouping::GroupedAlbumCatalog,
                  album_id: &str| {
        catalog.entries[catalog.id_to_entry[album_id]]
            .artist_key
            .clone()
    };
    for album_id in ["album-1", "album-2"] {
        assert_eq!(key_of(&forward, album_id), key_of(&reversed, album_id));
        assert_eq!(key_of(&forward, album_id), key_of(&again, album_id));
    }
    // The fallback derives from the settled grouping identity (the display
    // artist), never from display position.
    assert_eq!(
        key_of(&forward, "album-1"),
        ArtistKey::Fallback("Bravo".into())
    );
    assert_eq!(
        key_of(&forward, "album-2"),
        ArtistKey::Fallback("Alpha".into())
    );
}

#[test]
fn repeated_same_items_produce_stable_catalog_order() {
    let mut a1 = make_item("Bravo Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Bravo".into();
    let mut a2 = make_item("Alpha Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = "Alpha".into();

    let resolved: HashMap<String, String> = HashMap::new();
    let catalog1 = build_grouped_album_catalog(&[a1.clone(), a2.clone()], &resolved);
    let catalog2 = build_grouped_album_catalog(&[a1, a2], &resolved);

    assert_eq!(
        catalog1
            .entries
            .iter()
            .map(|e| e.artist.as_str())
            .collect::<Vec<_>>(),
        catalog2
            .entries
            .iter()
            .map(|e| e.artist.as_str())
            .collect::<Vec<_>>(),
        "same items should produce identical catalog order"
    );
}

#[test]
fn catalog_uses_resolved_over_item_tag() {
    let mut a1 = make_item("Tagged Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut resolved: HashMap<String, String> = HashMap::new();
    resolved.insert("album-1".to_string(), "Fetched Artist".to_string());
    let catalog = build_grouped_album_catalog(&[a1], &resolved);
    assert_eq!(catalog.entries[0].artist, "Fetched Artist");
}
