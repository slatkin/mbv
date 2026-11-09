//! The GUI Library's plain Emby-library projection (add-gui-library-panel
//! design D12, the USER-APPROVED GUI Emby-library design, row 8.1): a plain
//! library's flat name-sorted list with its rail letters and no pills, a
//! feed-view library's pill row of All and each folder with items with its
//! date-ordered list and no rail, the core's selected row, and the selected
//! row's Hero — the shared Emby hero producer's facts with no Workspace. The
//! row and label rules are the TUI's own (`row_for` and the All view's
//! gutter, `crates/mbv-components/src/emby_library_content.rs`), so the two
//! Clients cannot drift, and the projection reads `App` data only.

use mbv_components::library_panel::content::ArtworkSource;
use mbv_components::library_panel::hero::hero_content_emby;
use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_render::effective_sort_str;
use mbv_ui_model::feed::FeedHomeVideoSelection;
use mbv_ui_model::ui_util::fmt_duration_short;

use super::GuiLibraryArtwork;
use super::home::{GuiArtShape, artwork_shape};
use super::projection::sort_letter;
use crate::app::App;

/// The feed-group pill a press selects (row 8.1): `All`, or one root folder
/// by its stable Service id. The single-select rule (design D6) keeps one
/// pill selected, and the selection's one store is the library tab's own
/// feed state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiEmbyFeedGroup {
    /// The All pill: the library root's own listing, newest first.
    All,
    /// One root folder that holds items, by its stable Service id.
    Folder(String),
}

impl From<GuiEmbyFeedGroup> for FeedHomeVideoSelection {
    fn from(group: GuiEmbyFeedGroup) -> Self {
        match group {
            GuiEmbyFeedGroup::All => Self::All,
            GuiEmbyFeedGroup::Folder(folder_id) => Self::Folder(folder_id),
        }
    }
}

/// One pill of a feed-view library's pill row (row 8.1): single-select, the
/// selected one the feed state's current selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiEmbyPill {
    /// The group the pill selects: the set-feed-group command carries it.
    pub group: GuiEmbyFeedGroup,
    /// The pill's label: `All`, or the folder's name.
    pub name: String,
    /// Whether the feed state's selection is this pill's group.
    pub selected: bool,
}

/// One row's kind (row 8.1): `Collection` when the row is a folder or a
/// boxset — the container the folder-play rule expands — and `Media` on a
/// playable leaf, mirroring the Home rows' shared vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiEmbyRowKind {
    Collection,
    Media,
}

/// One row of the plain Emby-library list (row 8.1). The identity is the
/// stable Service id a select or play command sends, never a row position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiEmbyRow {
    /// The row's stable identity.
    pub id: String,
    /// The row's primary label: the TUI browse row's primary, with a
    /// folder's item-count decoration (`Name · N items`, `Name [count]`).
    pub title: String,
    /// The row's secondary label (plain mode): the production year, else the
    /// runtime. `None` in feed-group mode, whose row label rides the gutter.
    pub subtitle: Option<String>,
    /// The gutter date (feed-group mode): the TUI row's trailing gutter, the
    /// production year. `None` in plain mode, whose year rides `subtitle`.
    pub date: Option<String>,
    /// The row's rail letter (design D7): the first letter of the
    /// article-stripped sort name, or `#`. Set only in plain mode; the
    /// date-ordered feed list shows no rail.
    pub sort_letter: Option<char>,
    /// The row's kind.
    pub kind: GuiEmbyRowKind,
}

