//! Grouped Music tree-owner content and selection contracts: the projected
//! row/label/hero content, and the selection-summary state the owner reports.

use super::tree_fixtures::{find, paint_tree, tree_owner, tree_owner_with_tracks, tree_point};
use super::*;
use crate::app::components::music_content::workspace::build_track_rows;

/// double-click/Right Hero entry while filtered Enter stays local.
#[test]
fn artist_roots_are_hero_eligible_only_when_unfiltered() {
    use crate::app::components::library_panel::owner::LibraryContentOwner;

    let mut owner = artist_workspace_owner();
    assert!(
        owner.hero_overlay_target_available(),
        "an artist root is a hero-bearing row"
    );
    assert!(
        owner.hero_overlay_enter_available(),
        "an unfiltered artist root enters its Hero"
    );

    owner.on_key(&KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    });
    owner
        .browser
        .apply(TreeOperation::EditFilter("Alpha".to_string()));
    assert!(
        !owner.hero_overlay_enter_available(),
        "a filtered artist root keeps Enter local"
    );

    let mut album_owner = tree_owner(&[("Alpha", &["a-0"])]);
    assert!(album_owner.hero_overlay_enter_available());
    assert!(album_owner.hero_overlay_target_available());
}

#[test]
fn wide_album_metadata_removes_artist_and_year_prefix() {
    // The old `wide_album_metadata` characterization (rehomed here by task
    // 9.2): a tagged album whose display name still carries the
    // `Artist (Year) Title` folder prefix must present the bare title and
    // the parsed release year, even though `derive_album_display_name`
    // leaves a tagged album's name untouched.
    let mut album = make_item("Bob Dylan (1970) New Morning", "MusicAlbum");
    album.artist = "Bob Dylan".into();
    album.production_year = 1970;

    assert_eq!(
        wide_album_metadata(&album, "Bob Dylan"),
        ("New Morning".to_string(), 1970)
    );
}

#[test]
fn resolved_hero_data_uses_parsed_title_year_and_cached_artist() {
    // The `album_artist_cache` fallback names the artist; the folder-name
    // parse supplies the title/year the Wide hero presents.
    let mut owner = MusicContent::new();
    let mut album = make_item("Folder Artist (2024) First Album", "MusicAlbum");
    album.artist.clear();
    album.production_year = 0;
    let mut ctx = context(album, "overview");
    ctx.album_info = vec![("Folder Artist".into(), "2024".into(), "First Album".into())];
    owner.set_content(ctx);

    let data = owner.hero_data().expect("hero data");
    assert_eq!(data.facts.title, "First Album");
    assert_eq!(data.facts.meta_rows, vec!["Folder Artist", "2024"]);
}

/// Music never tracks played/progress in its rows: a played track and a
/// half-played track both project the ordinary row (mbv never resumes a music
/// track, so its stored position means nothing either).
#[test]
fn played_tracks_project_the_ordinary_state() {
    let mut played = make_item("Finished Track", "Audio");
    played.played = true;
    let mut half_played = make_item("Half-Played Track", "Audio");
    half_played.runtime_ticks = 1000;
    half_played.playback_position_ticks = 500;
    let fresh = make_item("Fresh Track", "Audio");
    let rows = build_track_rows(&[played, half_played, fresh]);
    let states: Vec<&MediaSemanticState> = rows
        .iter()
        .map(|row| match row {
            MediaListRow::Item { semantic_state, .. } => semantic_state,
            _ => panic!("track rows are items"),
        })
        .collect();
    assert_eq!(states[0], &MediaSemanticState::Ordinary);
    assert_eq!(states[1], &MediaSemanticState::Ordinary);
    assert_eq!(states[2], &MediaSemanticState::Ordinary);
}

#[test]
fn track_rows_project_runtime_in_the_green_gutter() {
    let mut track = make_item("Track", "Audio");
    track.runtime_ticks = 65 * TICKS_PER_SECOND;
    let rows = build_track_rows(&[track]);
    let MediaListRow::Item {
        trailing, duration, ..
    } = &rows[0]
    else {
        panic!("track rows are items");
    };
    assert_eq!(trailing, &Some(MediaListTrailing::Gutter("1:05".into())));
    assert_eq!(duration, &None);
}

#[test]
fn tree_tracks_use_numbered_workspace_labels_with_index_fallback() {
    let mut indexed = make_item("Indexed Track", "Audio");
    indexed.id = "track-indexed".into();
    indexed.index_number = 7;
    let mut fallback = make_item("Fallback Track", "Audio");
    fallback.id = "track-fallback".into();
    fallback.index_number = 0;

    let rows = build_track_rows(&[indexed.clone(), fallback.clone()]);
    let labels: Vec<&str> = rows
        .iter()
        .map(|row| match row {
            MediaListRow::Item { primary, .. } => primary.as_str(),
            _ => panic!("track rows are items"),
        })
        .collect();
    assert_eq!(labels, ["7. Indexed Track", "2. Fallback Track"]);

    let mut owner = tree_owner_with_tracks(&[("Alpha", &["a-0"])], Some(vec![indexed, fallback]));
    owner.expand_all_tree_roots();
    // The all-node fixture expansion already revealed the cached track rows.
    let labels: Vec<&str> = owner
        .browser
        .visible_targets()
        .iter()
        .filter_map(|target| owner.browser.node(target).map(|node| node.title.as_str()))
        .filter(|title| title.contains("Track"))
        .collect();
    assert_eq!(labels, ["7. Indexed Track", "2. Fallback Track"]);
}

// ── Task 2.4: tree chord mapping through the component boundary ──────────

/// A multi-artist Grouped Music owner whose tree starts on the first album of
/// the first root (`selected_album` drives the initial adoption). `artists` is
/// `(artist, [album target…])` in settled order; the derived title is the

#[test]
fn clearing_grouped_music_marks_removes_the_selection_summary() {
    let mut owner = tree_owner(&[("Alpha", &["a-0"])]);
    owner.set_selection_origin(crate::app::components::media_list::SelectionOrigin::Queue);
    owner.expand_all_tree_roots();
    let album = find(&owner, |target| target.album_leaf_target() == Some("a-0"));

    paint_tree(&mut owner, Rect::new(0, 0, 40, 10));
    let point = tree_point(&owner, &album);
    owner.browser.apply(TreeOperation::PointerToggleMark(point));
    assert_eq!(
        owner.selection_summary().map(|summary| summary.count),
        Some(1)
    );

    owner.clear_selection();
    assert_eq!(owner.selection_summary(), None);
}
