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

#[test]
fn audio_preference_selects_matching_track_only_when_needed() {
    let (audio, subtitles) = tracks();
    let prefs = SubtitlePrefs {
        audio_lang: "French".into(),
        ..Default::default()
    };
    assert_eq!(
        select_tracks(&audio, &subtitles, 1, "en", &prefs).0,
        Some(2)
    );
    assert_eq!(select_tracks(&audio, &subtitles, 2, "fr", &prefs).0, None);
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

#[test]
fn standalone_next_up_fires_at_threshold() {
    assert_eq!(
        standalone_next_up_decision(NextUp::Armed, true, 7_000_000_000, 6_500_000_000).fire,
        Some(NextUpFire::Standalone),
    );
}

#[test]
fn next_up_requires_consecutive_tv_episodes_and_preserves_minimum_runtime() {
    assert_eq!(
        queue_next_up_decision(NextUp::Idle, 0, 2, true, false, 1_000_000_000, 950_000_000),
        Default::default()
    );
    assert_eq!(
        queue_next_up_decision(NextUp::Idle, 0, 2, true, true, 5_999_999_999, 5_500_000_000),
        Default::default()
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
fn active_item_state_clears_when_queue_has_no_active_item() {
    let state = active_item_state(None);
    assert_eq!(state.osd_title, "");
    assert_eq!(state.last_valid_pos, 0);
    assert_eq!(state.series_id.as_str(), "");
    assert_eq!((state.season, state.episode), (0, 0));
}
