use rstest::rstest;

use crate::player::{
    active_item_state, advance_decision, parse_tracks, queue_next_up_decision, resolve_jump_target,
    seek_decision, select_tracks, standalone_next_up_decision, volume_decision, NextUp, NextUpFire,
    SubtitlePrefs, TrackInfo,
};

type AudioTracks = Vec<(i64, String)>;
type SubtitleTracks = Vec<(i64, String, bool)>;

fn tracks() -> (AudioTracks, SubtitleTracks) {
    (
        vec![
            (1, "English AAC Stereo".into()),
            (2, "French AAC Stereo".into()),
        ],
        vec![
            (3, "English".into(), false),
            (4, "French (Forced)".into(), true),
            (5, "English SDH".into(), false),
        ],
    )
}

#[rstest]
#[case::default_leaves_subtitle_unchanged("Default", "English", "fr", None)]
#[case::none_disables_subtitles("None", "", "fr", Some(None))]
#[case::forced_prefers_requested_language("OnlyForced", "French", "en", Some(Some(4)))]
#[case::always_matches_requested_language("Always", "French", "en", Some(Some(4)))]
#[case::smart_hides_matching_audio_language("Smart", "English", "en", Some(None))]
#[case::smart_selects_when_audio_differs("Smart", "French", "en", Some(Some(4)))]
#[case::hearing_impaired_prefers_sdh("HearingImpaired", "French", "fr", Some(Some(5)))]
#[case::only_forced_falls_back_to_any_forced("OnlyForced", "German", "en", Some(Some(4)))]
#[case::always_falls_back_to_first("Always", "German", "en", Some(Some(3)))]
#[case::unknown_mode_leaves_selection_unchanged("Unknown", "French", "en", None)]
fn subtitle_modes_choose_expected_track(
    #[case] mode: &str,
    #[case] subtitle_lang: &str,
    #[case] audio_lang: &str,
    #[case] expected: Option<Option<i64>>,
) {
    let (audio, subtitles) = tracks();
    let prefs = SubtitlePrefs {
        mode: mode.into(),
        subtitle_lang: subtitle_lang.into(),
        audio_lang: audio_lang.into(),
    };
    let (_, selected) = select_tracks(&audio, &subtitles, 1, "en", &prefs);
    assert_eq!(selected, expected);
}

#[rstest]
#[case::selects_preferred_track(1, "en", "French", Some(2))]
#[case::keeps_matching_current_track(2, "fr", "French", None)]
#[case::leaves_selection_when_no_match(1, "de", "German", None)]
fn audio_preference_selects_matching_track_only_when_needed(
    #[case] current_id: i64,
    #[case] current_lang: &str,
    #[case] preference: &str,
    #[case] expected: Option<i64>,
) {
    let (audio, subtitles) = tracks();
    let prefs = SubtitlePrefs {
        audio_lang: preference.into(),
        ..Default::default()
    };
    assert_eq!(
        select_tracks(&audio, &subtitles, current_id, current_lang, &prefs).0,
        expected
    );
}

#[test]
fn parses_track_labels_and_excludes_bitmap_subtitles() {
    let parsed = parse_tracks(&[
        TrackInfo {
            kind: "audio".into(),
            id: 7,
            lang: "en".into(),
            title: "ignored".into(),
            codec: "aac".into(),
            selected: true,
            channels: 2,
            forced: false,
            stream_index: -1,
        },
        TrackInfo {
            kind: "sub".into(),
            id: 8,
            lang: "fr".into(),
            title: "Signs".into(),
            codec: "subrip".into(),
            selected: true,
            channels: 0,
            forced: true,
            stream_index: 12,
        },
        TrackInfo {
            kind: "sub".into(),
            id: 9,
            lang: "en".into(),
            title: String::new(),
            codec: "hdmv_pgs_subtitle".into(),
            selected: false,
            channels: 0,
            forced: false,
            stream_index: 13,
        },
    ]);
    assert_eq!(parsed.audio_tracks, [(7, "English AAC Stereo".into())]);
    assert_eq!(parsed.sub_tracks, [(8, "Signs (Forced)".into(), true)]);
    assert_eq!(parsed.sub_track_stream_indexes, [(8, 12)]);
    assert_eq!((parsed.audio_id, parsed.audio_lang.as_str()), (7, "en"));
    assert_eq!((parsed.sub_id, parsed.sub_lang.as_str()), (8, "fr"));
}

