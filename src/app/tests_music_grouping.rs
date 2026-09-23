use super::app_struct::LevelFillState;
use super::library_browse_actions::retain_grouped_music_items;
use super::music_grouping::{build_grouped_album_catalog, derive_album_artist, ArtistKey};
use super::tests::{make_app_stub, make_item, make_items};
use super::types_events::LibEvent;
use super::{BrowseLevel, LibraryTab, TabSelection};
use crate::app::types_browse::BrowseResting;
use mbv_core::api::EmbyItem;
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use rstest::rstest;

fn make_music_album_level(albums: Vec<EmbyItem>) -> BrowseLevel {
    BrowseLevel {
        fetched_rows: 0,
        parent_id: "group-0".into(),
        title: "Alpha".into(),
        items: albums,
        total_count: 0,
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
            tv_content_mode: None,
        music_grouping: None,
    }
}

fn make_group_level() -> BrowseLevel {
    let mut group = make_item("Alpha", "MusicArtist");
    group.id = "group-0".into();
    group.is_folder = true;
    BrowseLevel {
        fetched_rows: 0,
        parent_id: "lib-music".into(),
        title: "Music".into(),
        items: vec![group],
        total_count: 1,
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
            tv_content_mode: None,
        music_grouping: None,
    }
}

#[rstest]
#[case::empty_folder((true, Some(0u32), false))]
#[case::unknown_count((true, None, true))]
#[case::non_folder((false, Some(0u32), true))]
fn grouped_music_filter_cases(#[case] fixture: (bool, Option<u32>, bool)) {
    let (is_folder, child_count, kept) = fixture;
    let mut item = make_item("candidate", "Folder");
    item.is_folder = is_folder;
    item.child_count = child_count;
    let mut items = vec![item];
    retain_grouped_music_items(&mut items, true);
    assert_eq!(items.is_empty(), !kept);
}

#[test]
fn non_grouped_music_filter_keeps_empty_folder() {
    let mut item = make_item("candidate", "Folder");
    item.is_folder = true;
    item.child_count = Some(0);
    let mut items = vec![item];
    retain_grouped_music_items(&mut items, false);
    assert_eq!(items.len(), 1);
}

#[test]
fn grouped_refresh_filters_at_event_boundary_and_keeps_server_row_count() {
    let mut app = make_music_app(Vec::new());
    let mut empty = make_group_item("empty", "Empty");
    empty.child_count = Some(0);
    let kept = make_group_item("kept", "Kept");

    app.handle_lib_event(LibEvent::Refreshed {
        lib_idx: 0,
        parent_id: "group-0".into(),
        item_types: None,
        unplayed_only: false,
        items: vec![empty, kept],
        total_count: 2,
    });

    let level = app.libs[0].nav_stack.last().unwrap();
    assert_eq!(level.items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(), ["kept"]);
    assert_eq!(level.fetched_rows, 2);
    assert_eq!(level.total_count, 2);
}

#[test]
fn grouped_refresh_clamps_cursor_after_empty_folder_filter() {
    let mut app = make_music_app(Vec::new());
    app.libs[0].nav_stack.last_mut().unwrap().resting = BrowseResting::new(1, 1);
    let mut empty = make_group_item("empty", "Empty");
    empty.child_count = Some(0);
    let kept = make_group_item("kept", "Kept");

    app.handle_lib_event(LibEvent::Refreshed {
        lib_idx: 0,
        parent_id: "group-0".into(),
        item_types: None,
        unplayed_only: false,
        items: vec![empty, kept],
        total_count: 2,
    });

    let level = app.libs[0].nav_stack.last().unwrap();
    assert_eq!(level.resting().cursor(), 0);
    let position = level.to_position_level();
    assert_eq!(position.cursor_index, 0);
    assert_eq!(position.focused_item_id.as_deref(), Some("kept"));
    assert_eq!(position.fetched_rows, Some(2));
}

