//! The Library list's row model (`add-gui-library-panel` design D7, the port
//! of `lib/library/list.ts`; gui-delineated-rows design D1, D2): the canvas
//! tree of artists, albums, and tracks, the 24 px letter headings above the
//! first artist of each letter, and the A–Z letter rail. The artist rows are
//! top-level rows: they carry their art and paint through the shared row part
//! at its height, while the nested album and track rows keep their own painters
//! and heights. The list paints fixed-height rows over native scroll, and the
//! window content owns the open nodes and the filter (design D1), while the
//! scroll store owns the scroll (design D9). This module is plain data:
//! [`super::state`] applies it and the components paint it. Keeping it apart
//! from both keeps the geometry testable without a layout. The window and
//! scroll geometry every list shares lives in [`geometry`].

mod geometry;
mod lettered;

pub use geometry::{
    GeometryRow, ListMove, ListWindow, ROWS_IN_VIEW_OVERSCAN, clamp_scroll_top, list_window,
    move_index, rows_height, rows_in_view_ids, scroll_step, scroll_top_to_show_cursor,
};
pub use lettered::{
    LetterHeading, LetteredRow, SortLetter, build_lettered_rows, carries_letters, lettered_rail,
};

use crate::bridge::frame::library::{
    GuiMusicAlbumFrame, GuiMusicArtistFrame, GuiMusicTrackFrame, GuiRowArtFrame,
    GuiRowMediaKindFrame,
};

/// The canvas's letter heading height in logical pixels (design D7).
pub const LETTER_ROW_HEIGHT: f32 = 24.0;
/// The top-level row's height in logical pixels (gui-delineated-rows design
/// D2): every destination's row constant reads it, so a list's geometry and
/// its painted rows share one height.
pub const LIBRARY_ROW_HEIGHT: f32 = 56.0;
/// The canvas's artist row height in logical pixels (design D2): the artist is
/// a top-level row, so it paints through the shared row part at that height.
pub const ARTIST_ROW_HEIGHT: f32 = LIBRARY_ROW_HEIGHT;
/// The canvas's album row height in logical pixels: a nested row keeps its own
/// height (design D1).
pub const NODE_ROW_HEIGHT: f32 = 30.0;
/// The canvas's track row height in logical pixels.
pub const TRACK_ROW_HEIGHT: f32 = 28.0;

/// The letters the rail spans (design D7): A to Z, as the canvas paints it. A
/// `#` heading for a Service's non-alphabetic names stays off the rail. The
/// Shows list's rail spans the same letters (design D12).
pub const RAIL_LETTERS: &[char] = &[
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

/// The fields every painted row carries: its stable id (the pointer marker's
/// value and the each-block's key), its top edge from the list's first row, its
/// painted height, and the sort letter of the artist it belongs to — the rail
/// jumps to the letter's first row.
#[derive(Debug, Clone, PartialEq)]
pub struct RowBase {
    /// The row's stable id.
    pub id: String,
    /// The row's top edge from the list's first row, in pixels.
    pub top: f32,
    /// The row's painted height, in pixels.
    pub height: f32,
    /// The sort letter of the artist the row belongs to.
    pub letter: char,
}

/// One painted row of the canvas tree (design D7). A letter heading is
/// structural: the pointer marker never names it.
#[derive(Debug, Clone, PartialEq)]
pub enum LibraryRow {
    /// A letter heading above the first artist of the letter.
    Letter(RowBase),
    /// An artist row, open or closed.
    Artist {
        /// The shared geometry and id fields.
        base: RowBase,
        /// The artist's display name.
        name: String,
        /// The Service id the detail command names, or `None` for an artist
        /// the Service does not resolve.
        item_id: Option<String>,
        /// Whether the row paints open.
        open: bool,
        /// Whether the open artist shows child rows: the row starts its guide
        /// under its thumb only while a kept album row paints under it
        /// (gui-tree-indent-guides design D4).
        shows_children: bool,
        /// The row's art (gui-delineated-rows design D4): the artist's cached
        /// image and the fit its pixels ask for, a fetch in flight, a source
        /// with no image, or a kind with no art source.
        art: GuiRowArtFrame,
        /// The media kind the row's thumb shows while it holds no image
        /// (design D4).
        media: GuiRowMediaKindFrame,
    },
    /// An album row under an open artist, open or closed.
    Album {
        /// The shared geometry and id fields.
        base: RowBase,
        /// The album's stable Service id.
        album_id: String,
        /// The album's display name.
        name: String,
        /// The release year, empty when the Service holds none.
        year: String,
        /// Whether the row paints open.
        open: bool,
        /// Whether the open album shows child rows: the row starts its guide
        /// only while a track row paints under it, so an album whose tracks
        /// have not loaded shows none (gui-tree-indent-guides design D4).
        shows_children: bool,
        /// Whether the carried content holds the album's tracklist: an album
        /// without one asks the core to load it when it opens (design D5).
        tracks_loaded: bool,
    },
    /// A track row under an open album.
    Track {
        /// The shared geometry and id fields.
        base: RowBase,
        /// The album the track belongs to: a picked track picks its album
        /// (design D5).
        album_id: String,
        /// The track's stable Service id.
        track_id: String,
        /// The track's display name.
        name: String,
        /// The number the row paints.
        number: i64,
        /// The runtime, `M:SS` or `H:MM:SS`; empty when unknown.
        duration: String,
    },
}

impl LibraryRow {
    /// The row's stable id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.base().id
    }

    /// The row's geometry: its id, top edge, height, and letter.
    #[must_use]
    pub fn base(&self) -> &RowBase {
        match self {
            LibraryRow::Letter(base)
            | LibraryRow::Artist { base, .. }
            | LibraryRow::Album { base, .. }
            | LibraryRow::Track { base, .. } => base,
        }
    }

    /// Whether a key may act on the row: a letter heading is structural and
    /// never carries the pointer marker.
    #[must_use]
    pub fn selectable(&self) -> bool {
        !matches!(self, LibraryRow::Letter(_))
    }
}

