//! The GUI Hero's facts (gui-hero-design-polish design D1): the GUI's own
//! split of each Hero's values into short header pills and labelled detail
//! rows, built from the source items. One builder per source kind, each
//! formatting through the shared `mbv_ui_model::ui_util` formatters and the
//! same `MetaSegment` kinds the shared TUI producers use for the same value,
//! so the text and the type colour do not change. A fact with no value is
//! left out. Facts do not decide the Hero title.

use mbv_client_core::App;
use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_queue::{
    AudiobookshelfBookQueueItem, AudiobookshelfItem, AudiobookshelfQueueItem, FeedEntry, QueueItem,
};
use mbv_ui_model::library_hero::{seasons_label, series_years_range};
use mbv_ui_model::meta::{MetaKind, MetaSegment};
use mbv_ui_model::ui_util::{fmt_duration_hms, fmt_publish_date, format_release_date};

use super::GuiLibraryArtwork;

/// The Hero's facts for one item (design D1): the header band's pills and the
/// detail block's labelled rows, both in paint order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GuiHeroFacts {
    /// One short fact per pill, in paint order (the header band's facts
    /// line).
    pub pills: Vec<GuiHeroPill>,
    /// One labelled fact per row, in paint order (the detail block).
    pub rows: Vec<GuiHeroFactRow>,
}

/// One short Hero fact (design D1): its typed segments, so the pill's text
/// and type colour stay as today, and the optional URL a link pill opens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuiHeroPill {
    /// The pill's typed runs of text.
    pub segments: Vec<MetaSegment>,
    /// The URL a link pill opens (`None` on a plain fact pill).
    pub link: Option<String>,
}

/// One labelled Hero fact row (design D1): the window paints the label word
/// from the label enum, and the value through its typed segments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuiHeroFactRow {
    /// Which label word the row paints.
    pub label: GuiHeroFactLabel,
    /// The row's typed runs of text.
    pub value: Vec<MetaSegment>,
}

/// The label word of one Hero fact row (design D1). The window paints the
/// word from the enum, so the builders stay free of label text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiHeroFactLabel {
    /// The movie's directors, joined with `, `.
    Director,
    /// The item's release date.
    Released,
    /// The movie's studios, joined with `, `.
    Studio,
    /// The series' network: its first studio.
    Network,
    /// The audiobook's author.
    Author,
    /// The podcast episode's or the feed entry's publish date.
    Published,
}

/// The Emby item's facts (design D1): the movie arm's year, genre, runtime,
/// and IMDb-link pills over the Director, Released, and Studio rows, or the
/// other-item arm's (an episode, a video) year and runtime pills over the
/// Released row.
#[must_use]
pub fn hero_facts_emby(item: &EmbyItem) -> GuiHeroFacts {
    let movie = item.item_type == "Movie";
    let mut facts = GuiHeroFacts::default();
    push_year_pill(&mut facts, item.production_year);
    if movie {
        push_genre_pills(&mut facts, &item.genres);
    }
    if item.runtime_ticks > 0 {
        facts
            .pills
            .push(duration_pill(item.runtime_ticks / TICKS_PER_SECOND));
    }
    if movie && let Some(link) = item.external_urls.iter().find(|link| link.name == "IMDb") {
        facts.pills.push(GuiHeroPill {
            segments: vec![MetaSegment::plain(link.name.clone())],
            link: Some(link.url.clone()),
        });
    }
    if movie {
        let directors = item
            .people
            .iter()
            .filter(|person| person.kind == "Director")
            .map(|person| person.name.as_str())
            .collect::<Vec<_>>();
        if !directors.is_empty() {
            facts.rows.push(fact_row(
                GuiHeroFactLabel::Director,
                MetaSegment::plain(directors.join(", ")),
            ));
        }
    }
    if !item.premiere_date.is_empty() {
        facts.rows.push(fact_row(
            GuiHeroFactLabel::Released,
            MetaSegment::new(format_release_date(&item.premiere_date), MetaKind::Date),
        ));
    }
    if movie {
        let studios = item
            .studios
            .iter()
            .map(String::as_str)
            .filter(|studio| !studio.is_empty())
            .collect::<Vec<_>>();
        if !studios.is_empty() {
            facts.rows.push(fact_row(
                GuiHeroFactLabel::Studio,
                MetaSegment::plain(studios.join(", ")),
            ));
        }
    }
    facts
}

