const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum FeedAgeGroup {
    New,
    Recent,
    OlderThanTwoWeeks,
    OlderThanMonth,
    Unknown,
}

impl FeedAgeGroup {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::Recent => "Recent",
            Self::OlderThanTwoWeeks => "Older than two weeks",
            Self::OlderThanMonth => "Older than a month",
            Self::Unknown => "Unknown date",
        }
    }
}

pub(in crate::app) fn feed_age_group(pub_date_secs: Option<u64>, now_secs: u64) -> FeedAgeGroup {
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
