//! The Library Hero's facts and geometry (`add-gui-library-panel` design D5,
//! D8, the port of `lib/library/hero.ts`): the picked album's lines as the
//! canvas prints them, and the artist's albums in the union that Shuffle artist
//! plays. The data is the core's, so this module only reads the carried Hero;
//! keeping the lines here lets the painters stay markup.
//!
//! A line that joins values of several data types is built as typed segments
//! (`add-gui-settings-screen` design D14): colour carries the type, so a count
//! and a time on one line each carry their own kind, and the painter paints the
//! kinds through `TypedText`.

use crate::bridge::frame::library::{
    GuiLibraryDetail, GuiMetaKindFrame, GuiMetaSegmentFrame, GuiMusicHeroFrame,
    GuiMusicHeroSubjectFrame,
};
use crate::window::sections::SectionRef;

use super::lettered::LetteredDestination;
use super::state::LibraryState;

/// One meta row as the frames carry it: a line of typed segments.
pub type MetaRow = Vec<GuiMetaSegmentFrame>;

/// One line's typed runs, in display order (design D14).
pub type Segments = MetaRow;

/// A line with no typed part: it inherits the colour the painter gives it.
#[must_use]
pub fn plain_segments(text: &str) -> Segments {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![GuiMetaSegmentFrame {
            text: text.to_owned(),
            kind: GuiMetaKindFrame::Plain,
        }]
    }
}

/// A section label's line, which paints the label token (design D14).
#[must_use]
pub fn label_segments(text: &str) -> Segments {
    vec![GuiMetaSegmentFrame {
        text: text.to_owned(),
        kind: GuiMetaKindFrame::Label,
    }]
}

/// The segments of several meta rows on one line, joined by ` · ` as the
/// frames order them.
#[must_use]
pub fn join_meta_rows(rows: &[MetaRow]) -> Segments {
    let mut joined: Segments = Vec::new();
    for (index, row) in rows.iter().filter(|row| !row.is_empty()).enumerate() {
        if index > 0 {
            joined.push(GuiMetaSegmentFrame {
                text: " · ".to_owned(),
                kind: GuiMetaKindFrame::Plain,
            });
        }
        joined.extend(row.iter().cloned());
    }
    joined
}

/// The artist header's first line, `ARTIST · HIP-HOP` as the canvas prints it:
/// the artist label with the selected collection names (design D5).
#[must_use]
pub fn hero_kicker(hero: &GuiMusicHeroFrame) -> Segments {
    // The artist page carries its collections in the facts line, so its
    // kicker stays empty: no repeated collection name and no kind word.
    if matches!(hero.subject, GuiMusicHeroSubjectFrame::Artist(_)) {
        return Vec::new();
    }
    let line = if hero.collections.is_empty() {
        "artist".to_owned()
    } else {
        format!("artist · {}", hero.collections.join(" + ").to_uppercase())
    };
    label_segments(&line)
}

/// `13 tracks · 44:25`: a track count with its total time, each in the
/// singular when the count is one. The count and the time carry their own type
/// (design D14).
#[must_use]
pub fn track_total_facts(
    total: &crate::bridge::frame::library::GuiMusicTrackTotalFrame,
) -> Segments {
    vec![
        GuiMetaSegmentFrame {
            text: total.count.to_string(),
            kind: GuiMetaKindFrame::Count,
        },
        GuiMetaSegmentFrame {
            text: match total.count {
                1 => " track · ",
                _ => " tracks · ",
            }
            .to_owned(),
            kind: GuiMetaKindFrame::Plain,
        },
        GuiMetaSegmentFrame {
            text: total.duration.clone(),
            kind: GuiMetaKindFrame::Time,
        },
    ]
}

/// An album count's row, `3 albums` in the singular when the count is one.
fn album_count_row(count: usize) -> Segments {
    vec![
        GuiMetaSegmentFrame {
            text: count.to_string(),
            kind: GuiMetaKindFrame::Count,
        },
        GuiMetaSegmentFrame {
            text: match count {
                1 => " album",
                _ => " albums",
            }
            .to_owned(),
            kind: GuiMetaKindFrame::Plain,
        },
    ]
}

/// A year span's text (gui-music-artist-page design D6): the year alone when
/// the span's ends meet, `1986–1989` otherwise.
fn year_span_text(first: &str, last: &str) -> String {
    if first == last {
        first.to_owned()
    } else {
        format!("{first}\u{2013}{last}")
    }
}

/// A year span's row: the span in the date kind, empty while no album names
/// a year.
fn year_span_row(years: Option<&(String, String)>) -> Segments {
    let Some((first, last)) = years else {
        return Vec::new();
    };
    vec![GuiMetaSegmentFrame {
        text: year_span_text(first, last),
        kind: GuiMetaKindFrame::Date,
    }]
}

