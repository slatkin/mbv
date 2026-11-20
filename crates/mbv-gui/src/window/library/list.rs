//! The Library list's row model (`add-gui-library-panel` design D7, the port
//! of `lib/library/list.ts`): the canvas tree of artists, albums, and tracks,
//! the 24 px letter headings above the first artist of each letter, the rows
//! in view, the scroll geometry, and the A–Z letter rail. The list paints
//! fixed-height rows over native scroll, and the window content owns the open
//! nodes and the filter (design D1), while the scroll store owns the scroll
//! (design D9). This module is plain data:
//! [`super::state`] applies it and the components paint it. Keeping it apart
//! from both keeps the geometry testable without a layout.

use crate::bridge::frame::library::{GuiMusicAlbumFrame, GuiMusicArtistFrame, GuiMusicTrackFrame};

/// The canvas's letter heading height in logical pixels (design D7).
pub const LETTER_ROW_HEIGHT: f32 = 24.0;
/// The canvas's artist and album row height in logical pixels.
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

/// The rows a `RowsInView` report adds above and below the painted window
/// (gui-delineated-rows design D7): the row art of the rows just outside the
/// viewport is fetched too, so a small scroll paints no empty thumb.
pub const ROWS_IN_VIEW_OVERSCAN: usize = 4;

/// The geometry the scroll and window geometry read (`add-gui-library-panel`
/// design D7, D12): the stable id, the top edge from the list's first row, and
/// the painted height. The Library tree's rows and the Home list's rows both
/// satisfy it, so one window implementation serves both lists.
pub trait GeometryRow {
    /// The row's stable id.
    fn id(&self) -> &str;

    /// The id the row's pointer marker carries
    /// (`gui-pointer-target-interaction` D1), or `None` for a structural row
    /// that no key acts on: a letter heading names no item, so the pointer
    /// over it names no row.
    fn pointer_row(&self) -> Option<&str> {
        Some(self.id())
    }

    /// The id this row takes in a `RowsInView` report (gui-delineated-rows
    /// design D7), or `None` for a row whose kind has no art source. The core
    /// resolves the id with no coordinates, and a nested row names none — its
    /// kind paints no thumb. A row overrides this where the core's own
    /// encoding differs from its id (`HomeListRow`, `LibraryRow::Artist`).
    fn rows_in_view_id(&self) -> Option<String> {
        self.pointer_row().map(str::to_owned)
    }

    /// The row's top edge from the list's first row.
    fn top(&self) -> f32;

    /// The row's painted height.
    fn height(&self) -> f32;
}

impl<T: GeometryRow> GeometryRow for &T {
    fn id(&self) -> &str {
        T::id(self)
    }

    fn pointer_row(&self) -> Option<&str> {
        T::pointer_row(self)
    }

    fn rows_in_view_id(&self) -> Option<String> {
        T::rows_in_view_id(self)
    }

    fn top(&self) -> f32 {
        T::top(self)
    }

    fn height(&self) -> f32 {
        T::height(self)
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
        },
        NODE_ROW_HEIGHT,
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

/// The painted height of the whole list: the bottom edge of its last row.
#[must_use]
pub fn rows_height(rows: &[impl GeometryRow]) -> f32 {
    rows.last().map_or(0.0, |row| row.top() + row.height())
}

/// One painted row of a flat fixed-height list (design D12): the carried
/// item under its stable id, at `top` for `height`, so the window and the
/// scroll geometry read it like every other list's rows. `Kind` carries a
/// list's row discriminator when its union names one; the plain lists leave
/// it `()`.
#[derive(Debug, Clone, PartialEq)]
pub struct FlatRow<Item, Kind = ()> {
    /// The row's stable id.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The carried item the row paints.
    pub row: Item,
    /// The row discriminator a list's union names, `()` for the plain lists.
    pub kind: Kind,
}

/// The fields of a flat list's carried item the row model reads (design D12):
/// the stable id and the title the filter narrows on.
pub trait FlatItem {
    /// The row's stable id.
    fn row_id(&self) -> &str;

