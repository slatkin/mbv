use super::*;
use mbv_emby_model::{EmbyArtistRef, EmbyImageTags, EmbyPerson};
use rstest::rstest;
use serde_json::json;

// ── parse_item ───────────────────────────────────────────────────────────

#[test]
fn parse_item_basic_fields() {
    let raw = json!({
        "Id": "abc", "Name": "Test", "Type": "Movie",
        "IsFolder": false, "MediaType": "Video",
        "RunTimeTicks": 36_000_000_000i64,
        "DateCreated": "2026-09-21T19:39:41.1234567Z",
        "SortName": "test",
        "UserData": { "Played": true, "PlaybackPositionTicks": 5_000_000i64 }
    });
    let item = parse_item(&raw);
    assert_eq!(item.id, "abc");
    assert_eq!(item.name, "Test");
    assert_eq!(item.runtime_ticks, 36_000_000_000);
    assert_eq!(item.date_added, "2026-09-21T19:39:41.1234567Z");
    assert!(item.played);
    assert_eq!(item.playback_position_ticks, 5_000_000);
}

#[rstest]
#[case::explicit_zero(json!({"Type": "Folder", "IsFolder": true, "ChildCount": 0}), Some(0))]
#[case::absent(json!({"Type": "Folder", "IsFolder": true}), None)]
fn parse_item_child_count(#[case] raw: serde_json::Value, #[case] expected: Option<u32>) {
    assert_eq!(parse_item(&raw).child_count, expected);
}

#[test]
fn parse_item_metadata_lists() {
    let item = parse_item(&json!({
        "Type": "Movie",
        "Genres": ["Action", "Drama"],
        "People": [
            {"Name": "Director", "Type": "Director"},
            {"Name": "Actor", "Role": "Hero", "Type": "Actor"}
        ],
        "ExternalUrls": [
            {"Name": "IMDb", "Url": "https://imdb.example/movie"},
            {"Name": "Blank", "Url": ""}
        ]
    }));
    assert_eq!(item.genres, vec!["Action", "Drama"]);
    assert_eq!(
        item.people[0],
        EmbyPerson {
            name: "Director".into(),
            role: String::new(),
            kind: "Director".into()
        }
    );
    assert_eq!(item.people[1].role, "Hero");
    assert_eq!(item.external_urls[0].name, "IMDb");
    assert_eq!(item.external_urls[1].url, "");
}

#[test]
fn parse_item_metadata_lists_absent_default_empty() {
    let item = parse_item(&json!({"Type": "Movie"}));
    assert_eq!(item.genres, [] as [std::string::String; 0]);
    assert_eq!(item.people, [] as [mbv_emby_model::EmbyPerson; 0]);
    assert_eq!(item.external_urls, [] as [mbv_emby_model::EmbyLink; 0]);
}

#[rstest]
#[case::collection_folder("CollectionFolder")]
#[case::channel("Channel")]
#[case::music_album("MusicAlbum")]
#[case::music_artist("MusicArtist")]
#[case::series("Series")]
fn parse_item_forces_is_folder(#[case] item_type: &str) {
    let raw = json!({ "Type": item_type, "IsFolder": false, "UserData": {} });
    assert!(parse_item(&raw).is_folder);
}

#[test]
fn parse_item_missing_fields_use_defaults() {
    let item = parse_item(&json!({}));
    assert_eq!(item.id, "");
    assert_eq!(item.runtime_ticks, 0);
    assert!(!item.played);
    assert!(!item.is_folder);
    assert_eq!(item.image_tags, EmbyImageTags::default());
}

// ── parse_item: declared image availability (task 5.3) ─────────────────

#[test]
fn parse_item_image_tags_when_present() {
    let raw = json!({
        "Type": "Movie",
        "ImageTags": { "Thumb": "thumb-tag", "Primary": "primary-tag", "Logo": "logo-tag" },
        "BackdropImageTags": ["backdrop-a", "backdrop-b"],
        "UserData": {}
    });
    let item = parse_item(&raw);
    assert_eq!(item.image_tags.thumb, "thumb-tag");
    assert_eq!(item.image_tags.primary, "primary-tag");
    assert_eq!(item.image_tags.logo, "logo-tag");
    assert_eq!(item.image_tags.backdrops, vec!["backdrop-a", "backdrop-b"]);
}

#[test]
fn parse_item_image_tags_absent_members_default_empty() {
    // Only `Primary` declared: `Thumb` and the backdrop list default empty.
    let raw = json!({ "Type": "Movie", "ImageTags": { "Primary": "primary-tag" } });
    let item = parse_item(&raw);
    assert_eq!(item.image_tags.thumb, "");
    assert_eq!(item.image_tags.primary, "primary-tag");
    assert_eq!(item.image_tags.logo, "");
    assert_eq!(item.image_tags.backdrops, [] as [std::string::String; 0]);
    assert_eq!(item.image_tags.series_thumb, "");
    assert_eq!(
        item.image_tags.series_backdrops,
        [] as [std::string::String; 0]
    );
}