#[rstest]
#[case::minimum(0, 130, (0, 0))]
#[case::sqrt_curve(25, 130, (25, 50))]
#[case::clamps_maximum(200, 130, (130, 114))]
fn volume_decision_clamps_and_maps_sqrt_curve(
    #[case] requested: i64,
    #[case] maximum: i64,
    #[case] expected: (i64, i64),
) {
    assert_eq!(volume_decision(requested, maximum), expected);
}

#[rstest]
#[case::threshold(NextUp::Idle, 7_000_000_000, 6_500_000_000)]
#[case::armed(NextUp::Armed, 7_000_000_000, 6_500_000_000)]
fn queue_next_up_fires_at_threshold(
    #[case] state: NextUp,
    #[case] runtime: i64,
    #[case] ticks: i64,
) {
    assert_eq!(
        queue_next_up_decision(state, 1, 3, true, true, runtime, ticks).fire,
        Some(NextUpFire::Queue(2)),
    );
}

#[rstest]
#[case::armed(NextUp::Armed, true, 7_000_000_000, 6_500_000_000, crate::player::NextUpDecision { fire: Some(NextUpFire::Standalone), ..Default::default() })]
#[case::no_series_arms(NextUp::Idle, false, 7_000_000_000, 1, crate::player::NextUpDecision { arm: true, ..Default::default() })]
#[case::no_series_already_armed(NextUp::Armed, false, 7_000_000_000, 1, Default::default())]
#[case::short_runtime_does_not_fire(NextUp::Armed, true, 60_000_000, 1, Default::default())]
#[case::fired_does_not_fire(NextUp::Fired, true, 7_000_000_000, 6_500_000_000, Default::default())]
fn standalone_next_up_obeys_state_and_series_gates(
    #[case] state: NextUp,
    #[case] has_series: bool,
    #[case] runtime: i64,
    #[case] ticks: i64,
    #[case] expected: crate::player::NextUpDecision,
) {
    assert_eq!(
        standalone_next_up_decision(state, has_series, runtime, ticks),
        expected
    );
}

#[rstest]
#[case::no_next_episode(
    NextUp::Idle,
    0,
    1,
    true,
    false,
    7_000_000_000,
    6_500_000_000,
    Default::default()
)]
#[case::next_item_not_episode(
    NextUp::Idle,
    0,
    2,
    true,
    false,
    7_000_000_000,
    6_500_000_000,
    Default::default()
)]
#[case::runtime_too_short(
    NextUp::Idle,
    0,
    2,
    true,
    true,
    5_999_999_999,
    5_500_000_000,
    Default::default()
)]
#[case::insufficient_remaining(
    NextUp::Idle,
    0,
    2,
    true,
    true,
    7_000_000_000,
    6_900_000_000,
    Default::default()
)]
#[case::resets_fired_below_window(NextUp::Fired, 0, 2, true, true, 7_000_000_000, 6_000_000_000, crate::player::NextUpDecision { reset: true, arm: false, fire: None })]
#[case::arms_near_start(NextUp::Idle, 0, 2, true, true, 7_000_000_000, 1, crate::player::NextUpDecision { arm: true, ..Default::default() })]
fn queue_next_up_preserves_guards_and_reset(
    #[case] state: NextUp,
    #[case] current_idx: usize,
    #[case] queue_len: usize,
    #[case] current_is_episode: bool,
    #[case] next_is_episode: bool,
    #[case] runtime: i64,
    #[case] ticks: i64,
    #[case] expected: crate::player::NextUpDecision,
) {
    assert_eq!(
        queue_next_up_decision(
            state,
            current_idx,
            queue_len,
            current_is_episode,
            next_is_episode,
            runtime,
            ticks,
        ),
        expected
    );
}