    /// The title the filter narrows on.
    fn row_title(&self) -> &str;
}

/// The painted rows of a flat fixed-height list (design D12): the carried
/// items in their order — a date-sorted list keeps the order the frame
/// carried — each `row_height` tall, narrowed to the titles that contain
/// `filter`'s text without case (design D7's filter rule). Every row carries
/// its top edge and height, so the list paints a window of rows over native
/// scroll.
#[must_use]
pub fn build_flat_rows<'a, Item: FlatItem>(
    rows: &'a [Item],
    row_height: f32,
    filter: &str,
) -> Vec<FlatRow<&'a Item>> {
    build_flat_rows_with_kind(rows, row_height, filter, ())
}

/// The painted rows of a flat list whose row union discriminates on a kind:
/// the Emby feed-group rows are items beside the plain mode's headings, and
/// the Shows Latest and Upcoming rows are flats beside the series tree.
#[must_use]
pub fn build_flat_rows_with_kind<'a, Item: FlatItem, Kind: Clone>(
    rows: &'a [Item],
    row_height: f32,
    filter: &str,
    kind: Kind,
) -> Vec<FlatRow<&'a Item, Kind>> {
    let mut top = 0.0;
    rows.iter()
        .filter(|row| filter_match(row.row_title(), filter))
        .map(|row| {
            let flat = FlatRow {
                id: row.row_id().to_owned(),
                row,
                top,
                height: row_height,
                kind: kind.clone(),
            };
            top += row_height;
            flat
        })
        .collect()
}

/// A list movement the design's keys name (design D3): row by row for Up and
/// Down, one viewport for Page Up and Page Down, and the list's ends for Home
/// and End.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMove {
    /// One row up.
    Up,
    /// One row down.
    Down,
    /// One viewport up.
    PageUp,
    /// One viewport down.
    PageDown,
    /// The list's first row.
    Home,
    /// The list's last row.
    End,
}

/// The index a movement leaves over `count` rows from `at` (design D7, D12):
/// the one clamp every list and the Settings panes share. `page` is the rows a
/// page spans.
#[must_use]
pub fn move_index(count: usize, at: usize, mv: ListMove, page: usize) -> usize {
    let step = page.max(1);
    let to = match mv {
        ListMove::Up => at.saturating_sub(1),
        ListMove::Down => at.saturating_add(1),
        ListMove::PageUp => at.saturating_sub(step),
        ListMove::PageDown => at.saturating_add(step),
        ListMove::Home => 0,
        ListMove::End => count.saturating_sub(1),
    };
    to.min(count.saturating_sub(1))
}

/// The half-open range of row indexes the viewport paints (design D7): the rows
/// that intersect `[scroll_top, scroll_top + viewport_height)`. An unmeasured
/// viewport paints nothing, so a long list never paints in full while it waits
/// for its first measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListWindow {
    /// The first row in view.
    pub start: usize,
    /// One past the last row in view.
    pub end: usize,
}

/// The rows in view: only these paint, so a long list stays fast.
#[must_use]
pub fn list_window(rows: &[impl GeometryRow], scroll_top: f32, viewport_height: f32) -> ListWindow {
    if viewport_height <= 0.0 {
        return ListWindow { start: 0, end: 0 };
    }
    let top = scroll_top.max(0.0);
    let bottom = top + viewport_height;
    let Some(start) = rows.iter().position(|row| row.top() + row.height() > top) else {
        let len = rows.len();
        return ListWindow {
            start: len,
            end: len,
        };
    };
    let mut end = start;
    while end < rows.len() && rows[end].top() < bottom {
        end += 1;
    }
    ListWindow { start, end }
}