#[test]
fn parse_item_without_image_tags_defaults_empty() {
    let item = parse_item(&json!({ "Type": "Movie" }));
    assert_eq!(item.image_tags, EmbyImageTags::default());
}

#[test]
fn parse_item_episode_series_image_tags() {
    // An episode reports the series' artwork as `ParentThumbImageTag` /
    // `ParentBackdropImageTags`.
    let raw = json!({
        "Type": "Episode", "SeriesName": "Lost",
        "ParentThumbImageTag": "series-thumb",
        "ParentBackdropImageTags": ["series-backdrop"],
        "UserData": {}
    });
    let item = parse_item(&raw);
    assert_eq!(item.image_tags.series_thumb, "series-thumb");
    assert_eq!(item.image_tags.series_backdrops, vec!["series-backdrop"]);
    assert_eq!(item.image_tags.thumb, "");
}

#[test]
fn parse_item_episode_series_thumb_tag_falls_back_to_series_level_tag() {
    let raw = json!({
        "Type": "Episode",
        "SeriesThumbImageTag": "series-thumb",
        "UserData": {}
    });
    let item = parse_item(&raw);
    assert_eq!(item.image_tags.series_thumb, "series-thumb");
}

#[test]
fn parse_item_parent_thumb_tag_wins_over_series_level_tag() {
    let raw = json!({
        "Type": "Episode",
        "ParentThumbImageTag": "parent",
        "SeriesThumbImageTag": "series",
        "UserData": {}
    });
    assert_eq!(parse_item(&raw).image_tags.series_thumb, "parent");
}

#[test]
fn parse_item_episode_fields() {
    let raw = json!({
        "Type": "Episode", "Name": "Pilot",
        "SeriesName": "Lost", "IndexNumber": 1, "ParentIndexNumber": 2,
        "UserData": {}
    });
    let item = parse_item(&raw);
    assert_eq!(item.series_name, "Lost");
    assert_eq!(item.index_number, 1);
    assert_eq!(item.parent_index_number, 2);
}

// ── parse_item: audio and music folder types ─────────────────────────────

#[test]
fn parse_item_audio_not_folder() {
    let raw = json!({ "Type": "Audio", "MediaType": "Audio", "UserData": {} });
    let item = parse_item(&raw);
    assert_eq!(item.item_type, "Audio");
    assert_eq!(item.media_type, "Audio");
    assert!(!item.is_folder);
}

#[rstest]
#[case::parse_item_artist_from_album_artist_field(json!({ "Type": "Audio", "AlbumArtist": "Pink Floyd", "UserData": {} }), "Pink Floyd")]
#[case::parse_item_artist_falls_back_to_artists_array(json!({ "Type": "Audio", "Artists": ["David Bowie"], "UserData": {} }), "David Bowie")]
#[case::parse_item_album_artist_takes_priority_over_artists_array(json!({ "Type": "Audio", "AlbumArtist": "Album Artist", "Artists": ["Track Artist"], "UserData": {} }), "Album Artist")]
fn parse_item_artist_cases(#[case] raw: serde_json::Value, #[case] expected: &str) {
    assert_eq!(parse_item(&raw).artist, expected);
}

// ── parse_item: ArtistItems identity pairs (task 1.2) ──────────────────────────────

#[test]
fn parse_item_artist_items_retained_when_present() {
    let item = parse_item(&json!({
        "Type": "MusicAlbum",
        "ArtistItems": [
            {"Name": "Alpha", "Id": "artist-1"},
            {"Name": "Beta", "Id": "artist-2"}
        ]
    }));
    assert_eq!(
        item.artist_items,
        vec![
            EmbyArtistRef {
                name: "Alpha".into(),
                id: "artist-1".into()
            },
            EmbyArtistRef {
                name: "Beta".into(),
                id: "artist-2".into()
            },
        ]
    );
}

#[test]
fn parse_item_artist_items_absent_default_empty() {
    assert_eq!(
        parse_item(&json!({"Type": "MusicAlbum"})).artist_items,
        [] as [mbv_emby_model::EmbyArtistRef; 0]
    );
}

// ── parse_video_info ─────────────────────────────────────────────────────

#[rstest]
#[case::four_k(3840, 2160, "hevc", "4K HEVC")]
#[case::one_zero_eighty_p(1920, 1080, "h264", "1080p H264")]
#[case::seven_twenty_p(1280, 720, "h264", "720p H264")]
#[case::codec_only_when_no_resolution(0, 0, "vp9", "VP9")]
fn parse_video_info_cases(
    #[case] width: i64,
    #[case] height: i64,
    #[case] codec: &str,
    #[case] expected: &str,
) {
    let streams = json!([{"Type": "Video", "Width": width, "Height": height, "Codec": codec}]);
    assert_eq!(parse_video_info(streams.as_array().unwrap()), expected);
}

#[test]
fn parse_video_info_empty_when_no_video_stream() {
    let streams = json!([{"Type": "Audio", "Codec": "aac"}]);
    assert_eq!(parse_video_info(streams.as_array().unwrap()), "");
}

// ── parse_audio_info ─────────────────────────────────────────────────────