#[test]
fn queue_advance_decision_keeps_audio_unplayed_but_consumable() {
    assert_eq!(
        advance_decision(true, false, false, true, 42),
        (true, false, true, 0)
    );
    assert_eq!(
        advance_decision(false, false, false, false, 42),
        (false, false, false, 42)
    );
}

#[test]
fn command_decisions_keep_seek_modes_and_reject_stale_slots() {
    let slot = crate::player::tests::owner_slot_id();
    let missing = crate::player::tests::owner_slot_id();
    assert_eq!(seek_decision(12.5, false), ("relative", "12.5".into()));
    assert_eq!(seek_decision(12.5, true), ("absolute", "12.5".into()));
    assert_eq!(resolve_jump_target(&[slot], slot), Some(0));
    assert_eq!(resolve_jump_target(&[slot], missing), None);
}

#[test]
fn active_item_state_resolves_episode_progress_and_identity() {
    let mut item = super::make_media_item("episode");
    item.playback_position_ticks = 75;
    let state = active_item_state(Some(&crate::playback_queue::QueueItem::Emby(Box::new(
        item,
    ))));
    assert_eq!(state.osd_title, "Show Test Episode");
    assert_eq!(state.last_valid_pos, 75);
    assert_eq!(state.series_id.as_str(), "series1");
    assert_eq!((state.season, state.episode), (1, 2));
}

#[rstest]
#[case::resumes_at_threshold(180_000_000, 180_000_000)]
#[case::resets_below_threshold(179_999_999, 0)]
fn active_item_state_gates_feed_resume_position(#[case] position: i64, #[case] expected: i64) {
    let mut entry = super::make_feed_entry("feed", "Podcast");
    entry.position_ticks = position;
    let state = active_item_state(Some(&crate::playback_queue::QueueItem::Feed(entry)));
    assert_eq!(state.last_valid_pos, expected);
}

#[test]
fn active_item_state_resolves_audiobookshelf_episode_and_book() {
    let episode = active_item_state(Some(&super::abs_item()));
    assert_eq!(episode.osd_title, "Episode");
    assert_eq!(episode.last_valid_pos, 0);
    assert_eq!(episode.series_id.as_str(), "");

    let book = active_item_state(Some(&super::abs_book_item()));
    assert_eq!(book.osd_title, "Book");
    assert_eq!(book.last_valid_pos, 0);
    assert_eq!(book.series_id.as_str(), "");
}

#[rstest]
#[case::audio("AudioTrack", "Audio", 0)]
#[case::non_episode("Movie", "Video", 75)]
fn active_item_state_clears_episode_identity_for_non_episodes(
    #[case] item_type: &str,
    #[case] media_type: &str,
    #[case] expected_position: i64,
) {
    let mut item = super::make_media_item("media");
    item.item_type = item_type.into();
    item.media_type = media_type.into();
    item.playback_position_ticks = 75;
    let state = active_item_state(Some(&crate::playback_queue::QueueItem::Emby(Box::new(
        item,
    ))));
    assert_eq!(state.last_valid_pos, expected_position);
    assert_eq!(state.series_id.as_str(), "");
    assert_eq!((state.season, state.episode), (0, 0));
}

#[test]
fn active_item_state_clears_when_queue_has_no_active_item() {
    let state = active_item_state(None);
    assert_eq!(state.osd_title, "");
    assert_eq!(state.last_valid_pos, 0);
    assert_eq!(state.series_id.as_str(), "");
    assert_eq!((state.season, state.episode), (0, 0));
}
