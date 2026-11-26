//! Tests of the GUI Hero facts builders (gui-hero-design-polish design D1,
//! D8): one `#[case]` table over the source kinds of D1's table, each case
//! asserting the pills in order and the rows in order, with each pill's type
//! kind and link and each row's label. Every fixture is in memory; no
//! service, no image cache, no live anything.

use mbv_emby_model::test_support::make_item;
use mbv_emby_model::{EmbyItem, EmbyLink, EmbyPerson, TICKS_PER_SECOND};
use mbv_queue::{
    AudiobookshelfBookQueueItem, AudiobookshelfQueueItem, FeedEntry, QueueItem, TwitchItem,
};
use mbv_ui_model::meta::{MetaKind, meta_row_text};
use rstest::rstest;

use super::{
    GuiHeroFactLabel, GuiHeroFacts, GuiHeroPill, hero_facts_abs_book, hero_facts_abs_episode,
    hero_facts_emby, hero_facts_feed, hero_facts_queue, hero_facts_series,
};

/// One source kind of design D1's table.
#[derive(Clone, Copy, Debug)]
enum SourceKind {
    Movie,
    OtherEmbyItem,
    Series,
    AbsBook,
    AbsEpisode,
    FeedEntry,
    Twitch,
}

/// One expected pill: the text, the type kind, and the link target.
type ExpectedPill = (&'static str, MetaKind, Option<&'static str>);