/// A track count and total time's row, empty while the total has not landed.
fn track_total_row(
    total: Option<&crate::bridge::frame::library::GuiMusicTrackTotalFrame>,
) -> Segments {
    total.map_or_else(Vec::new, track_total_facts)
}

/// The artist header's third line (design D5): the artist's album count in the
/// union, and the track count and total time once every one of those albums
/// holds a cached tracklist.
#[must_use]
pub fn artist_facts(hero: &GuiMusicHeroFrame) -> Segments {
    join_meta_rows(&[
        album_count_row(hero.album_count),
        track_total_row(hero.artist_tracks.as_ref()),
    ])
}

/// The album block's facts line, empty while the album's tracklist has not
/// landed (design D5), and empty while the subject is the artist page — the
/// page's block shows the artist, not an album (gui-music-artist-page design
/// D6).
#[must_use]
pub fn album_facts(hero: &GuiMusicHeroFrame) -> Segments {
    hero.album()
        .and_then(|album| album.total.as_ref())
        .map_or_else(Vec::new, track_total_facts)
}

/// The artist page header's facts line (gui-music-artist-page design D6): the
/// selected collections, the artist's album count in the union, the span of
/// the artist's release years, and the track count and total time once every
/// album of the artist has loaded its tracks. Empty while the subject is an
/// album.
#[must_use]
pub fn artist_page_facts(hero: &GuiMusicHeroFrame) -> Segments {
    let GuiMusicHeroSubjectFrame::Artist(page) = &hero.subject else {
        return Vec::new();
    };
    join_meta_rows(&[
        plain_segments(&hero.collections.join(" + ")),
        album_count_row(hero.album_count),
        year_span_row(page.years.as_ref()),
        track_total_row(hero.artist_tracks.as_ref()),
    ])
}

/// The artist's albums in the union, which Shuffle artist plays (design D8).
/// The union artist is the one that owns the picked album, so a name the
/// Service repeats cannot pick the wrong artist; the picked album alone is the
/// fallback while the union holds no owner. The union's artists are the
/// albums' own content, so the caller passes them alone — the Hero reads no
/// other part of the carried list.
#[must_use]
pub fn artist_union_albums(
    artists: &[crate::bridge::frame::library::GuiMusicArtistFrame],
    hero: &GuiMusicHeroFrame,
) -> Vec<String> {
    // The artist page's albums are the union's own, in the union's order, so
    // the page names them directly (gui-music-artist-page design D6).
    if let GuiMusicHeroSubjectFrame::Artist(page) = &hero.subject {
        return page
            .albums
            .iter()
            .map(|album| album.album_id.clone())
            .collect();
    }
    let Some(picked) = hero.album().map(|album| album.album_id.clone()) else {
        return Vec::new();
    };
    let owner = artists
        .iter()
        .find(|artist| artist.albums.iter().any(|album| album.album_id == picked));
    owner.map_or_else(
        || vec![picked],
        |owner| {
            owner
                .albums
                .iter()
                .map(|album| album.album_id.clone())
                .collect()
        },
    )
}

/// One meta row of a single-row Hero, with the role the frame paints it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroMetaRow {
    /// The row's typed segments.
    pub segments: Segments,
    /// The role the row paints in.
    pub role: HeroMetaRole,
}

/// The role one Hero meta row paints in (design D15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroMetaRole {
    /// A plain facts row.
    Meta,
    /// The row the runtime marks.
    Duration,
    /// The row the progress marks.
    Progress,
}

impl HeroMetaRole {
    /// The role's wire name, as the former union spells it (design D15).
    #[must_use]
    pub fn wire(self) -> &'static str {
        match self {
            Self::Meta => "meta",
            Self::Duration => "duration",
            Self::Progress => "progress",
        }
    }
}

/// The meta rows of a Hero that shows one selected row (`add-gui-library-panel`
/// design D15): each row's typed segments with its duration or progress role, as
/// the core's `durationRow` and `progressRow` indices mark them.
#[must_use]
pub fn hero_meta_rows(
    meta_rows: &[MetaRow],
    duration_row: Option<usize>,
    progress_row: Option<usize>,
) -> Vec<HeroMetaRow> {
    meta_rows
        .iter()
        .enumerate()
        .map(|(index, row)| HeroMetaRow {
            segments: row.clone(),
            role: if Some(index) == duration_row {
                HeroMetaRole::Duration
            } else if Some(index) == progress_row {
                HeroMetaRole::Progress
            } else {
                HeroMetaRole::Meta
            },
        })
        .collect()
}

