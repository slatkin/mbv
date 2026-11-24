//! The Shows list's row model (`add-gui-library-panel` design D14, the port
//! of `lib/library/shows-list.ts`): the Music tree over the TUI's TV node
//! kinds — series → season → episode — with the 24 px letter headings above
//! the first series of each letter, the rows in view, and the A–Z letter rail.
//! The GUI Shows destination paints this series list alone (`gui-collection-dropdown`
//! design D8), so the list carries no view mode. The list paints fixed-height
//! rows over native scroll and only the rows in view,
//! as the Music list does, and the keys act on the row the pointer marks
//! (`gui-pointer-target-interaction` D2), which is never a heading. The model is
//! plain data over fixed heights, so the geometry is asserted without a layout.

use crate::gui_core::GuiShowsView;

use crate::bridge::frame::library::{
    GuiRowArtFrame, GuiRowMediaKindFrame, GuiShowsEpisodeRowFrame, GuiShowsRowFrame,
    GuiShowsSeasonFrame, GuiShowsSeriesFrame,
};
use crate::window::library::list::{
    GeometryRow, LETTER_ROW_HEIGHT, LIBRARY_ROW_HEIGHT, LetterHeading, LetteredRow,
    NODE_ROW_HEIGHT, RailLetter, TRACK_ROW_HEIGHT, build_lettered_rows, filter_match,
    letter_rail_by,
};

/// The letter heading row's height in logical pixels: the Music list's letter
/// heading, which the All series list reuses.
pub const SHOWS_HEADING_ROW_HEIGHT: f32 = LETTER_ROW_HEIGHT;
/// The series row's height in logical pixels (gui-delineated-rows design D2):
/// the series is a top-level row, so it paints through the shared row part at
/// that part's height.
pub const SHOWS_SERIES_ROW_HEIGHT: f32 = LIBRARY_ROW_HEIGHT;
/// The season row's height: the Music node row's, because a season is a nested
/// row and keeps its own painter (design D1).
pub const SHOWS_SEASON_ROW_HEIGHT: f32 = NODE_ROW_HEIGHT;
/// The episode leaf's height in logical pixels: the Music track row.
pub const SHOWS_EPISODE_ROW_HEIGHT: f32 = TRACK_ROW_HEIGHT;
/// The flat New or Upcoming row's height in logical pixels (gui-delineated-rows
/// design D2): the shared top-level row part's height, whose two title lines
/// and date gutter replace the old Home item row's format (gui-collection-dropdown
/// D9, the live defect of 2026-11-18: the New and Upcoming rows painted cut
/// off).
pub const SHOWS_FLAT_ROW_HEIGHT: f32 = LIBRARY_ROW_HEIGHT;

/// The tree's open and closed node keys (design D14): the Music tree's rule.
/// `open` holds the nodes the user opened, and `closed` holds the nodes the
/// user closed while the filter was narrowing the rows, which override the
/// filter's auto-open.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShowsTreeState<'a> {
    /// The node row ids the user opened.
    pub open: &'a [String],
    /// The node row ids the user closed under a filter.
    pub closed: &'a [String],
}

/// The interaction facts the Shows rows are memoized on (design D11): the view
/// mode, the filter text, and the tree's open and closed nodes. The carried
/// content these narrow comes from the memo's `libraryList` revision, so the
/// hit test compares only these.
#[derive(Debug, Clone, PartialEq)]
pub struct ShowsRowsFacts {
    /// The view mode the rows were built for.
    pub view: GuiShowsView,
    /// The filter text the rows were narrowed by.
    pub filter: String,
    /// The node row ids the user opened.
    pub open: Vec<String>,
    /// The node row ids the user closed under a filter.
    pub closed: Vec<String>,
}

/// A painted series node (design D14): the Music artist row's counterpart.
#[derive(Debug, Clone, PartialEq)]
pub struct ShowsSeriesRow {
    /// The row's stable key.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The series' stable Service id: the select command names it.
    pub source_id: String,
    /// The series' display name.
    pub title: String,
    /// The rail letter the row paints under.
    pub letter: char,
    /// Whether the row paints open.
    pub open: bool,
    /// Whether the open series shows child rows: the row starts its guide under
    /// its thumb only while a kept season row paints under it
    /// (gui-tree-indent-guides design D4).
    pub shows_children: bool,
    /// Whether the node opens.
    pub expandable: bool,
    /// The series' art (gui-delineated-rows design D4): the cached row image
    /// and the fit its pixels ask for, a fetch in flight, a source with no
    /// image, or a kind with no art source.
    pub art: GuiRowArtFrame,
    /// The media kind the row's thumb shows while it holds no image (design
    /// D4).
    pub media: GuiRowMediaKindFrame,
}