/// The Shows series' facts (design D1): the year range (else the formatted
/// premiere date, as the pre-polish Hero showed, else nothing), each genre, and
/// the season count as pills, over the Network row.
#[must_use]
pub fn hero_facts_series(series: &EmbyItem, seasons: Option<usize>) -> GuiHeroFacts {
    let mut facts = GuiHeroFacts::default();
    let years = series_years_range(series).or_else(|| {
        (!series.premiere_date.is_empty()).then(|| format_release_date(&series.premiere_date))
    });
    if let Some(range) = years {
        facts.pills.push(GuiHeroPill {
            segments: vec![MetaSegment::new(range, MetaKind::Date)],
            link: None,
        });
    }
    push_genre_pills(&mut facts, &series.genres);
    if let Some(seasons) = seasons.filter(|count| *count > 0) {
        facts.pills.push(GuiHeroPill {
            segments: vec![MetaSegment::new(seasons_label(seasons), MetaKind::Count)],
            link: None,
        });
    }
    if let Some(network) = series.studios.first().filter(|studio| !studio.is_empty()) {
        facts.rows.push(fact_row(
            GuiHeroFactLabel::Network,
            MetaSegment::plain(network.clone()),
        ));
    }
    facts
}

/// The Audiobookshelf book's facts (design D1): the duration pill over the
/// Author row.
#[must_use]
pub fn hero_facts_abs_book(book: &AudiobookshelfBookQueueItem) -> GuiHeroFacts {
    let mut facts = GuiHeroFacts::default();
    if let Some(seconds) = duration_seconds(book.duration_ticks) {
        facts.pills.push(duration_pill(seconds));
    }
    if let Some(author) = book.author.as_deref().filter(|author| !author.is_empty()) {
        facts.rows.push(fact_row(
            GuiHeroFactLabel::Author,
            MetaSegment::plain(author),
        ));
    }
    facts
}

/// The Audiobookshelf podcast episode's facts (design D1): the duration pill
/// over the Published row.
#[must_use]
pub fn hero_facts_abs_episode(episode: &AudiobookshelfQueueItem) -> GuiHeroFacts {
    duration_and_published(episode.duration_ticks, episode.pub_date_secs)
}

/// The feed entry's facts (design D1): the duration pill over the Published
/// row (when the entry has a date).
#[must_use]
pub fn hero_facts_feed(entry: &FeedEntry) -> GuiHeroFacts {
    duration_and_published(entry.duration_ticks, entry.pub_date_secs)
}

/// The duration pill over the Published row, each present only when the source
/// carries it: the shape the episode and feed-entry Heroes share.
fn duration_and_published(duration_ticks: Option<u64>, pub_date_secs: Option<u64>) -> GuiHeroFacts {
    let mut facts = GuiHeroFacts::default();
    if let Some(seconds) = duration_seconds(duration_ticks) {
        facts.pills.push(duration_pill(seconds));
    }
    if let Some(secs) = pub_date_secs {
        facts.rows.push(published_row(secs));
    }
    facts
}

/// The queued item's facts (design D1): dispatches to the item kind's
/// builder, so a queue-backed Hero (Home, Shows New) and its source item's
/// own Hero share one set of facts. A Twitch item carries none.
#[must_use]
pub fn hero_facts_queue(item: &QueueItem) -> GuiHeroFacts {
    match item {
        QueueItem::Emby(item) => hero_facts_emby(item),
        QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(episode)) => {
            hero_facts_abs_episode(episode)
        }
        QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book)) => hero_facts_abs_book(book),
        QueueItem::Feed(entry) => hero_facts_feed(entry),
        QueueItem::Twitch(_) => GuiHeroFacts::default(),
    }
}