fn audio_stream(lang: &str, codec: &str, layout: &str) -> serde_json::Value {
    json!({"Type": "Audio", "Language": lang, "Codec": codec, "ChannelLayout": layout})
}

#[rstest]
#[case::parse_audio_info_multiple_tracks(json!([
    audio_stream("eng", "ac3", "5.1"),
    audio_stream("fra", "aac", "stereo"),
]), "English AC3 5.1  |  French AAC Stereo")]
#[case::parse_audio_info_unknown_lang_omitted_from_label(json!([
    audio_stream("und", "aac", "stereo"),
]), "AAC Stereo")]
#[case::parse_audio_info_skips_non_audio_streams(json!([
    {"Type": "Video", "Language": "eng", "Codec": "h264", "ChannelLayout": ""},
    audio_stream("eng", "aac", "stereo"),
]), "English AAC Stereo")]
fn parse_audio_info_cases(#[case] streams: serde_json::Value, #[case] expected: &str) {
    assert_eq!(parse_audio_info(streams.as_array().unwrap()), expected);
}

// Sync guard: every ISO code in parse_audio_info must produce the same English
// name as lang_code_to_name() in player.rs. Both tables must be updated together.
// The mirror test in player.rs::tests::lang_code_to_name_matches_api_table checks
// the other side.
#[test]
fn parse_audio_info_lang_table_matches_player_lang_code_to_name() {
    let cases: &[(&str, &str)] = &[
        ("en", "English"),
        ("eng", "English"),
        ("fr", "French"),
        ("fre", "French"),
        ("fra", "French"),
        ("de", "German"),
        ("ger", "German"),
        ("deu", "German"),
        ("es", "Spanish"),
        ("spa", "Spanish"),
        ("it", "Italian"),
        ("ita", "Italian"),
        ("pt", "Portuguese"),
        ("por", "Portuguese"),
        ("ja", "Japanese"),
        ("jpn", "Japanese"),
        ("ko", "Korean"),
        ("kor", "Korean"),
        ("zh", "Chinese"),
        ("chi", "Chinese"),
        ("zho", "Chinese"),
        ("ru", "Russian"),
        ("rus", "Russian"),
        ("ar", "Arabic"),
        ("ara", "Arabic"),
        ("nl", "Dutch"),
        ("nld", "Dutch"),
        ("dut", "Dutch"),
        ("sv", "Swedish"),
        ("swe", "Swedish"),
        ("no", "Norwegian"),
        ("nor", "Norwegian"),
        ("da", "Danish"),
        ("dan", "Danish"),
        ("fi", "Finnish"),
        ("fin", "Finnish"),
        ("pl", "Polish"),
        ("pol", "Polish"),
        ("cs", "Czech"),
        ("cze", "Czech"),
        ("ces", "Czech"),
        ("tr", "Turkish"),
        ("tur", "Turkish"),
    ];
    for (code, expected) in cases {
        let streams =
            json!([{"Type": "Audio", "Language": code, "Codec": "", "ChannelLayout": ""}]);
        let result = parse_audio_info(streams.as_array().unwrap());
        assert_eq!(
            result, *expected,
            "parse_audio_info: code {code:?} → expected {expected:?}, got {result:?}"
        );
    }
}

#[test]
fn parse_session_media_info_extracts_remote_stream_options() {
    let streams = json!([
        {"Type": "Video", "Width": 1920, "Height": 1080, "Codec": "h264"},
        {"Type": "Audio", "Index": 1, "Language": "eng", "Codec": "ac3", "ChannelLayout": "5.1"},
        {"Type": "Audio", "Index": 2, "Language": "jpn", "Codec": "aac", "ChannelLayout": "stereo"},
        {"Type": "Subtitle", "Index": 3, "Language": "eng", "IsForced": false},
        {"Type": "Subtitle", "Index": 4, "Language": "eng", "IsForced": true}
    ]);
    let media = parse_session_media_info(streams.as_array().unwrap());
    assert_eq!(media.video_label, "1080p H264");
    assert!(!media.audio_only);
    assert_eq!(media.audio_streams.len(), 2);
    assert_eq!(media.audio_streams[0].index, 1);
    assert_eq!(media.audio_streams[0].label, "English AC3 5.1");
    assert_eq!(media.audio_streams[1].label, "Japanese AAC Stereo");
    assert_eq!(media.subtitle_streams.len(), 2);
    assert_eq!(media.subtitle_streams[0].label, "English");
    assert_eq!(media.subtitle_streams[1].label, "English (Forced)");
}

#[test]
fn parse_session_media_info_handles_audio_only_sessions() {
    let streams = json!([
        {"Type": "Audio", "Index": 0, "Language": "eng", "Codec": "flac", "ChannelLayout": "stereo"}
    ]);
    let media = parse_session_media_info(streams.as_array().unwrap());
    assert!(media.audio_only);
    assert_eq!(media.video_label, "English FLAC Stereo");
    assert_eq!(media.audio_streams.len(), 1);
    assert_eq!(media.audio_streams[0].index, 0);
}
