use super::*;
use rstest::rstest;
use serde_json::json;

fn make_item(name: &str, item_type: &str) -> EmbyItem {
    EmbyItem {
        id: "id".into(),
        name: name.into(),
        item_type: item_type.into(),
        is_folder: false,
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
        image_tags: Default::default(),
    }
}

// ── EmbyItem::display_name ──────────────────────────────────────────────

#[test]
fn display_name_episode_without_series_falls_back_to_name() {
    let item = make_item("Standalone", "Episode");
    assert_eq!(item.display_name(), "Standalone");
}

#[test]
fn display_name_parts_splits_episode_from_series_title() {
    let mut item = make_item("Episode Title", "Episode");
    item.series_name = "Severance".into();
    assert_eq!(
        item.display_name_parts(),
        ("Severance".into(), Some("Episode Title".into()))
    );
    let standalone = make_item("Movie", "Movie");
    assert_eq!(standalone.display_name_parts(), ("Movie".into(), None));
}

// ── parse_item ───────────────────────────────────────────────────────────

#[test]
fn parse_item_basic_fields() {
    let raw = json!({
        "Id": "abc", "Name": "Test", "Type": "Movie",
        "IsFolder": false, "MediaType": "Video",
        "RunTimeTicks": 36_000_000_000i64,
        "SortName": "test",
        "UserData": { "Played": true, "PlaybackPositionTicks": 5_000_000i64 }
    });
    let item = parse_item(&raw);
    assert_eq!(item.id, "abc");
    assert_eq!(item.name, "Test");
    assert_eq!(item.runtime_ticks, 36_000_000_000);
    assert!(item.played);
    assert_eq!(item.playback_position_ticks, 5_000_000);
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
    assert_eq!(item.people[0], EmbyPerson { name: "Director".into(), role: "".into(), kind: "Director".into() });
    assert_eq!(item.people[1].role, "Hero");
    assert_eq!(item.external_urls[0].name, "IMDb");
    assert_eq!(item.external_urls[1].url, "");
}