#[test]
fn grouped_chain_landing_filters_every_level_and_preserves_server_rows() {
    let mut app = make_music_app(Vec::new());
    let mut root = make_group_level();
    root.items.clear();
    let mut empty_root = make_group_item("empty-root", "Empty root");
    empty_root.child_count = Some(0);
    let kept_root = make_group_item("kept-root", "Kept root");
    root.items.extend([empty_root, kept_root]);
    root.total_count = root.items.len();

    let mut child = make_music_album_level(Vec::new());
    let mut empty_child = make_group_item("empty-child", "Empty child");
    empty_child.child_count = Some(0);
    let kept_child = make_group_item("kept-child", "Kept child");
    child.items = vec![empty_child, kept_child];
    child.total_count = child.items.len();
    child.fetched_rows = child.items.len();

    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: super::types_events::NavigateLanding::Chain {
            nav_stack: vec![root, child],
        },
        switch_tab: false,
    });

    assert_eq!(app.libs[0].nav_stack.len(), 2);
    for (level, (expected_id, expected_fetched_rows)) in app.libs[0]
        .nav_stack
        .iter()
        .zip([("kept-root", 2), ("kept-child", 2)])
    {
        assert_eq!(
            level.items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
            [expected_id]
        );
        assert_eq!(level.fetched_rows, expected_fetched_rows);
        assert_eq!(level.to_position_level().fetched_rows, Some(expected_fetched_rows));
    }
}

fn make_music_library_tab() -> LibraryTab {
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();
    LibraryTab::new(library)
}

fn make_group_item(id: &str, name: &str) -> EmbyItem {
    let mut group = make_item(name, "Folder");
    group.id = id.into();
    group.is_folder = true;
    group
}

fn make_untagged_album(id: &str) -> EmbyItem {
    let mut album = make_item("Unknown Album", "MusicAlbum");
    album.id = id.into();
    album
}

/// A music library whose grouped view has not been opened: no nav stack,
/// the state startup warm-up (tasks 3.1, design D5) runs against.
fn make_unopened_music_app() -> super::App {
    let mut app = make_app_stub();
    app.music_levels = vec!["group".into(), "album".into()];
    app.libs.push(make_music_library_tab());
    app
}

fn make_music_app(albums: Vec<EmbyItem>) -> super::App {
    let mut app = make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.music_levels = vec!["group".into(), "album".into()];

    app.libs.push(LibraryTab {
        nav_stack: vec![make_group_level(), make_music_album_level(albums)],
        ..make_music_library_tab()
    });
    app
}

#[test]
fn focused_artist_arms_next_page_regardless_of_child_proximity() {
    // The grouped album level paginates to completion unconditionally
    // (client-side artist grouping needs the whole folder, and tree scroll
    // position doesn't correspond to flat-array position once artists
    // collapse/expand), so even a target near the top of the loaded items
    // arms the next page.
    let mut app = make_music_app(make_items(30));
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    level.fetched_rows = 30;
    level.total_count = 100;

    app.maybe_fetch_next_page_for_music_artist(0, &["id4".into()]);
    assert!(app.libs[0].nav_stack.last().unwrap().loading);
}

#[test]
fn focused_artist_with_no_loaded_child_does_not_arm_a_page() {
    let mut app = make_music_app(make_items(30));
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    level.fetched_rows = 30;
    level.total_count = 100;

    app.maybe_fetch_next_page_for_music_artist(0, &["not-loaded".into()]);
    assert!(!app.libs[0].nav_stack.last().unwrap().loading);
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

// ── Artist identity through the settled catalog (task 1.3) ──────────────

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

    let key_of = |catalog: &super::music_grouping::GroupedAlbumCatalog, album_id: &str| {
        catalog.entries[catalog.id_to_entry[album_id]].artist_key.clone()
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
        level.set_resting_cursor(1);
    }

    // Second settle (replacement) should anchor to album-2
    let items = app.libs[0].nav_stack.last().unwrap().items.clone();
    if let Some(level) = app.libs[0].nav_stack.last_mut() {
        level.items = items;
    }
    app.start_or_supersede_music_grouping(0);

    let level = app.libs[0].nav_stack.last().unwrap();
    let cursor_id = level.items[level.resting().cursor()].id.clone();
    assert_eq!(
        cursor_id, "album-2",
        "cursor should be anchored to the previously selected album"
    );
}

