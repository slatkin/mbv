//! The Library Hero's facts and geometry (`add-gui-library-panel` design D5, D8,
//! the port of `lib/library/hero.ts`): the picked album's lines as the canvas
//! prints them, and the artist's albums in the union that Shuffle artist
//! plays. The data is the core's, so this module only reads the carried Hero;
//! keeping the lines here lets the painters stay markup.
//!
//! A line that joins values of several data types is built as typed segments
//! (`add-gui-settings-screen` design D14): colour carries the type, so a count
//! and a time on one line each carry their own kind, and the painter paints the
//! kinds through `TypedText`. A fact that stands alone is a pill over its
//! typed segments (`gui-hero-design-polish` design D5).

use crate::bridge::frame::library::{
    GuiHeroPillFrame, GuiLibraryDetail, GuiMetaKindFrame, GuiMetaSegmentFrame, GuiMusicHeroFrame,
    GuiMusicHeroSubjectFrame, GuiMusicTrackTotalFrame,
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

/// One fact pill over its typed segments (design D1): a plain fact, no link.
fn fact_pill(segments: Segments) -> GuiHeroPillFrame {
    GuiHeroPillFrame {
        segments,
        link: None,
    }
}

/// `13 tracks` and `44:25` as two pills: the track count, then the total time.
/// The count is singular when it is one, and each value carries its own type
/// (design D14).
#[must_use]
pub fn track_total_pills(total: &GuiMusicTrackTotalFrame) -> Vec<GuiHeroPillFrame> {
    vec![
        fact_pill(vec![
            GuiMetaSegmentFrame {
                text: total.count.to_string(),
                kind: GuiMetaKindFrame::Count,
            },
            GuiMetaSegmentFrame {
                text: match total.count {
                    1 => " track",
                    _ => " tracks",
                }
                .to_owned(),
                kind: GuiMetaKindFrame::Plain,
            },
        ]),
        fact_pill(vec![GuiMetaSegmentFrame {
            text: total.duration.clone(),
            kind: GuiMetaKindFrame::Time,
        }]),
    ]
}

/// An album count's pill, `3 albums` in the singular when the count is one.
fn album_count_pill(count: usize) -> GuiHeroPillFrame {
    fact_pill(vec![
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
    ])
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

/// The album block's pills (design D5): the album's track count and total
/// time, empty while the album's tracklist has not landed and while the
/// subject is the artist page (the page's block shows the artist, not an
/// album; gui-music-artist-page design D6).
#[must_use]
pub fn album_pills(hero: &GuiMusicHeroFrame) -> Vec<GuiHeroPillFrame> {
    hero.album()
        .and_then(|album| album.total.as_ref())
        .map_or_else(Vec::new, track_total_pills)
}

/// The artist page header's pills (gui-music-artist-page design D6, design
/// D5): the artist's album count in the union, the span of the artist's
/// release years, and the track count and total time once every album of the
/// artist has loaded its tracks. Empty while the subject is an album.
#[must_use]
pub fn artist_page_pills(hero: &GuiMusicHeroFrame) -> Vec<GuiHeroPillFrame> {
    let GuiMusicHeroSubjectFrame::Artist(page) = &hero.subject else {
        return Vec::new();
    };
    let mut pills = vec![album_count_pill(hero.album_count)];
    if let Some((first, last)) = page.years.as_ref() {
        pills.push(fact_pill(vec![GuiMetaSegmentFrame {
            text: year_span_text(first, last),
            kind: GuiMetaKindFrame::Date,
        }]));
    }
    if let Some(total) = hero.artist_tracks.as_ref() {
        pills.extend(track_total_pills(total));
    }
    pills
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

#[cfg(test)]
mod tests;