impl GeometryRow for LibraryRow {
    fn id(&self) -> &str {
        LibraryRow::id(self)
    }

    fn pointer_row(&self) -> Option<&str> {
        self.selectable().then_some(LibraryRow::id(self))
    }

    fn rows_in_view_id(&self) -> Option<String> {
        match self {
            LibraryRow::Artist {
                item_id: Some(item_id),
                ..
            } => Some(format!("artist:{item_id}")),
            LibraryRow::Artist {
                name,
                item_id: None,
                ..
            } => Some(format!("artist-name:{name}")),
            // A nested row names no thumb (design D5), and a letter heading
            // names no item.
            LibraryRow::Letter(_) | LibraryRow::Album { .. } | LibraryRow::Track { .. } => None,
        }
    }

    fn top(&self) -> f32 {
        self.base().top
    }

    fn height(&self) -> f32 {
        self.base().height
    }
}

/// An artist row's id: the artist's stable identity, prefixed so an artist and
/// an album of the same Service id cannot collide.
#[must_use]
pub fn artist_row_id(artist: &GuiMusicArtistFrame) -> String {
    let (kind, id) = match &artist.artist_key {
        crate::bridge::frame::library::GuiArtistKey::Service(id) => ("service", id),
        crate::bridge::frame::library::GuiArtistKey::Fallback(id) => ("fallback", id),
    };
    format!("artist:{kind}:{id}")
}

/// An album row's id.
#[must_use]
pub fn album_row_id(album_id: &str) -> String {
    format!("album:{album_id}")
}

/// A track row's id.
#[must_use]
pub fn track_row_id(track_id: &str) -> String {
    format!("track:{track_id}")
}

/// Whether a row name survives the filter (design D7, D12): the name contains
/// the filter's text, without case, and an empty filter keeps every name. The
/// same rule narrows every Library destination's list — the canvas tree's, and
/// the flat and headed lists of the other destinations.
#[must_use]
pub fn filter_match(name: &str, filter: &str) -> bool {
    let needle = filter.trim().to_lowercase();
    needle.is_empty() || name.to_lowercase().contains(&needle)
}

/// The input the row model builds from: the carried union, the open nodes, and
/// the filter text.
#[derive(Debug, Clone, Copy)]
pub struct LibraryTree<'a> {
    /// The carried artists in their carried order.
    pub artists: &'a [GuiMusicArtistFrame],
    /// The open node ids (artist and album row ids).
    pub open: &'a [String],
    /// The node ids the user closed while the filter was narrowing the rows:
    /// they override the filter's auto-open, so a press still closes a node
    /// under a filter (design D7's open and close).
    pub closed: &'a [String],
    /// The filter text: the list paints only the rows whose name contains it,
    /// without case (design D7).
    pub filter: &'a str,
}

