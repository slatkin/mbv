//! Home flat projection contract (`openspec/specs/home-latest-sections`:
//! "Recently added is grouped by age with the Feeds rule").

use super::*;
use mbv_emby_model::test_support::make_item;
use mbv_render::components::media_list::MediaListRow;

const DAY: u64 = 24 * 60 * 60;
const NOW: u64 = 1_500_000_000;

fn iso_secs(secs: u64) -> String {
    use time::format_description::well_known::Iso8601;
    time::OffsetDateTime::from_unix_timestamp(i64::try_from(secs).unwrap_or(i64::MAX))
        .expect("in-range fixture timestamp")
        .format(&Iso8601::DEFAULT)
        .expect("ISO-8601 formatting always succeeds")
}

fn recent_row(name: &str, days_old: u64) -> HomeRecentRow {
    let mut item = make_item(name, "Movie");
    item.id = name.to_string();
    item.date_added = iso_secs(NOW - days_old * DAY);
    HomeRecentRow {
        item: QueueItem::Emby(Box::new(item)),
        feed_name: None,
    }
}

/// Heading text or item primary text, one string per row, `-` for a spacer.
fn labels(owner: &HomeContent) -> Vec<String> {
    owner
        .carrier
        .rows()
        .iter()
        .map(|row| match row {
            MediaListRow::Heading { text } => format!("# {text}"),
            MediaListRow::Item { primary, .. } => primary.clone(),
            MediaListRow::Spacer => "-".into(),
        })
        .collect()
}

#[test]
fn recent_rows_group_by_feeds_age_and_omit_empty_headers() {
    let mut owner = HomeContent::new();
    owner.set_recent_rows(
        vec![
            recent_row("fresh", 1),
            recent_row("week", 13),
            recent_row("stale", 14),
        ],
        NOW,
    );
    assert_eq!(
        labels(&owner),
        ["# Recently added", "# New", "fresh", "# Recent", "week"]
    );

    owner.set_recent_rows(vec![recent_row("stale", 14)], NOW);
    assert!(labels(&owner).is_empty());
}