/// The stable ids a `RowsInView` report carries (gui-delineated-rows design
/// D7): the painted window's rows plus [`ROWS_IN_VIEW_OVERSCAN`] rows above and
/// below it. An unmeasured viewport paints nothing, so its list reports no rows
/// — the set arrives once the list is measured. A row whose kind paints no
/// thumb names no id.
#[must_use]
pub fn rows_in_view_ids<Row: GeometryRow>(rows: &[Row], window: ListWindow) -> Vec<String> {
    if window.end <= window.start {
        return Vec::new();
    }
    let start = window.start.saturating_sub(ROWS_IN_VIEW_OVERSCAN);
    let end = window
        .end
        .saturating_add(ROWS_IN_VIEW_OVERSCAN)
        .min(rows.len());
    rows[start..end]
        .iter()
        .filter_map(GeometryRow::rows_in_view_id)
        .collect()
}

/// A scroll offset held inside the list's scrollable range.
#[must_use]
pub fn clamp_scroll_top(rows: &[impl GeometryRow], scroll_top: f32, viewport_height: f32) -> f32 {
    let max = (rows_height(rows) - viewport_height.max(0.0)).max(0.0);
    scroll_top.max(0.0).min(max)
}

/// The scroll offset that keeps the cursor row fully visible (design D7, the
/// spec's "the list scrolls so that the cursor row is fully visible"): the row's
/// top when it sits above the viewport, its bottom edge when it sits below, and
/// an unchanged, clamped offset while it already shows. A cursor with no row
/// moves nothing.
#[must_use]
pub fn scroll_top_to_show_cursor<Row: GeometryRow>(
    rows: &[Row],
    cursor: Option<&str>,
    scroll_top: f32,
    viewport_height: f32,
) -> f32 {
    let row = cursor.and_then(|cursor| rows.iter().find(|row| row.id() == cursor));
    let top = scroll_top.max(0.0);
    // An unmeasured viewport scrolls nothing: the first measurement takes the
    // cursor's row (design D9).
    let Some(row) = row.filter(|_| viewport_height > 0.0) else {
        return clamp_scroll_top(rows, top, viewport_height);
    };
    if row.top() < top {
        clamp_scroll_top(rows, row.top(), viewport_height)
    } else if row.top() + row.height() > top + viewport_height {
        clamp_scroll_top(
            rows,
            row.top() + row.height() - viewport_height,
            viewport_height,
        )
    } else {
        clamp_scroll_top(rows, top, viewport_height)
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

/// The scroll offset a movement key leaves (design D6 of
/// `gui-pointer-target-interaction`): Up and Down move by the height of the row
/// at the top edge, Page Up and Page Down by the viewport height, and Home and
/// End go to the list's ends. The value stays inside `[0, total − viewport]`,
/// so a key at an end moves nothing. `view` is the list's scroll offset and
/// measured viewport height, the fields every destination's scroll store
/// carries.
#[must_use]
pub fn scroll_step(
    rows: &[impl GeometryRow],
    view: super::scroll::ListScroll,
    mv: ListMove,
) -> f32 {
    let step = match mv {
        ListMove::Up => -top_row_height(rows, view.scroll_top),
        ListMove::Down => top_row_height(rows, view.scroll_top),
        ListMove::PageUp => -view.viewport_height,
        ListMove::PageDown => view.viewport_height,
        ListMove::Home => return clamp_scroll_top(rows, 0.0, view.viewport_height),
        ListMove::End => {
            return clamp_scroll_top(rows, rows_height(rows), view.viewport_height);
        }
    };
    clamp_scroll_top(rows, view.scroll_top + step, view.viewport_height)
}

/// The height of the row that covers the list's top edge: the row the first
/// visible pixel belongs to, the last row's height while the offset lies past
/// them, and no step while the list paints nothing.
#[must_use]
fn top_row_height(rows: &[impl GeometryRow], scroll_top: f32) -> f32 {
    let top = scroll_top.max(0.0);
    rows.iter()
        .find(|row| row.top() + row.height() > top)
        .or_else(|| rows.last())
        .map_or(0.0, GeometryRow::height)
}

#[cfg(test)]
mod tests;
