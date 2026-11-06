//! The status-indicator cluster's mapping (`status_indicator_items`): the
//! resolution, audio, and subtitle facts the queue band paints as plain
//! text, uppercased, each in the role its state resolves.

use mbv_render::components::indicators::{
    IndicatorData, IndicatorFlags, StatusIndicator, status_indicator_items,
};
use mbv_theme::Role;
use rstest::rstest;

fn data(
    res_label: &str,
    audio_label: &str,
    sub_label: &str,
    flags: IndicatorFlags,
) -> IndicatorData {
    IndicatorData {
        res_label: res_label.to_string(),
        audio_label: audio_label.to_string(),
        sub_label: sub_label.to_string(),
        flags,
    }
}

fn labels(items: &[StatusIndicator]) -> Vec<(&str, Role)> {
    items
        .iter()
        .map(|item| (item.label.as_str(), item.role))
        .collect()
}

/// The cluster's mapping: the resolved facts uppercased, dimmed facts in the
/// muted role, the live subtitle label in the metadata role, and audio-only
/// playback collapsed to the resolution alone.
#[rstest]
#[case::video_audio_subtitles(
    data(
        "FHD",
        "en",
        "eng",
        IndicatorFlags {
            res_dim: false,
            audio_dim: false,
            audio_only: false,
            sub_on: true,
        },
    ),
    &[
        ("FHD", Role::IndicatorResolutionFg),
        ("EN", Role::IndicatorAudioFg),
        ("ENG", Role::TextMetadata),
    ],
)]
#[case::dimmed_and_off(
    data(
        "--",
        "x",
        "CC",
        IndicatorFlags {
            res_dim: true,
            audio_dim: true,
            audio_only: false,
            sub_on: false,
        },
    ),
    &[
        ("--", Role::TextMuted),
        ("X", Role::TextMuted),
        ("CC", Role::TextMuted),
    ],
)]
#[case::audio_only(
    data(
        "FLAC",
        "en",
        "CC",
        IndicatorFlags {
            res_dim: false,
            audio_dim: false,
            audio_only: true,
            sub_on: true,
        },
    ),
    &[("FLAC", Role::IndicatorResolutionFg)],
)]
fn the_cluster_maps_the_resolved_facts(
    #[case] source: IndicatorData,
    #[case] expected: &[(&str, Role)],
) {
    assert_eq!(labels(&status_indicator_items(&source)), expected.to_vec());
}
