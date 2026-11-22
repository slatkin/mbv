use super::*;
use crate::meta::meta_rows_text;
use mbv_emby_model::test_support::make_item;
use rstest::rstest;

const MINUTE: i64 = 60 * TICKS_PER_SECOND;

fn series() -> EmbyItem {
    let mut item = make_item("Ann Droid", "Series");
    item.production_year = 2026;
    item.premiere_date = "2026-07-16".into();
    item.runtime_ticks = 28 * MINUTE;
    item.genres = vec!["Science Fiction".into()];
    item
}

fn episode(premiere_date: &str, runtime_minutes: i64) -> EmbyItem {
    let mut item = make_item("Episode 1", "Episode");
    item.premiere_date = premiere_date.into();
    item.runtime_ticks = runtime_minutes * MINUTE;
    item
}

/// The focused Workspace swaps the episode's overview and Primary still
/// into the hero; unfocused keeps the show-level ones.
#[rstest]
#[case::focused(true, "Episode overview")]
#[case::unfocused(false, "Series overview")]
fn workspace_focus_swaps_episode_overview_and_artwork(
    #[case] workspace_focused: bool,
    #[case] expected_overview: &str,
) {
    let mut series = series();
    series.overview = "Series overview".into();
    series.image_tags.primary = "series-poster".into();
    let mut episode = episode("2026-07-20", 25);
    episode.overview = "Episode overview".into();
    let data =
        hero_content_series_with_episode(&series, Some(&episode), workspace_focused, Some(11));
    assert_eq!(data.overview.as_deref(), Some(expected_overview));
    let source = data.facts.artwork.source.expect("artwork source");
    let ArtworkSource::Emby {
        item_id,
        image_types,
        ..
    } = source
    else {
        panic!("emby artwork source");
    };
    if workspace_focused {
        assert_eq!(item_id, episode.id);
        assert_eq!(image_types, vec!["Primary".to_string()]);
        assert_eq!(data.facts.artwork.shape, ArtworkShape::Landscape);
    } else {
        assert_eq!(item_id, series.id);
    }
}

/// The hero meta rows follow the Workspace selection: focused shows the
/// selected episode's air date and duration; unfocused shows the show's
/// year range and no duration row. The show's season count and network
/// are show-level rows in both states. Per-row fallback when the episode
/// does not carry the value.
#[rstest]
#[case::focused_episode_overrides_both(
    true,
    Some(("2026-07-20", 25)),
    &["SCIENCE FICTION", "20 Jul 2026", "25:00", "11 Seasons", "NBC"],
)]
#[case::unfocused_shows_year_range_and_no_duration(
    false,
    Some(("2026-07-20", 25)),
    &["SCIENCE FICTION", "2026 - Present", "11 Seasons", "NBC"],
)]
#[case::focused_without_episode_keeps_series_rows(
    true,
    None,
    &["SCIENCE FICTION", "16 Jul 2026", "28:00", "11 Seasons", "NBC"],
)]
#[case::episode_without_date_keeps_series_date(
    true,
    Some(("", 25)),
    &["SCIENCE FICTION", "16 Jul 2026", "25:00", "11 Seasons", "NBC"],
)]
#[case::episode_without_runtime_keeps_series_duration(
    true,
    Some(("2026-07-20", 0)),
    &["SCIENCE FICTION", "20 Jul 2026", "28:00", "11 Seasons", "NBC"],
)]
fn workspace_selection_overlays_air_date_and_duration(
    #[case] workspace_focused: bool,
    #[case] selection: Option<(&str, i64)>,
    #[case] expected: &[&str],
) {
    let mut show = series();
    show.studios = vec!["NBC".into()];
    let episode = selection.map(|(date, minutes)| episode(date, minutes));
    let data =
        hero_content_series_with_episode(&show, episode.as_ref(), workspace_focused, Some(11));
    assert_eq!(meta_rows_text(&data.facts.meta_rows), expected);
    assert_eq!(
        data.facts.duration_row.is_some(),
        workspace_focused,
        "duration row presence follows workspace focus"
    );
}

/// The Series line types its year range as a Date segment; the genre
/// after it stays Plain (design D14).
#[test]
fn series_line_types_year_range_as_date() {
    let data = hero_content_emby(&series());
    assert_eq!(
        data.facts.meta_rows[0].first(),
        Some(&MetaSegment::new("2026", MetaKind::Date)),
    );
}