#[test]
fn parse_item_metadata_lists_absent_default_empty() {
    let item = parse_item(&json!({"Type": "Movie"}));
    assert!(item.genres.is_empty());
    assert!(item.people.is_empty());
    assert!(item.external_urls.is_empty());
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
fn image_tags_serde_defaults_logo_for_legacy_items() {
    let tags: EmbyImageTags = serde_json::from_value(json!({"thumb": "thumb-tag"})).unwrap();
    assert_eq!(tags.logo, "");
    let serialized = serde_json::to_value(tags).unwrap();
    assert_eq!(serialized["thumb"], "thumb-tag");
    assert_eq!(serialized["logo"], "");
}

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
    assert!(item.image_tags.backdrops.is_empty());
    assert_eq!(item.image_tags.series_thumb, "");
    assert!(item.image_tags.series_backdrops.is_empty());
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
    assert!(item.image_tags.thumb.is_empty());
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

// ── EmbyItem::playback_label ────────────────────────────────────────────

#[rstest]
#[case::playback_label_audio_without_artist_falls_back_to_display_name("Song", "Audio", "Song")]
#[case::playback_label_video_uses_display_name("Inception", "Movie", "Inception")]
fn playback_label_cases(#[case] name: &str, #[case] item_type: &str, #[case] expected: &str) {
    let item = make_item(name, item_type);
    assert_eq!(item.playback_label(), expected);
}

// ── EmbyItem::file_name / sort_key ──────────────────────────────────────

#[test]
fn file_name_extracts_from_path() {
    let mut item = make_item("Movie", "Movie");
    item.path = "/media/movies/Inception (2010).mkv".into();
    assert_eq!(item.file_name(), "Inception (2010).mkv");
}

#[test]
fn file_name_falls_back_to_name_when_path_empty() {
    let item = make_item("Inception", "Movie");
    assert_eq!(item.file_name(), "Inception");
}

#[test]
fn sort_key_prefers_path_filename() {
    let mut item = make_item("Movie", "Movie");
    item.path = "/media/A.mkv".into();
    item.sort_name = "sort".into();
    assert_eq!(item.sort_key(), "A.mkv");
}

#[test]
fn sort_key_falls_back_to_sort_name() {
    let mut item = make_item("Movie", "Movie");
    item.sort_name = "inception the".into();
    assert_eq!(item.sort_key(), "inception the");
}

#[test]
fn sort_key_falls_back_to_name() {
    let item = make_item("Movie", "Movie");
    assert_eq!(item.sort_key(), "Movie");
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

// ── decode_entities ─────────────────────────────────────────────────────

#[test]
fn decode_entities_known_entities() {
    assert_eq!(decode_entities("&quot;hi&quot;"), "\"hi\"");
    assert_eq!(decode_entities("it&apos;s"), "it's");
    assert_eq!(decode_entities("a &lt; b &gt; c"), "a < b > c");
    assert_eq!(decode_entities("a &amp; b"), "a & b");
}

#[test]
fn decode_entities_passthrough() {
    assert_eq!(decode_entities("plain text"), "plain text");
    assert_eq!(decode_entities(""), "");
}

#[test]
fn decode_entities_numeric_refs() {
    assert_eq!(decode_entities("&#38;"), "&");
    assert_eq!(decode_entities("&#x27;"), "'");
    assert_eq!(decode_entities("&#x27A1;"), "➡");
    // Unknown named/numeric refs and stray '&' are left untouched.
    assert_eq!(decode_entities("&unknown;"), "&unknown;");
    assert_eq!(decode_entities("a & b"), "a & b");
    assert_eq!(decode_entities("50% &amp;amp; chance"), "50% &amp; chance");
}

// ── html_to_text ─────────────────────────────────────────────────────────

#[test]
fn html_to_text_paragraph_breaks_and_entities() {
    assert_eq!(
        html_to_text("<p>First paragraph</p><p>Second &amp; final</p>"),
        "First paragraph\nSecond & final"
    );
    // `<br/>` (self-closing and bare) makes a single line break.
    assert_eq!(html_to_text("Line one<br/>Line two"), "Line one\nLine two");
    // Adjacent block tags collapse to one newline, not blank lines.
    assert_eq!(html_to_text("<p>One</p><br/><p>Two</p>"), "One\nTwo");
}

#[test]
fn html_to_text_keeps_link_text_and_url() {
    assert_eq!(
        html_to_text(r#"<p>See <a href="https://example.test/a&amp;b">the article</a>.</p>"#),
        "See the article (https://example.test/a&b)."
    );
}

#[test]
fn html_to_text_drops_inline_formatting_and_images() {
    // Inline spans/strong keep their text; the whole `<img.../>` tag (with its
    // many attributes) is dropped entirely.
    assert_eq!(
        html_to_text(
            r#"<p><span style="font-weight: 400;">Rightwing &amp; left</span><img width="534" src="https://example.test/x.png" alt="" /></p>"#
        ),
        "Rightwing & left"
    );
}

#[test]
fn html_to_text_numeric_entities() {
    // Curly quotes and ellipses from the Novara feed decode via numeric refs.
    assert_eq!(
        html_to_text("&#8220;quoted&#8221; and &#8230;"),
        "\u{201c}quoted\u{201d} and \u{2026}"
    );
}

#[test]
fn html_to_text_plain_passthrough() {
    assert_eq!(html_to_text("plain text"), "plain text");
    assert_eq!(html_to_text(""), "");
    // A stray unknown tag is dropped from the text (it is markup).
    assert_eq!(html_to_text("no <tags> here"), "no here");
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
            "parse_audio_info: code {:?} → expected {:?}, got {:?}",
            code, expected, result
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

// ── should_resume ────────────────────────────────────────────────────────

#[rstest]
#[case::should_resume_zero_position_returns_false(0, 0, false)]
#[case::should_resume_negative_position_returns_false(0, -1, false)]
#[case::should_resume_mid_way_returns_true(TICKS_PER_SECOND * 7200, TICKS_PER_SECOND * 3600, true)]
#[case::should_resume_under_six_percent_returns_false(TICKS_PER_SECOND * 7200, TICKS_PER_SECOND * 60, false)]
#[case::should_resume_exactly_six_percent_returns_true(TICKS_PER_SECOND * 100, TICKS_PER_SECOND * 6, true)]
#[case::should_resume_just_below_six_percent_returns_false(TICKS_PER_SECOND * 100, TICKS_PER_SECOND * 6 - 1, false)]
#[case::should_resume_with_unknown_runtime_returns_true(0, TICKS_PER_SECOND * 60, true)]
fn should_resume_cases(
    #[case] runtime_ticks: i64,
    #[case] playback_position_ticks: i64,
    #[case] expected: bool,
) {
    let mut item = make_item("X", "Movie");
    item.runtime_ticks = runtime_ticks;
    item.playback_position_ticks = playback_position_ticks;
    assert_eq!(item.should_resume(), expected);
}