/// The painted rows of the canvas tree (design D7): the artists of the union in
/// their carried order, each with its albums when the artist is open and its
/// tracks when the album is open, under the letter heading of the first artist
/// of each sort letter. Every row carries its top edge and height, so the list
/// can paint a window of rows over native scroll and the rail can scroll to a
/// letter.
///
/// The filter narrows the rows to the names that contain the text, without
/// case: a row paints when its own name matches. An artist or album whose rows
/// all drop out stays when one of its descendants matches, so a match is never
/// orphaned, and a node with a matching child paints open so no match hides —
/// unless the user closed that node under the filter, which the closed set
/// keeps shut. Outside a filter, a node paints open when the window content has
/// it open, even while an album's tracklist is still loading.
#[must_use]
pub fn build_library_rows(tree: LibraryTree<'_>) -> Vec<LibraryRow> {
    let needle = tree.filter.trim().to_lowercase();
    let narrowing = !needle.is_empty();
    // Whether a name contains the filter's text; an empty filter matches
    // every name.
    let matches = |name: &str| -> bool { !narrowing || name.to_lowercase().contains(&needle) };

    let mut builder = RowBuilder::default();
    let mut heading: Option<char> = None;
    for artist in tree.artists {
        let albums = filtered_albums(artist, &matches);
        if !matches(&artist.name) && albums.iter().all(|(_, _, kept)| !kept) {
            continue;
        }
        push_letter_and_artist(
            &mut builder,
            &mut heading,
            artist,
            &albums,
            &tree,
            narrowing,
        );
        if !builder.artist_open {
            continue;
        }
        push_albums(&mut builder, &albums, &tree, narrowing);
    }
    builder.rows
}

/// One artist's albums over the filter: the album, the tracks the filter
/// keeps, and whether the album paints at all (design D7).
type FilteredAlbum<'a> = (&'a GuiMusicAlbumFrame, Vec<&'a GuiMusicTrackFrame>, bool);

/// The albums of one artist, each with the tracks the filter keeps and the
/// paint flag: an album paints when its name matches or a track does.
fn filtered_albums<'a>(
    artist: &'a GuiMusicArtistFrame,
    matches: &impl Fn(&str) -> bool,
) -> Vec<FilteredAlbum<'a>> {
    artist
        .albums
        .iter()
        .map(|album| {
            let tracks: Vec<&GuiMusicTrackFrame> = album
                .tracks
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter(|track| matches(&track.name))
                .collect();
            let kept = matches(&album.name) || !tracks.is_empty();
            (album, tracks, kept)
        })
        .collect()
}