/// The attribute name of one part of a destination's Hero (`add-gui-library-panel`
/// design D15): `data-library-hero-<name>` for Music (`attr` empty) and
/// `data-library-<attr>-hero-<name>` for the rest. The one rule the frame, the
/// artwork arm, and the action buttons share, so a destination's hooks stay
/// stable.
#[must_use]
pub fn hero_attr(attr: &str, name: &str) -> String {
    if attr.is_empty() {
        format!("data-library-hero-{name}")
    } else {
        format!("data-library-{attr}-hero-{name}")
    }
}

/// The id a Hero's action names (`add-gui-library-panel` design D12): the
/// frame's selection, only while the frame also carries a Hero. A button whose
/// Hero is gone sends nothing, and the guard reads the same in every
/// destination's Hero.
#[must_use]
pub fn selected_with_hero<T: ?Sized>(selected: Option<&T>, hero_present: bool) -> Option<&T> {
    if hero_present { selected } else { None }
}

/// The command a Hero's button sends (`gui-tech-debt-simplification` design
/// D12): destination `D`'s command over the frame's selected item, which sends
/// only while the frame holds both the selection and a Hero. The frame's
/// narrowing and the guard read the same in every lettered destination's Hero,
/// so a Hero names the destination's command and holds no mapping of its own.
/// The `IMDb` link and the chapter and episode Workspace plays name no
/// destination command, so they stay in their Heroes.
pub fn hero_command<D: LetteredDestination>(
    state: &LibraryState,
    detail: &SectionRef<GuiLibraryDetail>,
    command: fn(&LibraryState, &str),
) {
    let Some(part) = D::detail(Some(detail)) else {
        return;
    };
    let Some(item_id) = selected_with_hero(D::selected(part), D::hero_present(part)) else {
        return;
    };
    command(state, item_id);
}

/// The facts every Hero carries, as the core's frames declare them.
#[derive(Debug)]
pub struct HeroLayoutFacts<'a> {
    /// The item's title.
    pub title: &'a str,
    /// The largest unit's name (show, feed, series), when the item has one.
    pub parent: Option<&'a str>,
    /// The selected item's meta rows.
    pub meta_rows: &'a [MetaRow],
    /// The meta row the runtime marks.
    pub duration_row: Option<usize>,
    /// The meta row the progress marks.
    pub progress_row: Option<usize>,
    /// The item's overview.
    pub overview: Option<&'a str>,
    /// Whether the item is an episode. An episode titles the block's name line
    /// under its parent, and that line paints one step smaller
    /// (user request 2026-11-24).
    pub episode: bool,
}

/// The Hero's painted header and block (`add-gui-library-panel` design D15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroLayout {
    /// The header above the artwork.
    pub header: HeroLayoutHeader,
    /// The block beside the artwork.
    pub block: HeroLayoutBlock,
}

/// The Hero header's lines (design D15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroLayoutHeader {
    /// The section label's line.
    pub kicker: Segments,
    /// The big title: the largest unit.
    pub title: String,
    /// The plain facts on one line.
    pub facts: Segments,
}

/// The Hero block's lines beside the artwork (design D15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroLayoutBlock {
    /// The block's label: the item's title.
    pub label: String,
    /// The selected item's name, only when a parent took the header.
    pub name: Option<String>,
    /// Whether the name names an episode: the Hero paints an episode's name one
    /// step smaller than a series-level item's (user request 2026-11-24).
    pub name_episode: bool,
    /// The duration and progress rows.
    pub meta: Vec<HeroMetaRow>,
    /// The item's overview.
    pub overview: Option<String>,
}

/// The frame's header and block for one Hero (`add-gui-library-panel` design D15,
/// invariant 30): the header's big title is the largest unit — the parent when
/// the item has one, else the item itself — over the plain facts on one line;
/// the block beside the art holds the selected item's name (only when a parent
/// took the header), its duration and progress rows, and its overview.
#[must_use]
pub fn hero_layout(facts: &HeroLayoutFacts<'_>, kicker: &str) -> HeroLayout {
    let rows = hero_meta_rows(facts.meta_rows, facts.duration_row, facts.progress_row);
    let parent = facts.parent.filter(|parent| !parent.is_empty());
    HeroLayout {
        header: HeroLayoutHeader {
            kicker: label_segments(kicker),
            title: parent.unwrap_or(facts.title).to_owned(),
            facts: join_meta_rows(
                &rows
                    .iter()
                    .filter(|row| row.role == HeroMetaRole::Meta)
                    .map(|row| row.segments.clone())
                    .collect::<Vec<_>>(),
            ),
        },
        block: HeroLayoutBlock {
            label: facts.title.to_owned(),
            name: parent.map(|_| facts.title.to_owned()),
            name_episode: facts.episode,
            meta: rows
                .into_iter()
                .filter(|row| row.role != HeroMetaRole::Meta)
                .collect(),
            overview: facts.overview.map(str::to_owned),
        },
    }
}

#[cfg(test)]
mod tests;