/// A painted season node (design D14): the Music album row's counterpart.
#[derive(Debug, Clone, PartialEq)]
pub struct ShowsSeasonRow {
    /// The row's stable key.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The season's stable Service id: the select-season command names it.
    pub source_id: String,
    /// The series the season belongs to, so an open asks for the pair.
    pub series_id: String,
    /// The season's display name.
    pub title: String,
    /// The rail letter the row paints under.
    pub letter: char,
    /// Whether the row paints open.
    pub open: bool,
    /// Whether the node opens.
    pub expandable: bool,
    /// Whether the season's episode load has landed: an open season without
    /// one asks the core for them.
    pub loaded: bool,
}

impl ShowsSeasonRow {
    /// Whether the open season shows child rows: the row starts its guide only
    /// while an episode row paints under it, so a season whose episodes have not
    /// landed shows none (gui-tree-indent-guides design D4). The row's own
    /// fields already carry the three facts the builder knows — it paints open,
    /// its episode load has landed, and its kept episodes are not empty — so the
    /// flag derives here instead of taking a fourth bool field.
    #[must_use]
    pub fn shows_children(&self) -> bool {
        self.open && self.loaded && self.expandable
    }
}

/// A painted episode leaf (design D14): the Music track row's counterpart.
#[derive(Debug, Clone, PartialEq)]
pub struct ShowsEpisodeRow {
    /// The row's stable key.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The episode's stable Service id: a play command names it.
    pub source_id: String,
    /// The series the episode belongs to.
    pub series_id: String,
    /// The episode's display name.
    pub title: String,
    /// The number the row paints.
    pub number: i64,
    /// The runtime, `M:SS` or `H:MM:SS`; empty when unknown.
    pub duration: String,
    /// The rail letter the row paints under.
    pub letter: char,
}

/// One painted row of the New or Upcoming list (gui-collection-dropdown
/// design D9): the stable Service id, the two title lines, and the gutter
/// date the old Home sections carried. An Upcoming row is display-only.
#[derive(Debug, Clone, PartialEq)]
pub struct ShowsFlatRow {
    /// The row's stable key and command id.
    pub id: String,
    /// The primary title line.
    pub title: String,
    /// The secondary title line.
    pub subtitle: Option<String>,
    /// The gutter date.
    pub date: Option<String>,
    /// Whether the row takes no select, play, or enqueue.
    pub display_only: bool,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArtFrame,
    /// The media kind the row's thumb shows while it holds no image (design
    /// D4).
    pub media: GuiRowMediaKindFrame,
    /// The rail letter a title-sorted New row paints under
    /// (gui-flat-list-sort design D6): `None` under a date sort and on every
    /// Upcoming row.
    pub letter: Option<char>,
}

/// One painted row of the Shows list (design D14). The `id` is the row's
/// stable key: the pointer marker's value and the each-block's key. A heading
/// is structural; the keys act on the flat and tree rows.
#[derive(Debug, Clone, PartialEq)]
pub enum ShowsListRow {
    /// A letter heading above the first series of the letter.
    Heading(LetterHeading),
    /// A series node, open or closed.
    Series(ShowsSeriesRow),
    /// A season node under an open series, open or closed.
    Season(ShowsSeasonRow),
    /// An episode leaf under an open season.
    Episode(ShowsEpisodeRow),
    /// A flat New or Upcoming row.
    Flat(ShowsFlatRow),
}

impl ShowsListRow {
    /// The row's stable id.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            ShowsListRow::Heading(row) => &row.id,
            ShowsListRow::Series(row) => &row.id,
            ShowsListRow::Season(row) => &row.id,
            ShowsListRow::Episode(row) => &row.id,
            ShowsListRow::Flat(row) => &row.id,
        }
    }

    /// The row's rail letter, or `None` on a row no rail anchors
    /// (gui-flat-list-sort design D8): every tree row carries the letter of
    /// its series, and a flat row carries one exactly while the New view sorts
    /// by name.
    #[must_use]
    pub fn letter(&self) -> Option<char> {
        match self {
            ShowsListRow::Heading(heading) => Some(heading.letter),
            ShowsListRow::Series(row) => Some(row.letter),
            ShowsListRow::Season(row) => Some(row.letter),
            ShowsListRow::Episode(row) => Some(row.letter),
            ShowsListRow::Flat(row) => row.letter,
        }
    }

    /// The row's node key when the row is a node the open and close acts
    /// apply to: a series or a season. A leaf and a heading name none.
    #[must_use]
    pub fn as_node(&self) -> Option<&str> {
        match self {
            ShowsListRow::Series(row) => Some(&row.id),
            ShowsListRow::Season(row) => Some(&row.id),
            _ => None,
        }
    }
}

impl GeometryRow for ShowsListRow {
    fn id(&self) -> &str {
        ShowsListRow::id(self)
    }