/// The Hero's facts for the selected row (row 8.1): the shared Emby hero
/// producer's facts for the item — the same facts the TUI paints — and the
/// artwork at the artwork policy's chain. A plain Emby-library Hero has no
/// Workspace, so none rides here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiEmbyHero {
    /// The hero's title.
    pub title: String,
    /// The meta rows in display order.
    pub meta_rows: Vec<String>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role, when one does. The shared
    /// Emby hero producer fills none, so an Emby-library hero carries `None`
    /// exactly as the TUI's does.
    pub progress_row: Option<usize>,
    /// The cleaned overview text, when the item's payload carries one.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves: the artwork policy's declared
    /// shape (`emby_artwork_policy`).
    pub art_shape: GuiArtShape,
    /// The Hero's artwork, fetched through the image cache at the artwork
    /// policy's chain. `None` when the item declares no image source; the
    /// window content paints its own placeholder there.
    pub art: Option<GuiLibraryArtwork>,
}

/// The plain Emby-library projection the GUI Library panel paints (row 8.1):
/// the feed pills in the feed-group mode, the active list's rows, the core's
/// selected row, and its Hero.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiEmbyProjection {
    /// The feed-group mode's pills in pill order — All, then each counted
    /// folder in listing order. Empty in plain mode, whose rail replaces the
    /// TUI's letter-range pill row.
    pub pills: Vec<GuiEmbyPill>,
    /// The selected collection's name the Library Status bar's location
    /// reads (design D10), as the Shows projection carries its mode's name:
    /// the selection's pill name in the feed-group mode, and nothing in
    /// plain mode, whose list is the whole library.
    pub collection_names: Vec<String>,
    /// Whether the library is a feed-view library in the feed-group mode
    /// (`feed_view_libraries`): the mode the pills, the row order, and the
    /// rail follow.
    pub feed_group: bool,
    /// Whether the active list has not landed yet: the root-list store's
    /// fetch in plain mode, the feed state's fetch or the root listing in
    /// the feed-group mode.
    pub loading: bool,
    /// Whether the plain mode's root-list fetch failed (row 8.1). The
    /// feed-group mode's fetches report through the feed state's own
    /// loading, so only the plain mode's store names a failure here.
    pub failed: bool,
    /// The active list's rows in the list's order: the store's name-sorted
    /// root list in plain mode, the selected group's items in the server's
    /// date order in the feed-group mode.
    pub rows: Vec<GuiEmbyRow>,
    /// The core's selected row (design D2, row 8.1): the row the select
    /// command last named while the list still carries it, else the first
    /// row — the Hero default, so the Hero paints as soon as a list lands.
    /// `None` while the list holds no row.
    pub selected: Option<String>,
    /// The selected row's Hero facts; `None` while no row is selected.
    pub hero: Option<GuiEmbyHero>,
}

impl App {
    /// The plain Emby-library projection the GUI Library panel paints (row
    /// 8.1): the feed pills, the active list's rows, the selected row, and
    /// its Hero. A `lib_idx` that names no plain Emby library projects the
    /// empty projection, as the frame's gate excludes that destination
    /// anyway.
    #[must_use]
    pub fn gui_emby_projection(&mut self, lib_idx: usize) -> GuiEmbyProjection {
        if !self.gui_emby_library(lib_idx) {
            return GuiEmbyProjection::default();
        }
        let feed_group = self.is_feed_home_video_library(lib_idx);
        if feed_group {
            self.gui_emby_feed_projection(lib_idx)
        } else {
            self.gui_emby_plain_projection(lib_idx)
        }
    }

    /// The plain mode's projection (row 8.1): the root-list store's rows in
    /// the server's `SortName` order, each with its rail letter and no pill,
    /// and the selected row's Hero.
    fn gui_emby_plain_projection(&mut self, lib_idx: usize) -> GuiEmbyProjection {
        let (items, loading, failed) = match self.gui_emby_roots.get(&lib_idx) {
            Some(super::GuiEmbyRootItems::Loaded(items)) => (items.as_slice(), false, false),
            Some(super::GuiEmbyRootItems::Loading) | None => (&[][..], true, false),
            Some(super::GuiEmbyRootItems::Failed) => (&[][..], false, true),
        };
        let rows: Vec<GuiEmbyRow> = items.iter().map(plain_row).collect();
        let selected = self.gui_emby_selected(lib_idx, &rows);
        let hero = selected
            .as_ref()
            .and_then(|selected| self.gui_emby_item(lib_idx, selected))
            .map(|item| self.gui_emby_hero(&item));
        GuiEmbyProjection {
            pills: Vec::new(),
            collection_names: Vec::new(),
            feed_group: false,
            loading,
            failed,
            rows,
            selected,
            hero,
        }
    }