/// Push the letter heading above the first artist of the letter, then the
/// artist row, and leave the builder saying whether the artist paints open
/// (design D7).
fn push_letter_and_artist(
    builder: &mut RowBuilder,
    heading: &mut Option<char>,
    artist: &GuiMusicArtistFrame,
    albums: &[FilteredAlbum<'_>],
    tree: &LibraryTree<'_>,
    narrowing: bool,
) {
    let letter = artist.sort_letter;
    let artist_id = artist_row_id(artist);
    if *heading != Some(letter) {
        *heading = Some(letter);
        builder.push(
            LibraryRow::Letter(RowBase {
                id: format!("letter:{letter}"),
                top: 0.0,
                height: 0.0,
                letter,
            }),
            LETTER_ROW_HEIGHT,
        );
    }
    let any_kept_albums = albums.iter().any(|(_, _, kept)| *kept);
    let artist_open = if narrowing {
        any_kept_albums && !tree.closed.contains(&artist_id)
    } else {
        tree.open.contains(&artist_id)
    };
    builder.push(
        LibraryRow::Artist {
            base: RowBase {
                id: artist_id,
                top: 0.0,
                height: 0.0,
                letter,
            },
            name: artist.name.clone(),
            item_id: match &artist.artist_key {
                crate::bridge::frame::library::GuiArtistKey::Service(id) => Some(id.clone()),
                crate::bridge::frame::library::GuiArtistKey::Fallback(_) => None,
            },
            open: artist_open,
            shows_children: artist_open && any_kept_albums,
            art: artist.art.clone(),
            media: artist.media,
        },
        ARTIST_ROW_HEIGHT,
    );
    builder.artist_open = artist_open;
}

/// Push the artist's painted albums and, where an album paints open, its
/// tracks (design D7).
fn push_albums(
    builder: &mut RowBuilder,
    albums: &[FilteredAlbum<'_>],
    tree: &LibraryTree<'_>,
    narrowing: bool,
) {
    let letter = builder.rows.last().map(|row| row.base().letter);
    let Some(letter) = letter else {
        return;
    };
    for (album, tracks, kept) in albums {
        if !kept {
            continue;
        }
        let album_id = album_row_id(&album.album_id);
        let album_open = if narrowing {
            !tracks.is_empty() && !tree.closed.contains(&album_id)
        } else {
            tree.open.contains(&album_id)
        };
        builder.push(
            LibraryRow::Album {
                base: RowBase {
                    id: album_id,
                    top: 0.0,
                    height: 0.0,
                    letter,
                },
                album_id: album.album_id.clone(),
                name: album.name.clone(),
                year: album.year.clone(),
                open: album_open,
                shows_children: album_open && !tracks.is_empty(),
                tracks_loaded: album.tracks.is_some(),
            },
            NODE_ROW_HEIGHT,
        );
        if !album_open {
            continue;
        }
        for track in tracks {
            builder.push(
                LibraryRow::Track {
                    base: RowBase {
                        id: track_row_id(&track.track_id),
                        top: 0.0,
                        height: 0.0,
                        letter,
                    },
                    album_id: album.album_id.clone(),
                    track_id: track.track_id.clone(),
                    name: track.name.clone(),
                    number: track.number,
                    duration: track.duration.clone(),
                },
                TRACK_ROW_HEIGHT,
            );
        }
    }
}

#[derive(Default)]
struct RowBuilder {
    rows: Vec<LibraryRow>,
    top: f32,
    /// Whether the artist row the builder pushed last paints open.
    artist_open: bool,
}

impl RowBuilder {
    fn push(&mut self, mut row: LibraryRow, height: f32) {
        let base = match &mut row {
            LibraryRow::Letter(base)
            | LibraryRow::Artist { base, .. }
            | LibraryRow::Album { base, .. }
            | LibraryRow::Track { base, .. } => base,
        };
        base.top = self.top;
        base.height = height;
        self.top += height;
        self.rows.push(row);
    }
}

/// One letter of the rail (design D7): the letter and the first artist row
/// under it, or `None`'s row while no painted row carries the letter — a
/// disabled letter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailLetter {
    /// The rail letter.
    pub letter: char,
    /// The first anchored row's id, `None` while the letter is disabled.
    pub row_id: Option<String>,
}

/// The A–Z rail over the canvas tree's painted rows (design D7): each letter
/// with the first artist row under it, so a press scrolls to that row and
/// moves the cursor there. A letter no visible row carries is disabled.
#[must_use]
pub fn letter_rail(rows: &[LibraryRow]) -> Vec<RailLetter> {
    letter_rail_by(rows, |row| match row {
        LibraryRow::Artist { base, .. } => Some(base.letter),
        _ => None,
    })
}

/// The A–Z rail over a destination's painted rows (design D7, D12, D14):
/// each letter with the first row `letter_of` anchors under it — a plain
/// list's item rows, the Shows tree's series rows, never a heading. A letter
/// no anchor carries is disabled.
#[must_use]
pub fn letter_rail_by<Row: GeometryRow>(
    rows: &[Row],
    letter_of: impl Fn(&Row) -> Option<char>,
) -> Vec<RailLetter> {
    RAIL_LETTERS
        .iter()
        .map(|letter| RailLetter {
            letter: *letter,
            row_id: rows
                .iter()
                .find(|row| letter_of(row) == Some(*letter))
                .map(|row| row.id().to_owned()),
        })
        .collect()
}

/// The cursor and scroll offset a rail press leaves (design D7): the first
/// row under `letter`, scrolled so it paints fully — or `None` while no
/// painted row carries the letter, so the press changes nothing. `view` is
/// the list's scroll offset and measured viewport height, the fields every
/// destination's scroll store carries.
#[must_use]
pub fn jump_to_letter(
    rail: &[RailLetter],
    rows: &[impl GeometryRow],
    letter: char,
    view: super::scroll::ListScroll,
) -> Option<(String, f32)> {
    let target = rail
        .iter()
        .find(|entry| entry.letter == letter)
        .and_then(|entry| entry.row_id.as_deref())?;
    Some((
        target.to_owned(),
        scroll_top_to_show_cursor(rows, Some(target), view.scroll_top, view.viewport_height),
    ))
}

#[cfg(test)]
mod tests;