#[test]
fn level_event_bulk_fills_cache_and_settles_candidate() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut a2 = make_item("Other Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = String::new();
    let mut app = make_music_app(vec![a1, a2]);
    app.start_or_supersede_music_grouping(0);
    app.album_artist_levels
        .insert("level-1".into(), LevelFillState::Loading { orphan_risk: false });

    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "level-1".into(),
        artists: vec![
            ("album-1".into(), "Artist One".into()),
            ("album-2".into(), "Artist Two".into()),
        ],
    });

    assert_eq!(
        app.album_artist_levels.get("level-1"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );
    assert_eq!(
        app.album_artist_cache.get("album-1").map(String::as_str),
        Some("Artist One")
    );
    assert_eq!(
        app.album_artist_cache.get("album-2").map(String::as_str),
        Some("Artist Two")
    );
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(
        state.candidate.is_none(),
        "arrival resolves every waiting album at once"
    );
    let catalog = state.settled.as_ref().expect("settled catalog");
    assert_eq!(catalog.entries[0].artist, "Artist One");
    assert_eq!(catalog.entries[1].artist, "Artist Two");
}

#[test]
fn level_event_empty_artists_marks_failed_without_filling() {
    let mut app = make_music_app(vec![]);
    app.album_artist_levels
        .insert("level-1".into(), LevelFillState::Loading { orphan_risk: false });

    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "level-1".into(),
        artists: vec![],
    });

    assert_eq!(
        app.album_artist_levels.get("level-1"),
        Some(&LevelFillState::Failed)
    );
    assert!(app.album_artist_cache.is_empty());
}

#[test]
fn service_reset_clears_album_artist_state() {
    let mut app = make_music_app(vec![]);
    app.album_artist_cache.insert("album-1".into(), "A".into());
    app.album_artist_levels
        .insert("level-1".into(), LevelFillState::Filled { orphan_risk: false });
    app.pending_level_artist_warmups.push_back("level-2".into());
    app.level_artist_warmups_in_flight.insert("level-3".into());

    app.remove_emby_confirmed();

    assert!(app.album_artist_cache.is_empty());
    assert!(app.album_artist_levels.is_empty());
    assert!(app.pending_level_artist_warmups.is_empty());
    assert!(app.level_artist_warmups_in_flight.is_empty());
}

#[test]
fn level_event_empty_artist_pair_fills_only_non_empty_pair() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut a2 = make_item("Other Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = String::new();
    let mut app = make_music_app(vec![a1, a2]);
    app.start_or_supersede_music_grouping(0);
    app.album_artist_levels
        .insert("group-0".into(), LevelFillState::Loading { orphan_risk: false });

    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "group-0".into(),
        artists: vec![
            ("album-1".into(), String::new()),
            ("album-2".into(), "Artist Two".into()),
        ],
    });

    // The empty-artist pair must not poison the cache with an empty
    // tombstone; only the non-empty pair fills.
    assert_eq!(
        app.album_artist_cache.get("album-1"),
        None,
        "empty artist must not be cached"
    );
    assert_eq!(
        app.album_artist_cache.get("album-2").map(String::as_str),
        Some("Artist Two")
    );
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );
    // The arrival still resolves every waiting album: the empty-artist
    // album settles to the folder fallback instead of the cache.
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(state.candidate.is_none());
    let catalog = state.settled.as_ref().expect("settled catalog");
    assert_eq!(catalog.entries[0].album_id, "album-2");
    assert_eq!(catalog.entries[0].artist, "Artist Two");
    assert_eq!(catalog.entries[1].album_id, "album-1");
    assert_eq!(catalog.entries[1].artist, "Unknown Artist");
}