/// One member of a movie's cast section (design D3): the person's name and
/// role, the initials the photo's round box falls back to, and the photo the
/// Service serves when the person carries an id and a tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiHeroCastMember {
    /// The person's name.
    pub name: String,
    /// The role painted beside the name: the person's role, else their kind.
    pub role: String,
    /// The initials the photo box paints when no photo shows: the first
    /// letter of the name's first two words, upper case.
    pub initials: String,
    /// The person's Primary photo, read from the image cache. `None` with no
    /// id or no tag; `Some` holds the fetch's loading state while it runs.
    pub photo: Option<GuiLibraryArtwork>,
}

/// The movie's cast (design D3): the people that are not directors, in the
/// order Emby gives, each with its role, initials, and photo. A non-movie
/// carries no cast, as the shared producer's credits table does. A person
/// with an id and a tag starts its photo fetch, so the section paints the
/// loading state until the bytes land.
#[must_use]
pub fn hero_cast_emby(app: &mut App, item: &EmbyItem) -> Vec<GuiHeroCastMember> {
    if item.item_type != "Movie" {
        return Vec::new();
    }
    item.people
        .iter()
        .filter(|person| person.kind != "Director")
        .map(|person| GuiHeroCastMember {
            role: if person.role.is_empty() {
                person.kind.clone()
            } else {
                person.role.clone()
            },
            name: person.name.clone(),
            initials: cast_initials(&person.name),
            photo: super::artwork::emby_person_photo(app, &person.id, &person.primary_image_tag),
        })
        .collect()
}

/// The initials of one cast member's name (design D3): the first letter of
/// the name's first two words, upper case.
fn cast_initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

/// The duration seconds behind a u64 tick source (design D1): `None` when the
/// source carries no duration or a zero one. The division and the saturating
/// conversion mirror the shared producers' row text exactly.
fn duration_seconds(ticks: Option<u64>) -> Option<i64> {
    let ticks = ticks.filter(|ticks| *ticks > 0)?;
    Some(i64::try_from(ticks / TICKS_PER_SECOND as u64).unwrap_or(i64::MAX))
}

/// The year pill (design D1): the production year as a `Date` pill, omitted
/// when the payload declares none.
fn push_year_pill(facts: &mut GuiHeroFacts, year: u32) {
    if year > 0 {
        facts.pills.push(GuiHeroPill {
            segments: vec![MetaSegment::new(year.to_string(), MetaKind::Date)],
            link: None,
        });
    }
}

/// One pill per non-empty genre (design D1), each a `Plain` pill.
fn push_genre_pills(facts: &mut GuiHeroFacts, genres: &[String]) {
    for genre in genres.iter().filter(|genre| !genre.is_empty()) {
        facts.pills.push(GuiHeroPill {
            segments: vec![MetaSegment::plain(genre.clone())],
            link: None,
        });
    }
}

/// The runtime pill (design D1): the shared duration format as a `Time`
/// pill.
fn duration_pill(seconds: i64) -> GuiHeroPill {
    GuiHeroPill {
        segments: vec![MetaSegment::new(fmt_duration_hms(seconds), MetaKind::Time)],
        link: None,
    }
}

/// The Published row (design D1): the shared publish-date format as a
/// `Date` value.
fn published_row(secs: u64) -> GuiHeroFactRow {
    fact_row(
        GuiHeroFactLabel::Published,
        MetaSegment::new(fmt_publish_date(secs), MetaKind::Date),
    )
}

/// One single-segment fact row (design D1): every builder's value is one
/// typed segment.
fn fact_row(label: GuiHeroFactLabel, value: MetaSegment) -> GuiHeroFactRow {
    GuiHeroFactRow {
        label,
        value: vec![value],
    }
}

#[cfg(test)]
mod tests;