    fn pointer_row(&self) -> Option<&str> {
        (!matches!(self, ShowsListRow::Heading(_))).then_some(ShowsListRow::id(self))
    }

    /// The id this row takes in a `RowsInView` report (gui-delineated-rows
    /// design D7): the core resolves it against its own data, never against the
    /// row key this list adds. A series row names its stable Service id — the
    /// raw Emby id — and a flat row the id it already carries; a heading, a
    /// season, and an episode paint no thumb, so none of them consumes the set.
    fn rows_in_view_id(&self) -> Option<String> {
        match self {
            ShowsListRow::Series(row) => Some(row.source_id.clone()),
            ShowsListRow::Flat(row) => Some(row.id.clone()),
            ShowsListRow::Heading(_) | ShowsListRow::Season(_) | ShowsListRow::Episode(_) => None,
        }
    }

    fn top(&self) -> f32 {
        match self {
            ShowsListRow::Heading(row) => row.top,
            ShowsListRow::Series(row) => row.top,
            ShowsListRow::Season(row) => row.top,
            ShowsListRow::Episode(row) => row.top,
            ShowsListRow::Flat(row) => row.top,
        }
    }

    fn height(&self) -> f32 {
        match self {
            ShowsListRow::Heading(row) => row.height,
            ShowsListRow::Series(row) => row.height,
            ShowsListRow::Season(row) => row.height,
            ShowsListRow::Episode(row) => row.height,
            ShowsListRow::Flat(row) => row.height,
        }
    }
}

/// The row key of a series node.
#[must_use]
pub fn series_row_id(series_id: &str) -> String {
    format!("shows-series:{series_id}")
}

/// The row key of a season node: the series is part of the key, so two series
/// that share a season id cannot collide.
#[must_use]
pub fn season_row_id(series_id: &str, season_id: &str) -> String {
    format!("shows-season:{series_id}:{season_id}")
}

/// The row key of an episode leaf.
#[must_use]
pub fn episode_row_id(episode_id: &str) -> String {
    format!("shows-episode:{episode_id}")
}

/// The painted rows of the Shows list (design D14): the Music tree over the
/// TUI's node kinds — each series node under the letter heading of its rail
/// letter, its seasons when it is open, and each season's episodes when that
/// season is open. The GUI Shows destination paints this list alone (design
/// D8).
///
/// The filter narrows the rows to the titles that contain its text, without
/// case: a row paints when its own title matches, and a node whose descendants
/// match stays so a match is never orphaned. Under a filter a node paints open
/// unless the user closed it (the Music tree's `closed` rule). Every row
/// carries its top edge and height, so the list paints a window of rows over
/// native scroll.
#[must_use]
pub fn build_shows_rows(
    view: GuiShowsView,
    series: &[GuiShowsSeriesFrame],
    rows: &[GuiShowsRowFrame],
    filter: &str,
    tree: ShowsTreeState<'_>,
) -> Vec<ShowsListRow> {
    match view {
        GuiShowsView::All => series_shows_rows(series, filter, tree),
        GuiShowsView::New | GuiShowsView::Upcoming => {
            // The filter's empty text keeps every row (`filter_match`'s own
            // rule), so no separate emptiness test rides here.
            let keeps = |row: &GuiShowsRowFrame| {
                filter_match(&row.title, filter)
                    || row
                        .subtitle
                        .as_deref()
                        .is_some_and(|text| filter_match(text, filter))
            };
            build_lettered_rows(
                rows,
                SHOWS_FLAT_ROW_HEIGHT,
                "shows-letter",
                keeps,
                |row| row.sort_letter,
                shows_flat_row,
            )
            .into_iter()
            .map(|row| match row {
                LetteredRow::Heading(heading) => ShowsListRow::Heading(heading),
                LetteredRow::Item(flat) => ShowsListRow::Flat(flat),
            })
            .collect()
        }
    }
}

/// A series node's painted row: the top-level row's height and art, its rail
/// letter, and how it opens.
fn series_row(
    node: &GuiShowsSeriesFrame,
    id: String,
    letter: char,
    open: bool,
    shows_children: bool,
    top: f32,
) -> ShowsListRow {
    ShowsListRow::Series(ShowsSeriesRow {
        id,
        source_id: node.id.clone(),
        title: node.title.clone(),
        letter,
        open,
        shows_children,
        expandable: node.expandable,
        top,
        height: SHOWS_SERIES_ROW_HEIGHT,
        art: node.art.clone(),
        media: node.media,
    })
}

