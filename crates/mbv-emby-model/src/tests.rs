use super::*;
use rstest::rstest;

fn item(name: &str, item_type: &str) -> EmbyItem {
    EmbyItem {
        id: "id".into(),
        name: name.into(),
        item_type: item_type.into(),
        is_folder: false,
        child_count: None,
        studios: Vec::new(),
        media_type: "Video".into(),
        collection_type: String::new(),
        runtime_ticks: 0,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        track_artist: String::new(),
        artist_items: Vec::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genres: Vec::new(),
        people: Vec::new(),
        external_urls: Vec::new(),
        playlist_item_id: String::new(),
        image_tags: EmbyImageTags::default(),
    }
}

#[test]
fn ticks_per_second_f64_matches_integer_constant() {
    assert_eq!(
        TICKS_PER_SECOND_F64.to_bits(),
        f64::from(i32::try_from(TICKS_PER_SECOND).unwrap()).to_bits()
    );
}

#[test]
fn ticks_seconds_round_trip_preserves_representative_ticks() {
    assert_eq!(seconds_to_ticks(ticks_to_seconds(0)), 0);
    assert_eq!(
        seconds_to_ticks(ticks_to_seconds(TICKS_PER_SECOND)),
        TICKS_PER_SECOND
    );
    let day_ticks = 24 * 60 * 60 * TICKS_PER_SECOND;
    assert_eq!(seconds_to_ticks(ticks_to_seconds(day_ticks)), day_ticks);
}

#[test]
fn seconds_to_ticks_truncates_fractional_ticks() {
    assert_eq!(seconds_to_ticks(1.9 / TICKS_PER_SECOND_F64), 1);
    assert_eq!(seconds_to_ticks(-1.9 / TICKS_PER_SECOND_F64), -1);
}

#[test]
fn seconds_to_ticks_saturates_and_maps_nan_to_zero() {
    assert_eq!(seconds_to_ticks(f64::NAN), 0);
    assert_eq!(seconds_to_ticks(f64::MAX), i64::MAX);
    assert_eq!(seconds_to_ticks(-f64::MAX), i64::MIN);
    assert_eq!(saturating_i64_from_f64(f64::INFINITY), i64::MAX);
    assert_eq!(saturating_i64_from_f64(f64::NEG_INFINITY), i64::MIN);
    assert_eq!(saturating_i64_from_f64(f64::MAX), i64::MAX);
    assert_eq!(saturating_i64_from_f64(-f64::MAX), i64::MIN);
}

#[test]
fn emby_item_display_names() {
    let mut episode = item("Episode Title", "Episode");
    episode.series_name = "Severance".into();
    assert_eq!(episode.display_name(), "Severance Episode Title");
    assert_eq!(
        episode.display_name_parts(),
        ("Severance".into(), Some("Episode Title".into()))
    );
    assert_eq!(item("Standalone", "Episode").display_name(), "Standalone");
}

#[rstest]
#[case::audio_without_artist("Song", "Audio", "Song")]
#[case::video("Inception", "Movie", "Inception")]
fn emby_item_playback_label(#[case] name: &str, #[case] item_type: &str, #[case] expected: &str) {
    assert_eq!(item(name, item_type).playback_label(), expected);
}

#[test]
fn image_tags_serde_defaults_logo_for_legacy_items() {
    let tags: EmbyImageTags =
        serde_json::from_value(serde_json::json!({"thumb": "thumb-tag"})).unwrap();
    assert_eq!(tags.logo, "");
    let serialized = serde_json::to_value(tags).unwrap();
    assert_eq!(serialized["thumb"], "thumb-tag");
    assert_eq!(serialized["logo"], "");
}

#[test]
fn emby_item_path_controls_file_name_and_sort_key() {
    let mut item = item("Movie", "Movie");
    item.path = "/media/movies/Inception (2010).mkv".into();
    item.sort_name = "sort".into();
    assert_eq!(item.file_name(), "Inception (2010).mkv");
    assert_eq!(item.sort_key(), "Inception (2010).mkv");
    item.path.clear();
    assert_eq!(item.file_name(), "Movie");
    assert_eq!(item.sort_key(), "sort");
    item.sort_name.clear();
    assert_eq!(item.sort_key(), "Movie");
}

#[test]
fn emby_item_artist_identity_requires_one_matching_pair() {
    let mut item = item("Album", "MusicAlbum");
    item.artist_items = vec![EmbyArtistRef {
        name: "Alpha".into(),
        id: "artist-1".into(),
    }];
    assert_eq!(item.matched_artist_item_id(" alpha "), Some("artist-1"));
    item.artist_items.push(EmbyArtistRef {
        name: "Alpha".into(),
        id: "artist-2".into(),
    });
    assert_eq!(item.matched_artist_item_id("Alpha"), None);
}

#[rstest]
#[case::one_matching_pair("Alpha", "artist-1", Some("artist-1"))]
#[case::trimmed_case_insensitive_match(" alpha ", "artist-1", Some("artist-1"))]
#[case::empty_id("Alpha", "", None)]
#[case::empty_display_name("", "artist-1", None)]
fn emby_item_artist_identity_cases(
    #[case] display_name: &str,
    #[case] id: &str,
    #[case] expected: Option<&str>,
) {
    let mut item = item("Album", "MusicAlbum");
    item.artist_items = vec![EmbyArtistRef {
        name: "Alpha".into(),
        id: id.into(),
    }];
    assert_eq!(item.matched_artist_item_id(display_name), expected);
}

#[rstest]
#[case::zero_position(0, 0, false)]
#[case::negative_position(-1, 0, false)]
#[case::midway(TICKS_PER_SECOND * 7200, TICKS_PER_SECOND * 3600, true)]
#[case::under_one_percent(TICKS_PER_SECOND * 7200, TICKS_PER_SECOND * 60, false)]
#[case::exactly_one_percent(TICKS_PER_SECOND * 100, TICKS_PER_SECOND, true)]
#[case::below_one_percent(TICKS_PER_SECOND * 100, TICKS_PER_SECOND - 1, false)]
#[case::unknown_runtime(0, TICKS_PER_SECOND * 60, true)]
fn should_resume_cases(
    #[case] runtime_ticks: i64,
    #[case] position_ticks: i64,
    #[case] expected: bool,
) {
    assert_eq!(should_resume(position_ticks, runtime_ticks), expected);
}

#[test]
fn emby_item_should_resume_uses_its_saved_position() {
    let mut item = item("Movie", "Movie");
    item.runtime_ticks = TICKS_PER_SECOND * 100;
    item.playback_position_ticks = TICKS_PER_SECOND;
    assert!(item.should_resume());
}

#[test]
fn emby_item_music_classification_includes_audio_album_and_artist() {
    assert!(item("Track", "Audio").is_music());
    assert!(item("Album", "MusicAlbum").is_music());
    assert!(item("Artist", "MusicArtist").is_music());
    assert!(!item("Movie", "Movie").is_music());
    assert!(!item("Episode", "Episode").is_music());
    assert!(!item("Series", "Series").is_music());
}
