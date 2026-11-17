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

use std::sync::Arc;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiShowsEpisodeRowFrame, GuiShowsSeasonFrame, GuiShowsSeriesFrame,
};
use crate::window::library::list::{
    GeometryRow, LETTER_ROW_HEIGHT, NODE_ROW_HEIGHT, RailLetter, TRACK_ROW_HEIGHT, letter_rail_by,
};

/// The letter heading row's height in logical pixels: the Music list's letter
/// heading, which the All series list reuses.
pub const SHOWS_HEADING_ROW_HEIGHT: f32 = LETTER_ROW_HEIGHT;
/// The series and season row height in logical pixels: the Music list's node
/// row.
pub const SHOWS_SERIES_ROW_HEIGHT: f32 = NODE_ROW_HEIGHT;
/// The season row's height: the Music node row's, as the series row's is.
pub const SHOWS_SEASON_ROW_HEIGHT: f32 = NODE_ROW_HEIGHT;
/// The episode leaf's height in logical pixels: the Music track row.
pub const SHOWS_EPISODE_ROW_HEIGHT: f32 = TRACK_ROW_HEIGHT;

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

/// A painted heading row (structural: no key acts on it).
#[derive(Debug, Clone, PartialEq)]
pub struct ShowsHeadingRow {
    /// The row's stable key.
    pub id: String,
    /// The row's top edge from the list's first row.
    pub top: f32,
    /// The row's painted height.
    pub height: f32,
    /// The heading's rail letter.
    pub letter: char,
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
    /// Whether the node opens.
    pub expandable: bool,
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

/// One painted row of the Shows list (design D14). The `id` is the row's
/// stable key: the pointer marker's value and the each-block's key. A heading
/// is structural; the keys act on the other four kinds only.
#[derive(Debug, Clone, PartialEq)]
pub enum ShowsListRow {
    /// A letter heading above the first series of the letter.
    Heading(ShowsHeadingRow),
    /// A series node, open or closed.
    Series(ShowsSeriesRow),
    /// A season node under an open series, open or closed.
    Season(ShowsSeasonRow),
    /// An episode leaf under an open season.
    Episode(ShowsEpisodeRow),
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

    fn top(&self) -> f32 {
        match self {
            ShowsListRow::Heading(row) => row.top,
            ShowsListRow::Series(row) => row.top,
            ShowsListRow::Season(row) => row.top,
            ShowsListRow::Episode(row) => row.top,
        }
    }

    fn height(&self) -> f32 {
        match self {
            ShowsListRow::Heading(row) => row.height,
            ShowsListRow::Series(row) => row.height,
            ShowsListRow::Season(row) => row.height,
            ShowsListRow::Episode(row) => row.height,
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
    series: &[GuiShowsSeriesFrame],
    filter: &str,
    tree: ShowsTreeState<'_>,
) -> Vec<ShowsListRow> {
    series_shows_rows(series, filter, tree)
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
            built.push(ShowsListRow::Heading(ShowsHeadingRow {
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
        built.push(ShowsListRow::Series(ShowsSeriesRow {
            id: node_id.clone(),
            source_id: node.id.clone(),
            title: node.title.clone(),
            letter,
            open: series_open,
            expandable: node.expandable,
            top,
            height: SHOWS_SERIES_ROW_HEIGHT,
        }));
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

/// The A–Z rail over the painted rows (design D7, D14): each letter with the
/// first series row under it, so a press scrolls there. A letter no painted row
/// carries is disabled, as the Music rail's is.
#[must_use]
pub fn shows_letter_rail(rows: &[ShowsListRow]) -> Vec<RailLetter> {
    letter_rail_by(rows, |row| match row {
        ShowsListRow::Series(series) => Some(series.letter),
        _ => None,
    })
}

/// The rows the memo handed back last, with the cheap key and the interaction
/// facts they were built from. The rows sit behind an `Arc` because the former client
/// semantics the memo preserves are shared state: one scroll step reads the
/// same rows the last one built, and the list repaints only its visible
/// window.
#[derive(Debug, Clone)]
struct ShowsMemoEntry {
    /// The `libraryList` section revision the rows came from (design D6): a
    /// section's value at one revision is immutable, so equal revisions carry
    /// equal rows and the hit test never compares the union.
    revision: Option<u64>,
    filter: String,
    open: Vec<String>,
    closed: Vec<String>,
    rows: Arc<[ShowsListRow]>,
}

/// The Shows rows, memoized on the carried content, the filter, and the tree's
/// open and closed nodes (design D9). One slot, because a list paints one
/// content at a time.
#[derive(Clone, Default)]
pub struct ShowsRowsMemo {
    last: Signal<Option<ShowsMemoEntry>>,
}

impl std::fmt::Debug for ShowsRowsMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShowsRowsMemo").finish_non_exhaustive()
    }
}

impl ShowsRowsMemo {
    /// The rows `build_shows_rows` leaves over the carried content and the
    /// interaction facts: the ones it built last while the arguments are the
    /// same, and a fresh build otherwise. The rows come keyed on the
    /// `libraryList` section revision they borrowed from (design D6); the
    /// caller must pass the slices the section's current value carries.
    #[must_use]
    pub fn rows(
        &self,
        revision: Option<u64>,
        series: &[GuiShowsSeriesFrame],
        filter: &str,
        open: &[String],
        closed: &[String],
    ) -> Arc<[ShowsListRow]> {
        // `peek`, never `read`: a caller inside an effect that writes the
        // memo (a filter's scroll clamp) would re-queue itself on the write
        // below otherwise.
        if let Some(entry) = self.last.peek().as_ref()
            && entry.revision == revision
            && entry.filter == filter
            && entry.open == open
            && entry.closed == closed
        {
            return Arc::clone(&entry.rows);
        }
        let tree = ShowsTreeState { open, closed };
        let built: Arc<[ShowsListRow]> = series_shows_rows(series, filter, tree).into();
        let mut last = self.last;
        last.set(Some(ShowsMemoEntry {
            revision,
            filter: filter.to_owned(),
            open: open.to_vec(),
            closed: closed.to_vec(),
            rows: Arc::clone(&built),
        }));
        built
    }
}

#[cfg(test)]
mod tests;