    /// The feed-group mode's projection (row 8.1): the All and folder pills
    /// over the counted folders, the selected group's items in the server's
    /// date order with the gutter and no rail, and the selected row's Hero.
    fn gui_emby_feed_projection(&mut self, lib_idx: usize) -> GuiEmbyProjection {
        let selection = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .map(|state| state.selection.clone())
            .unwrap_or_default();
        let pills = self.gui_emby_feed_pills(lib_idx, &selection);
        let collection_names = vec![selection_name(&pills, &selection)];
        let root_loading = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.nav_stack.first())
            .is_none_or(|root| root.loading);
        let loading = root_loading
            || self
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.feed_home_video.as_ref())
                .is_some_and(|state| state.loading);
        let rows: Vec<GuiEmbyRow> = self
            .feed_home_video_selected_items(lib_idx)
            .iter()
            .map(feed_row)
            .collect();
        let selected = self.gui_emby_selected(lib_idx, &rows);
        let hero = selected
            .as_ref()
            .and_then(|selected| self.gui_emby_item(lib_idx, selected))
            .map(|item| self.gui_emby_hero(&item));
        GuiEmbyProjection {
            pills,
            collection_names,
            feed_group: true,
            loading,
            failed: false,
            rows,
            selected,
            hero,
        }
    }

    /// The feed-group mode's pill row (row 8.1): All first, then each root
    /// folder whose video count is positive, in listing order — the TUI's
    /// pill row's membership rule. The selected pill is the feed state's
    /// current selection; a folder the counts no longer admit falls back to
    /// All, as the TUI's pill index clamps there.
    fn gui_emby_feed_pills(
        &self,
        lib_idx: usize,
        selection: &FeedHomeVideoSelection,
    ) -> Vec<GuiEmbyPill> {
        let mut pills = vec![GuiEmbyPill {
            group: GuiEmbyFeedGroup::All,
            name: "All".to_string(),
            selected: *selection == FeedHomeVideoSelection::All,
        }];
        pills.extend(self.counted_folder_iter(lib_idx).map(|folder| GuiEmbyPill {
            group: GuiEmbyFeedGroup::Folder(folder.id.clone()),
            name: folder.name.clone(),
            selected: matches!(selection, FeedHomeVideoSelection::Folder(id) if &folder.id == id),
        }));
        pills
    }

    /// The core's selected row (design D2, row 8.1): the stored pick while
    /// the list still carries it, else the first row — the Hero default.
    fn gui_emby_selected(&self, lib_idx: usize, rows: &[GuiEmbyRow]) -> Option<String> {
        let stored = self
            .gui_emby_libs
            .get(&lib_idx)
            .and_then(|selection| selection.picked_row.clone());
        stored
            .filter(|stored| rows.iter().any(|row| &row.id == stored))
            .or_else(|| rows.first().map(|row| row.id.clone()))
    }

    /// The Emby item a command names (row 8.1): the active list's rows — the
    /// feed state's selected group's items in the feed-group mode, the
    /// root-list store's rows in plain mode. `None` when no row carries the
    /// id, so a stale command plays nothing.
    pub(in crate::app::gui_library) fn gui_emby_item(
        &self,
        lib_idx: usize,
        item_id: &str,
    ) -> Option<EmbyItem> {
        if self.is_feed_home_video_library(lib_idx) {
            return self
                .feed_home_video_selected_items(lib_idx)
                .into_iter()
                .find(|item| item.id == item_id);
        }
        self.gui_stored_emby_root_items(lib_idx)
            .find(|item| item.id == item_id)
            .cloned()
    }

    /// The selected row's Hero facts (row 8.1): the shared Emby hero
    /// producer's facts, with the artwork fetched through the image cache at
    /// the artwork policy's declared chain — the same facts and fetch the
    /// TUI's hero paints. No Workspace rides a plain Emby-library Hero.
    fn gui_emby_hero(&mut self, item: &EmbyItem) -> GuiEmbyHero {
        let content = hero_content_emby(item);
        let art = match &content.facts.artwork.source {
            Some(ArtworkSource::Emby {
                item_id,
                series_id,
                image_types,
                cache_key,
            }) => Some(self.gui_home_hero_artwork(cache_key, item_id, series_id, image_types)),
            Some(_) | None => None,
        };
        GuiEmbyHero {
            title: content.facts.title.clone(),
            meta_rows: content.facts.meta_rows.clone(),
            duration_row: content.facts.duration_row,
            progress_row: content.facts.progress_row,
            overview: content.overview.clone(),
            art_shape: artwork_shape(content.facts.artwork.shape),
            art,
        }
    }
}

