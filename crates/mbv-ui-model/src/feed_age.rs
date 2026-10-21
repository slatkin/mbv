const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedAgeGroup {
    New,
    Recent,
    OlderThanTwoWeeks,
    OlderThanMonth,
    Unknown,
}

impl FeedAgeGroup {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::Recent => "Recent",
            Self::OlderThanTwoWeeks => "Older than two weeks",
            Self::OlderThanMonth => "Older than a month",
            Self::Unknown => "Unknown date",
        }
    }
}

#[must_use]
pub fn feed_age_group(pub_date_secs: Option<u64>, now_secs: u64) -> FeedAgeGroup {
    let Some(pub_date_secs) = pub_date_secs else {
        return FeedAgeGroup::Unknown;
    };

    match now_secs.saturating_sub(pub_date_secs) / SECONDS_PER_DAY {
        0..=1 => FeedAgeGroup::New,
        2..=13 => FeedAgeGroup::Recent,
        14..=29 => FeedAgeGroup::OlderThanTwoWeeks,
        _ => FeedAgeGroup::OlderThanMonth,
    }
}

/// One row of an age-grouped projection: a group heading, the spacer between
/// groups, or one item with its position in the input slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgeGroupedRow<'a, T> {
    Spacer,
    Heading(FeedAgeGroup),
    Item(usize, &'a T),
}

// At most one heading per feed age group plus one spacer between groups
// (5 groups today, so 9 extra rows max); adding a group without raising
// this bound only costs a reallocation, never correctness.
const MAX_GROUP_ROWS: usize = 9;

/// Group `items` under the age-group headings, in input order: one heading
/// when a row's age group differs from the previous row's, one spacer
/// between consecutive groups, and every item after its group's heading.
/// The Feeds tab's rows and the homevideos All list share this projection
/// (spec `feed-home-video-group-view`); `date_key` is each item's
/// epoch-seconds date (`None` groups under `Unknown`). The `row` closure
/// maps each row kind into the caller's row vocabulary.
#[must_use]
pub fn age_grouped_rows<T, R>(
    items: &[T],
    now_secs: u64,
    date_key: impl Fn(&T) -> Option<u64>,
    row: impl FnMut(AgeGroupedRow<'_, T>) -> R,
) -> Vec<R> {
    let mut rows = Vec::with_capacity(items.len() + MAX_GROUP_ROWS);
    let mut last_group = None;
    let mut map_row = row;
    for (idx, item) in items.iter().enumerate() {
        let group = feed_age_group(date_key(item), now_secs);
        if last_group != Some(group) {
            if last_group.is_some() {
                rows.push(map_row(AgeGroupedRow::Spacer));
            }
            rows.push(map_row(AgeGroupedRow::Heading(group)));
            last_group = Some(group);
        }
        rows.push(map_row(AgeGroupedRow::Item(idx, item)));
    }
    rows
}