#[test]
fn filled_level_with_unresolvable_albums_settles_immediately() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut app = make_music_app(vec![a1]);
    app.album_artist_levels
        .insert("group-0".into(), LevelFillState::Filled { orphan_risk: false });

    app.start_or_supersede_music_grouping(0);

    // The fill already had its chance: the album is terminal via the
    // fallback, with no candidate left waiting on `SETTLE_WINDOW`.
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
fn warmup_orphan_risk_gets_one_browse_upgrade_then_stays_terminal() {
    let mut app = make_music_app(vec![make_untagged_album("album-1")]);
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Filled { orphan_risk: true },
    );

    // With the test stub's absent client, this transition proves the
    // browse-triggered upgrade was requested rather than falling back from
    // the warm-up Filled state.
    app.start_or_supersede_music_grouping(0);
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed)
    );
    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap()
        .candidate
        .is_some());

    // Model the one upgrade's arrival with an unknown artist. It clears the
    // orphan risk, while the waiting album takes the existing fallback path.
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Loading { orphan_risk: false },
    );
    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "group-0".into(),
        artists: vec![("album-1".into(), String::new())],
    });
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );

    // A later candidate does not request another fill, even though the
    // album remains unresolved after the upgrade.
    app.start_or_supersede_music_grouping(0);
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );
    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap()
        .candidate
        .is_none());
}

#[test]
fn candidate_filled_level_is_terminal_without_orphan_upgrade() {
    let mut app = make_music_app(vec![make_untagged_album("album-1")]);
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Filled { orphan_risk: false },
    );

    app.start_or_supersede_music_grouping(0);

    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap()
        .candidate
        .is_none());
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );
}

#[test]
fn orphan_risk_filled_level_with_no_unresolved_items_does_not_upgrade() {
    let mut tagged = make_untagged_album("album-1");
    tagged.artist = "Tagged Artist".into();
    let mut app = make_music_app(vec![tagged]);
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Filled { orphan_risk: true },
    );

    app.start_or_supersede_music_grouping(0);

    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: true })
    );
    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap()
        .candidate
        .is_none());
}

#[test]
fn stale_warmup_listing_is_ignored_by_emby_generation() {
    let mut app = make_unopened_music_app();
    let status_before = app.status.clone();
    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: mbv_core::service_runtime::SetupGeneration::new(1),
        groups: vec![make_group_item("stale-group", "Stale")],
    });

    assert!(app.album_artist_levels.is_empty());
    assert!(app.pending_level_artist_warmups.is_empty());
    assert_eq!(app.status, status_before);
    assert!(app.libs[0].nav_stack.is_empty());
}

#[test]
fn current_generation_warmup_listing_is_accepted() {
    let mut app = make_unopened_music_app();
    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: mbv_core::service_runtime::SetupGeneration::default(),
        groups: vec![make_group_item("current-group", "Current")],
    });

    assert_eq!(
        app.album_artist_levels.get("current-group"),
        Some(&LevelFillState::Failed)
    );
    assert!(app.libs[0].nav_stack.is_empty());
}

#[test]
fn page_two_albums_resolve_from_whole_level_fill_without_new_request() {
    // Page-starvation regression (whole-level coverage): a fill requested
    // while only page-1 albums were listed still bulk-fills every album in
    // the level, so a page-2 candidate resolves from the cache instead of
    // clearing as terminal under the `Filled` state.
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut app = make_music_app(vec![a1]);
    app.start_or_supersede_music_grouping(0);

    // One whole-level fill arrives, covering a page-2 album that was never
    // part of the listing in hand when the fill was requested.
    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "group-0".into(),
        artists: vec![
            ("album-1".into(), "Artist One".into()),
            ("album-2".into(), "Artist Two".into()),
        ],
    });
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );

    // The user pages on: album-2 is appended and a new candidate is created.
    let mut a2 = make_item("Other Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = String::new();
    if let Some(level) = app.libs[0].nav_stack.last_mut() {
        level.items.push(a2);
    }
    app.start_or_supersede_music_grouping(0);

    // album-2 resolved up front from the fill's cache row; the `Filled`
    // level did not starve it into the folder fallback.
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(state.candidate.is_none());
    let catalog = state.settled.as_ref().expect("settled catalog");
    let pos = catalog.id_to_entry["album-2"];
    assert_eq!(catalog.entries[pos].artist, "Artist Two");
}