/// One row of the plain list (row 8.1): the TUI browse row's primary label
/// and gutter facts, with the year on `subtitle` — the plain list's rail
/// replaces the TUI's letter-range pill row, and the row carries no gutter
/// date.
fn plain_row(item: &EmbyItem) -> GuiEmbyRow {
    GuiEmbyRow {
        id: item.id.clone(),
        title: row_title(item),
        subtitle: row_secondary(item),
        date: None,
        sort_letter: Some(sort_letter(effective_sort_str(item))),
        kind: row_kind(item),
    }
}

/// One row of the feed-group list (row 8.1): the TUI row's primary label
/// with the year in the gutter — the date-ordered list's gutter — and no
/// rail letter.
fn feed_row(item: &EmbyItem) -> GuiEmbyRow {
    let year = (item.production_year > 0).then(|| item.production_year.to_string());
    GuiEmbyRow {
        id: item.id.clone(),
        title: row_title(item),
        subtitle: None,
        date: year,
        sort_letter: None,
        kind: row_kind(item),
    }
}

/// The TUI browse row's primary label (`row_for`, `crates/mbv-components/
/// src/emby_library_content.rs`): a root folder's item-count decorations,
/// else the item's display name.
fn row_title(item: &EmbyItem) -> String {
    if item.is_folder && item.item_type == "Folder" && item.total_count > 0 {
        format!("{} · {} items", item.display_name(), item.total_count)
    } else if item.is_folder && item.unplayed_item_count > 0 && item.item_type != "Series" {
        format!("{} [{}]", item.display_name(), item.unplayed_item_count)
    } else {
        item.display_name()
    }
}

/// The row's secondary label (row 8.1): the production year, else the
/// runtime, as the plain list's row facts.
fn row_secondary(item: &EmbyItem) -> Option<String> {
    if item.production_year > 0 {
        return Some(item.production_year.to_string());
    }
    (item.runtime_ticks > 0).then(|| fmt_duration_short(item.runtime_ticks / TICKS_PER_SECOND))
}

/// The row's kind (row 8.1): a folder or a boxset is the container the
/// folder-play rule expands.
fn row_kind(item: &EmbyItem) -> GuiEmbyRowKind {
    if item.is_folder || item.item_type == "BoxSet" {
        GuiEmbyRowKind::Collection
    } else {
        GuiEmbyRowKind::Media
    }
}

/// The selected pill's name (row 8.1), the Library Status bar's location's
/// collection name: the selection's pill's label, with a folder the pill row
/// no longer carries falling back to All — the TUI pill index's clamp.
fn selection_name(pills: &[GuiEmbyPill], selection: &FeedHomeVideoSelection) -> String {
    pills.iter().find(|pill| pill.selected).map_or_else(
        || match selection {
            FeedHomeVideoSelection::All => "All".to_string(),
            FeedHomeVideoSelection::Folder(id) => id.clone(),
        },
        |pill| pill.name.clone(),
    )
}
