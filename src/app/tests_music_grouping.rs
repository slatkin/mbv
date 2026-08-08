use super::music_grouping::{build_grouped_album_catalog, derive_album_artist};
use super::tests::{make_app_stub, make_item};
use super::{BrowseLevel, LibraryTab};
use mbv_core::api::EmbyItem;
use std::collections::HashMap;
use std::time::{Duration, Instant};

fn make_music_album_level(albums: Vec<EmbyItem>) -> BrowseLevel {
    BrowseLevel {
        parent_id: "group-0".into(),
        title: "Alpha".into(),
        items: albums,
        total_count: 0,
        cursor: 0,
        scroll: 0,
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    }
}

fn make_group_level() -> BrowseLevel {
    let mut group = make_item("Alpha", "MusicArtist");
    group.id = "group-0".into();
    group.is_folder = true;
    BrowseLevel {
        parent_id: "lib-music".into(),
        title: "Music".into(),
        items: vec![group],
        total_count: 1,
        cursor: 0,
        scroll: 0,
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    }
}

fn make_music_app(albums: Vec<EmbyItem>) -> super::App {
    let mut app = make_app_stub();
    app.library_tab = 1;
    app.music_levels = vec!["group".into(), "album".into()];

    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();

    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: vec![make_group_level(), make_music_album_level(albums)],
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app
}

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

    assert_eq!(catalog.groups.len(), 2, "should have two artist groups");
    assert_eq!(catalog.groups[0].artist, "Alpha");
    assert_eq!(catalog.groups[1].artist, "Beta");
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

#[test]
fn start_or_supersede_creates_candidate_for_music_group_level() {
    let mut a1 = make_item("First Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    let mut app = make_music_app(vec![a1]);

    app.start_or_supersede_music_grouping(0);

    let level = app.libs[0].nav_stack.last().unwrap();
    let state = level.music_grouping.as_ref().expect("grouping state");
    // All albums have artist tags, so the candidate commits immediately.
    assert_eq!(state.revision, 1);
    assert!(
        state.settled.is_some(),
        "should settle immediately when all terminal"
    );
}

#[test]
fn start_or_supersede_schedules_lookups_for_unresolved_albums() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut app = make_music_app(vec![a1]);

    app.start_or_supersede_music_grouping(0);

    let level = app.libs[0].nav_stack.last().unwrap();
    let state = level.music_grouping.as_ref().expect("grouping state");
    let candidate = state.candidate.as_ref().expect("candidate");
    assert!(
        candidate.unresolved.contains("album-1"),
        "album without artist should be unresolved"
    );
    assert!(
        app.album_artist_loading.contains("album-1"),
        "artist fetch should be scheduled"
    );
}

#[test]
fn advance_removes_from_unresolved_and_resolves() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut app = make_music_app(vec![a1]);
    app.start_or_supersede_music_grouping(0);

    app.advance_music_grouping_candidates("album-1", "Resolved Artist");

    let level = app.libs[0].nav_stack.last().unwrap();
    let state = level.music_grouping.as_ref().expect("grouping state");
    assert!(
        state.candidate.is_none(),
        "candidate should be committed after all resolved"
    );
    let catalog = state.settled.as_ref().expect("settled catalog");
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].artist, "Resolved Artist");
}

#[test]
fn silent_artist_lookups_expire_to_fallback() {
    let mut album = make_item("Unknown Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = String::new();
    let mut app = make_music_app(vec![album]);
    app.start_or_supersede_music_grouping(0);

    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .music_grouping
        .as_mut()
        .unwrap()
        .candidate
        .as_mut()
        .unwrap()
        .created_at = Instant::now() - Duration::from_secs(4);

    app.expire_music_grouping_candidates();

    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(state.candidate.is_none());
    assert_eq!(
        state.settled.as_ref().unwrap().entries[0].artist,
        "Unknown Artist"
    );
}

#[test]
fn obsolete_candidate_does_not_commit() {
    let mut a1 = make_item("Album One", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut a2 = make_item("Album Two", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = String::new();
    let mut app = make_music_app(vec![a1, a2]);
    app.start_or_supersede_music_grouping(0);

    // Advance the first album to settle the candidate
    app.advance_music_grouping_candidates("album-1", "Artist One");

    // Replace items and restart grouping (supersedes)
    let mut a3 = make_item("Album Three", "MusicAlbum");
    a3.id = "album-3".into();
    a3.artist = String::new();
    let mut a4 = make_item("Album Four", "MusicAlbum");
    a4.id = "album-4".into();
    a4.artist = String::new();

    if let Some(level) = app.libs[0].nav_stack.last_mut() {
        level.items = vec![a3, a4];
    }
    app.start_or_supersede_music_grouping(0);

    // The old album-2 result should not affect the new candidate
    app.advance_music_grouping_candidates("album-2", "Stale Artist");

    let level = app.libs[0].nav_stack.last().unwrap();
    let state = level.music_grouping.as_ref().expect("grouping state");
    if let Some(candidate) = &state.candidate {
        assert!(
            !candidate.resolved.contains_key("album-2"),
            "stale result must not be in the new candidate"
        );
    }
}

#[test]
fn catalog_preserves_artist_identity_across_settle() {
    let mut a1 = make_item("Alpha Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    let mut a2 = make_item("Beta Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = "Beta".into();
    let mut app = make_music_app(vec![a1, a2]);

    app.start_or_supersede_music_grouping(0);

    let level = app.libs[0].nav_stack.last().unwrap();
    let state = level.music_grouping.as_ref().expect("grouping state");
    let catalog = state.settled.as_ref().expect("settled catalog");
    assert_eq!(catalog.entries.len(), 2);
    assert_eq!(catalog.entries[0].artist, "Alpha");
    assert_eq!(catalog.entries[1].artist, "Beta");
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
    assert_eq!(
        catalog1
            .groups
            .iter()
            .map(|g| g.artist.as_str())
            .collect::<Vec<_>>(),
        catalog2
            .groups
            .iter()
            .map(|g| g.artist.as_str())
            .collect::<Vec<_>>(),
        "same items should produce identical group boundaries"
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

#[test]
fn commit_anchors_cursor_to_selected_album() {
    let mut a1 = make_item("Alpha Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    let mut a2 = make_item("Beta Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = "Beta".into();
    let mut app = make_music_app(vec![a1, a2]);

    // First settle
    app.start_or_supersede_music_grouping(0);
    // Set cursor to album-2
    if let Some(level) = app.libs[0].nav_stack.last_mut() {
        level.cursor = 1;
    }

    // Second settle (replacement) should anchor to album-2
    let items = app.libs[0].nav_stack.last().unwrap().items.clone();
    if let Some(level) = app.libs[0].nav_stack.last_mut() {
        level.items = items;
    }
    app.start_or_supersede_music_grouping(0);

    let level = app.libs[0].nav_stack.last().unwrap();
    let cursor_id = level.items[level.cursor].id.clone();
    assert_eq!(
        cursor_id, "album-2",
        "cursor should be anchored to the previously selected album"
    );
}