#[test]
fn spawn_level_fetch_dedupes_on_loading_and_filled() {
    let mut app = make_music_app(vec![]);
    let albums = Vec::new();

    app.album_artist_levels
        .insert("group-0".into(), LevelFillState::Loading { orphan_risk: false });
    app.spawn_level_artist_fetch("group-0".into(), albums.clone());
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Loading { orphan_risk: false }),
        "Loading level must not be re-requested"
    );

    app.album_artist_levels
        .insert("group-0".into(), LevelFillState::Filled { orphan_risk: false });
    app.spawn_level_artist_fetch("group-0".into(), albums);
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false }),
        "Filled level must not be re-requested"
    );
}

#[test]
fn spawn_level_fetch_without_client_marks_failed_for_retry() {
    let mut app = make_music_app(vec![]); // stub has no Emby client

    app.spawn_level_artist_fetch("group-0".into(), Vec::new());

    // `Failed` (not a stuck `Loading`): the next candidate creation for
    // the level retries, per design D4.
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed)
    );
}

#[test]
fn failed_level_retries_on_next_candidate_creation() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut app = make_music_app(vec![a1]);

    app.start_or_supersede_music_grouping(0);
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed),
        "no-client stub fails the fill attempt"
    );

    app.start_or_supersede_music_grouping(0);
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed),
        "Failed level was retried (and failed again without a client)"
    );
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(
        state.candidate.is_some(),
        "candidate stays unresolved waiting for the retry's arrival"
    );
}

#[test]
fn warmup_fan_out_stays_bounded_before_level_arrivals() {
    let mut app = make_unopened_music_app();
    for index in 0..6 {
        let level_id = format!("in-flight-{index}");
        app.album_artist_levels
            .insert(level_id.clone(), LevelFillState::Loading { orphan_risk: false });
        app.level_artist_warmups_in_flight.insert(level_id);
    }

    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: Default::default(),
        groups: (0..7)
            .map(|index| make_group_item(&format!("group-{index}"), "Group"))
            .collect(),
    });

    assert_eq!(app.level_artist_warmups_in_flight.len(), 6);
    assert_eq!(
        app.pending_level_artist_warmups.len(),
        7,
        "warm-up levels beyond the six request slots stay queued"
    );
}

#[test]
fn pending_warmup_is_removed_when_candidate_wins_the_level_race() {
    let mut app = make_music_app(vec![make_untagged_album("album-1")]);
    app.album_artist_levels
        .insert("group-0".into(), LevelFillState::Loading { orphan_risk: false });
    app.pending_level_artist_warmups.push_back("group-0".into());

    app.spawn_level_artist_fetch("group-0".into(), Vec::new());

    assert!(app.pending_level_artist_warmups.is_empty());
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Loading { orphan_risk: false }),
        "the candidate observes the existing warm-up request rather than spawning"
    );
}

#[test]
fn warmup_group_id_matches_the_opened_level_parent_id() {
    let group = make_group_item("group-0", "A-D");
    let album_level = make_music_album_level(vec![make_untagged_album("album-1")]);

    assert_eq!(group.id, album_level.parent_id);
}