/// One expected row: the label, the text, and the type kind.
type ExpectedRow = (GuiHeroFactLabel, &'static str, MetaKind);

/// The facts of `kind`, from the fixture the kind's builder takes. The Twitch
/// kind reaches its builder through `hero_facts_queue`'s dispatch, the only
/// builder a Twitch item has (design D1: no pills, no rows).
fn facts_for(kind: SourceKind) -> GuiHeroFacts {
    match kind {
        SourceKind::Movie => hero_facts_emby(&movie()),
        SourceKind::OtherEmbyItem => hero_facts_emby(&other_emby_item()),
        SourceKind::Series => hero_facts_series(&series(), Some(5)),
        SourceKind::AbsBook => hero_facts_abs_book(&abs_book()),
        SourceKind::AbsEpisode => hero_facts_abs_episode(&abs_episode()),
        SourceKind::FeedEntry => hero_facts_feed(&feed_entry()),
        SourceKind::Twitch => hero_facts_queue(&QueueItem::Twitch(twitch_item())),
    }
}

/// A movie with a year, two genres, a runtime, an `IMDb` link, two directors,
/// a release date, and a studio — the given of the spec scenario
/// "A movie's Hero".
fn movie() -> EmbyItem {
    let mut item = make_item("A Movie", "Movie");
    item.production_year = 2015;
    item.genres = vec!["Science Fiction".into(), "Thriller".into()];
    item.runtime_ticks = 5_852 * TICKS_PER_SECOND;
    item.external_urls = vec![EmbyLink {
        name: "IMDb".into(),
        url: "https://www.imdb.com/title/tt0000000".into(),
    }];
    item.people = vec![
        EmbyPerson {
            name: "Lana Wachowski".into(),
            role: String::new(),
            kind: "Director".into(),
            id: String::new(),
            primary_image_tag: String::new(),
        },
        EmbyPerson {
            name: "Lilly Wachowski".into(),
            role: String::new(),
            kind: "Director".into(),
            id: String::new(),
            primary_image_tag: String::new(),
        },
        EmbyPerson {
            name: "Keanu Reeves".into(),
            role: "Neo".into(),
            kind: "Actor".into(),
            id: String::new(),
            primary_image_tag: String::new(),
        },
    ];
    item.premiere_date = "2015-06-19T00:00:00.0000000Z".into();
    item.studios = vec!["Warner Bros. Pictures".into(), "Village Roadshow".into()];
    item
}

/// An episode with a year, a genre, a runtime, and a release date — the
/// other-item arm, which drops the genre pill and the Director and Studio
/// rows.
fn other_emby_item() -> EmbyItem {
    let mut item = make_item("An Episode", "Episode");
    item.production_year = 2023;
    item.genres = vec!["Drama".into()];
    item.runtime_ticks = 2_700 * TICKS_PER_SECOND;
    item.premiere_date = "2023-03-05T00:00:00.0000000Z".into();
    item
}

/// An ended series with two genres, a network, and (in the table) five
/// seasons — the Shows builder.
fn series() -> EmbyItem {
    let mut item = make_item("A Series", "Series");
    item.production_year = 1982;
    item.end_year = 1993;
    item.genres = vec!["Drama".into(), "Mystery".into()];
    item.studios = vec!["NBC".into()];
    item
}

/// An audiobook with an author and a duration (the Books builder).
fn abs_book() -> AudiobookshelfBookQueueItem {
    AudiobookshelfBookQueueItem {
        library_item_id: "lib-1".into(),
        title: "A Book".into(),
        author: Some("An Author".into()),
        duration_ticks: Some(4_500 * TICKS_PER_SECOND as u64),
        position_ticks: 0,
        played: false,
        is_finished: false,
        cover_path: None,
    }
}

/// A podcast episode with a duration and a publish date (the Audiobookshelf
/// episode builder).
fn abs_episode() -> AudiobookshelfQueueItem {
    AudiobookshelfQueueItem {
        library_item_id: "lib-1".into(),
        episode_id: "ep-1".into(),
        title: "An Episode".into(),
        show_title: Some("A Show".into()),
        author: None,
        description: None,
        duration_ticks: Some(1_800 * TICKS_PER_SECOND as u64),
        position_ticks: 0,
        played: false,
        pub_date_secs: Some(1_752_000_000),
        is_finished: false,
        cover_path: None,
    }
}

/// A feed entry with a duration and a publish date (the Feeds builder).
fn feed_entry() -> FeedEntry {
    FeedEntry {
        guid: "f1".into(),
        title: "A Feed Entry".into(),
        enclosure_url: Some("https://example.invalid/e.mp3".into()),
        link: None,
        mime_type: None,
        duration_ticks: Some(600 * TICKS_PER_SECOND as u64),
        pub_date_secs: Some(1_740_000_000),
        feed_kind: None,
        feed_id: None,
        position_ticks: 0,
        played: false,
    }
}

/// A Twitch item, whose facts are none (design D1).
fn twitch_item() -> TwitchItem {
    TwitchItem {
        url: "https://www.twitch.tv/a".into(),
        title: "A Stream".into(),
    }
}

/// The pills of `facts` as `(text, type kind, link)`, in paint order.
fn actual_pills(facts: &GuiHeroFacts) -> Vec<(String, Option<MetaKind>, Option<String>)> {
    let pill_text = |pill: &GuiHeroPill| {
        pill.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<String>()
    };
    facts
        .pills
        .iter()
        .map(|pill| {
            (
                pill_text(pill),
                pill.segments.first().map(|segment| segment.kind),
                pill.link.clone(),
            )
        })
        .collect()
}

/// The rows of `facts` as `(label, text, type kind)`, in paint order.
fn actual_rows(facts: &GuiHeroFacts) -> Vec<(GuiHeroFactLabel, String, Option<MetaKind>)> {
    facts
        .rows
        .iter()
        .map(|row| {
            (
                row.label,
                meta_row_text(&row.value),
                row.value.first().map(|segment| segment.kind),
            )
        })
        .collect()
}

/// The source kinds of design D1's table give the pills and the rows of
/// their table row, in order. Each row list is exact, so a fact the table
/// leaves out fails the case: the movie's pills carry the runtime and the
/// `IMDb` link but no runtime row exists, and the other-item arm drops the
/// genre pill and the Director and Studio rows.
///
/// The movie case is the contract of the spec scenario "A movie's Hero": the
/// year pill and the Released row are both present, no runtime row exists,
/// and the directors sit joined in a Director row.
#[rstest]
#[case::movie(
    SourceKind::Movie,
    &[
        ("2015", MetaKind::Date, None),
        ("Science Fiction", MetaKind::Plain, None),
        ("Thriller", MetaKind::Plain, None),
        ("01:37:32", MetaKind::Time, None),
        ("IMDb", MetaKind::Plain, Some("https://www.imdb.com/title/tt0000000")),
    ],
    &[
        (GuiHeroFactLabel::Director, "Lana Wachowski, Lilly Wachowski", MetaKind::Plain),
        (GuiHeroFactLabel::Released, "19 Jun 2015", MetaKind::Date),
        (GuiHeroFactLabel::Studio, "Warner Bros. Pictures, Village Roadshow", MetaKind::Plain),
    ],
)]
#[case::other_emby_item(
    SourceKind::OtherEmbyItem,
    &[
        ("2023", MetaKind::Date, None),
        ("45:00", MetaKind::Time, None),
    ],
    &[(GuiHeroFactLabel::Released, "5 Mar 2023", MetaKind::Date)],
)]
#[case::series(
    SourceKind::Series,
    &[
        ("1982 - 1993", MetaKind::Date, None),
        ("Drama", MetaKind::Plain, None),
        ("Mystery", MetaKind::Plain, None),
        ("5 Seasons", MetaKind::Count, None),
    ],
    &[(GuiHeroFactLabel::Network, "NBC", MetaKind::Plain)],
)]
#[case::abs_book(
    SourceKind::AbsBook,
    &[("01:15:00", MetaKind::Time, None)],
    &[(GuiHeroFactLabel::Author, "An Author", MetaKind::Plain)],
)]
#[case::abs_episode(
    SourceKind::AbsEpisode,
    &[("30:00", MetaKind::Time, None)],
    &[(GuiHeroFactLabel::Published, "8 Jul 2025", MetaKind::Date)],
)]
#[case::feed_entry(
    SourceKind::FeedEntry,
    &[("10:00", MetaKind::Time, None)],
    &[(GuiHeroFactLabel::Published, "19 Feb 2025", MetaKind::Date)],
)]
#[case::twitch(SourceKind::Twitch, &[], &[])]
fn the_facts_of_each_source_kind_follow_the_design_table(
    #[case] kind: SourceKind,
    #[case] pills: &[ExpectedPill],
    #[case] rows: &[ExpectedRow],
) {
    let facts = facts_for(kind);
    let expected_pills = pills
        .iter()
        .map(|(text, kind, link)| ((*text).to_owned(), Some(*kind), link.map(ToOwned::to_owned)))
        .collect::<Vec<_>>();
    let expected_rows = rows
        .iter()
        .map(|(label, text, kind)| (*label, (*text).to_owned(), Some(*kind)))
        .collect::<Vec<_>>();
    assert_eq!(
        actual_pills(&facts),
        expected_pills,
        "{kind:?}: the pills of design D1's table, in order"
    );
    assert_eq!(
        actual_rows(&facts),
        expected_rows,
        "{kind:?}: the rows of design D1's table, in order"
    );
}