/// The All series mode's tree rows.
fn series_shows_rows(
    series: &[GuiShowsSeriesFrame],
    filter: &str,
    tree: ShowsTreeState<'_>,
) -> Vec<ShowsListRow> {
    let needle = filter.trim().to_lowercase();
    let narrowing = !needle.is_empty();
    let matches = |title: &str| -> bool { !narrowing || title.to_lowercase().contains(&needle) };
    let mut built: Vec<ShowsListRow> = Vec::new();
    let mut top = 0.0_f32;
    let mut heading: Option<char> = None;
    for node in series {
        // The frame's rail letter is one `char`, so the empty-letter row the
        // TypeScript model guards arrives as `#` from the core.
        let letter = node.sort_letter;
        let node_id = series_row_id(&node.id);
        let seasons: Vec<(&GuiShowsSeasonFrame, Vec<&GuiShowsEpisodeRowFrame>, bool)> = node
            .seasons
            .iter()
            .map(|season| {
                let episodes = season.episodes.as_deref().unwrap_or_default();
                let kept: Vec<&GuiShowsEpisodeRowFrame> = episodes
                    .iter()
                    .filter(|episode| matches(&episode.name))
                    .collect();
                let kept_flag = matches(&season.name) || !kept.is_empty();
                (season, kept, kept_flag)
            })
            .collect();
        let kept_seasons: Vec<&(&GuiShowsSeasonFrame, Vec<&GuiShowsEpisodeRowFrame>, bool)> =
            seasons.iter().filter(|(_, _, kept)| *kept).collect();
        if !matches(&node.title) && kept_seasons.is_empty() {
            continue;
        }
        if heading != Some(letter) {
            heading = Some(letter);
            built.push(ShowsListRow::Heading(LetterHeading {
                id: format!("shows-letter:{letter}"),
                letter,
                top,
                height: SHOWS_HEADING_ROW_HEIGHT,
            }));
            top += SHOWS_HEADING_ROW_HEIGHT;
        }
        let series_open = if narrowing {
            !kept_seasons.is_empty() && !tree.closed.contains(&node_id)
        } else {
            tree.open.contains(&node_id)
        };
        built.push(series_row(
            node,
            node_id.clone(),
            letter,
            series_open,
            series_open && !kept_seasons.is_empty(),
            top,
        ));
        top += SHOWS_SERIES_ROW_HEIGHT;
        if !series_open {
            continue;
        }
        for (season, kept_episodes, kept) in kept_seasons {
            if !kept {
                continue;
            }
            let season_id = season_row_id(&node.id, &season.id);
            let open = if narrowing {
                !kept_episodes.is_empty() && !tree.closed.contains(&season_id)
            } else {
                tree.open.contains(&season_id)
            };
            built.push(ShowsListRow::Season(ShowsSeasonRow {
                id: season_id,
                source_id: season.id.clone(),
                series_id: node.id.clone(),
                title: season.name.clone(),
                letter,
                open,
                expandable: season.episodes.is_none() || !kept_episodes.is_empty(),
                loaded: season.episodes.is_some(),
                top,
                height: SHOWS_SEASON_ROW_HEIGHT,
            }));
            top += SHOWS_SEASON_ROW_HEIGHT;
            if !open {
                continue;
            }
            for episode in kept_episodes {
                built.push(ShowsListRow::Episode(ShowsEpisodeRow {
                    id: episode_row_id(&episode.episode_id),
                    source_id: episode.episode_id.clone(),
                    series_id: node.id.clone(),
                    title: episode.name.clone(),
                    number: episode.number,
                    duration: episode.duration.clone(),
                    letter,
                    top,
                    height: SHOWS_EPISODE_ROW_HEIGHT,
                }));
                top += SHOWS_EPISODE_ROW_HEIGHT;
            }
        }
    }
    built
}

/// The A–Z rail over the painted rows (design D7, D14; gui-flat-list-sort
/// design D8): each letter with the first series or flat row under it, so a
/// press scrolls there. A letter no painted row carries is disabled, as the
/// Music rail's is.
#[must_use]
pub fn shows_letter_rail(rows: &[ShowsListRow]) -> Vec<RailLetter> {
    letter_rail_by(rows, |row| match row {
        ShowsListRow::Series(series) => Some(series.letter),
        ShowsListRow::Flat(flat) => flat.letter,
        _ => None,
    })
}

/// One New or Upcoming row of the carried frame (gui-collection-dropdown design
/// D9; gui-flat-list-sort design D8): its two title lines, its gutter date, and
/// its rail letter.
fn shows_flat_row(row: &GuiShowsRowFrame, top: f32, height: f32) -> ShowsFlatRow {
    ShowsFlatRow {
        id: row.id.clone(),
        title: row.title.clone(),
        subtitle: row.subtitle.clone(),
        date: row.date.clone(),
        display_only: row.display_only,
        top,
        height,
        art: row.art.clone(),
        media: row.media,
        letter: row.sort_letter,
    }
}

#[cfg(test)]
mod tests;