#[test]
fn warmup_listing_requests_one_fill_per_group_child_without_a_view() {
    let mut app = make_unopened_music_app();

    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: Default::default(),
        groups: vec![
            make_group_item("group-0", "A-D"),
            make_group_item("group-1", "E-H"),
        ],
    });

    // The stub has no Emby client, so each requested fill immediately marks
    // its level `Failed` for retry — the established client-less observable
    // for "a fill was requested". Both group children were requested.
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed)
    );
    assert_eq!(
        app.album_artist_levels.get("group-1"),
        Some(&LevelFillState::Failed)
    );
    // The view was never opened: browsing state is untouched.
    assert!(app.libs[0].nav_stack.is_empty());
    assert!(app.album_artist_cache.is_empty());
}

#[test]
fn warmup_library_selection_gates_on_group_config_and_music_collection() {
    let mut app = make_unopened_music_app();
    assert_eq!(
        app.music_group_warmup_library_ids(),
        vec!["lib-music".to_string()]
    );

    // Not a group-first level config: no warm-up targets (the same gate
    // `is_music_group_view` applies).
    app.music_levels = vec!["album".into()];
    assert!(app.music_group_warmup_library_ids().is_empty());

    // Group config restored, but the library is not music.
    app.music_levels = vec!["group".into(), "album".into()];
    app.libs[0].library.collection_type = "movies".into();
    assert!(app.music_group_warmup_library_ids().is_empty());
}

#[test]
fn warmup_dedupes_on_loading_and_filled_levels() {
    let mut app = make_unopened_music_app();
    app.album_artist_levels
        .insert("group-0".into(), LevelFillState::Loading { orphan_risk: false });

    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: Default::default(),
        groups: vec![make_group_item("group-0", "A-D")],
    });

    // An in-flight level does no work through the same
    // `LevelFillState::action_for` decision candidates use. In this
    // client-less stub a re-request would have cycled `Loading` -> `Failed`,
    // so `Loading` surviving proves the warm-up spawned nothing.
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Loading { orphan_risk: false })
    );

    app.album_artist_levels
        .insert("group-1".into(), LevelFillState::Filled { orphan_risk: false });
    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: Default::default(),
        groups: vec![make_group_item("group-1", "E-H")],
    });
    assert_eq!(
        app.album_artist_levels.get("group-1"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );
}

#[test]
fn warmup_and_candidate_share_one_fill_decision() {
    let mut app = make_music_app(vec![make_untagged_album("album-1")]);
    // Seed the in-flight state a real warm-up spawn marks (the stub has no
    // client, so simulate it).
    app.album_artist_levels
        .insert("group-0".into(), LevelFillState::Loading { orphan_risk: false });

    // Opening the grouped view while warm-up is in flight: the candidate
    // takes the NoWork arm and waits instead of starting a second fill.
    app.start_or_supersede_music_grouping(0);
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(state.candidate.is_some());

    // The warm-up listing arrives too: still no second fill for the level.
    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: Default::default(),
        groups: vec![make_group_item("group-0", "A-D")],
    });
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Loading { orphan_risk: false })
    );
}

#[test]
fn warmup_fill_failure_marks_failed_and_leaves_browsing_untouched() {
    let mut app = make_unopened_music_app();
    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: Default::default(),
        groups: vec![make_group_item("group-0", "A-D")],
    });
    let status_before = app.status.clone();

    // The per-level fill failed: empty artists is the HTTP-failure shape.
    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "group-0".into(),
        artists: vec![],
    });

    // Failed (retryable), and otherwise silent: no cache fill, no status/
    // toast, no queue, no browsing state.
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed)
    );
    assert!(app.album_artist_cache.is_empty());
    assert_eq!(app.status, status_before);
    assert!(app.player_tab.all_queue_items().is_empty());
    assert!(app.libs[0].nav_stack.is_empty());

    // Grouped browsing remains usable: opening the level settles through
    // the existing fallback within the grouping resolution window.
    app.libs[0].nav_stack = vec![
        make_group_level(),
        make_music_album_level(vec![make_untagged_album("album-1")]),
    ];
    app.start_or_supersede_music_grouping(0);
    // Force the settle window to have elapsed, as the fallback test does.
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
