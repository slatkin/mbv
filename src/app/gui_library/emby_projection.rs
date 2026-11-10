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
use mbv_queue::QueueItem;
use mbv_render::effective_sort_str;
use mbv_ui_model::feed::FeedHomeVideoSelection;
use mbv_ui_model::ui_util::fmt_duration_short;

use super::GuiLibraryArtwork;
use super::home::{GuiArtShape, artwork_shape};
use super::projection::sort_letter;
use crate::app::App;

/// The feed-group pill a press selects (row 8.1): `New` (the destination
/// Latest view), `All`, or one root folder by its stable Service id. The
/// single-select rule (design D6) keeps one pill selected. `All` and `Folder`
/// write the library tab's shared feed state (the TUI's one store of that
/// fact); `New` is the GUI's own latest view over the destination-Latest
/// snapshot, whose store the shared snapshot event fills.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiEmbyFeedGroup {
    /// The New pill: the library's destination-Latest snapshot, newest first.
    Latest,
    /// The All pill: the library root's own listing, newest first.
    All,
    /// One root folder that holds items, by its stable Service id.
    Folder(String),
}

/// One pill of a feed-view library's pill row (row 8.1): single-select, the
/// selected one the feed state's current selection. The New pill's label is
/// the TUI's own (`crates/mbv-components/src/emby_library_content/
/// panel.rs`), so a user who knows one Client reads the other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiEmbyPill {
    /// The group the pill selects: the set-feed-group command carries it.
    pub group: GuiEmbyFeedGroup,
    /// The pill's label: `New`, `All`, or the folder's name.
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
    /// The library's display name: the list header's title, so the column
    /// names the library it holds as the TUI's panel header does.
    pub library_name: String,
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
        let projection = if self.is_feed_home_video_library(lib_idx) {
            self.gui_emby_feed_projection(lib_idx)
        } else {
            self.gui_emby_plain_projection(lib_idx)
        };
        self.warm_gui_emby_hero_neighbours(lib_idx, &projection);
        projection
    }

    /// Warm the Hero artwork of the rows neighbouring the picked one in the
    /// active list's order, through the shared navigation-idle gate (the TUI's
    /// cursor-neighbour rule, `App::fetch_nearby_movie_posters`, on the keys
    /// this Client's Hero paints instead of the list's). The pick's own art is
    /// the Hero's fetch, so the picked row is never warmed here.
    fn warm_gui_emby_hero_neighbours(&mut self, lib_idx: usize, projection: &GuiEmbyProjection) {
        let Some(picked) = projection.selected.as_deref() else {
            return;
        };
        let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
        let Some(picked_index) = ids.iter().position(|id| *id == picked) else {
            return;
        };
        self.warm_gui_hero_neighbours(&ids, picked_index, |app, id| {
            app.gui_emby_hero_source(lib_idx, id)
        });
    }

    /// The artwork source the Hero paints for `item_id`: the shared Emby hero
    /// producer's policy source, and `None` for a row the Hero gate excludes —
    /// a folder, or a non-`Movie` in a Movies library — or an id this App no
    /// longer holds.
    fn gui_emby_hero_source(&self, lib_idx: usize, item_id: &str) -> Option<ArtworkSource> {
        let item = self.gui_emby_item(lib_idx, item_id)?;
        if !self.gui_emby_hero_item(lib_idx, &item) {
            return None;
        }
        hero_content_emby(&item).facts.artwork.source
    }

    /// The library's display name (row 8.1): the list header's title.
    fn gui_emby_library_name(&self, lib_idx: usize) -> String {
        self.libs
            .get(lib_idx)
            .map(|lib| lib.library.name.clone())
            .unwrap_or_default()
    }

    /// The root listing's folders as `(id, name)` pairs (row 8.1): the
    /// programme lookup's source, the listing the pill row already holds.
    /// Empty while the listing has not landed.
    fn gui_root_folder_names(&self, lib_idx: usize) -> Vec<(String, String)> {
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.nav_stack.first())
            .map(|root| {
                root.items
                    .iter()
                    .map(|item| (item.id.clone(), item.name.clone()))
                    .collect()
            })
            .unwrap_or_default()
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
            .filter(|item| self.gui_emby_hero_item(lib_idx, item))
            .map(|item| self.gui_emby_hero(&item));
        GuiEmbyProjection {
            library_name: self.gui_emby_library_name(lib_idx),
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

    /// The feed-group mode's projection (row 8.1): the New, All, and folder
    /// pills over the counted folders, the selected pill's items in the
    /// server's date order with the gutter and no rail, and the selected
    /// row's Hero. The New pill's list is the destination-Latest store's
    /// snapshot; the All and folder pills' lists are the feed state's.
    fn gui_emby_feed_projection(&mut self, lib_idx: usize) -> GuiEmbyProjection {
        if self
            .gui_emby_libs
            .get(&lib_idx)
            .is_some_and(|selection| selection.feed_latest)
        {
            self.gui_emby_latest_projection(lib_idx)
        } else {
            self.gui_emby_group_projection(lib_idx)
        }
    }

    /// The New pill's projection (row 8.1): the destination-Latest store's
    /// snapshot — the TUI New pill's own list (`latest_items`, the TUI's
    /// `EmbyLibraryContent` push) — with the feed rows' programme line and
    /// the selected row's Hero. The store's loading mark is the list's
    /// loading state; a failed fetch paints its empty list, as the TUI's New
    /// pill does.
    fn gui_emby_latest_projection(&mut self, lib_idx: usize) -> GuiEmbyProjection {
        let (loading, rows) = {
            let folders = self.gui_root_folder_names(lib_idx);
            match self.gui_emby_latest.get(&lib_idx) {
                Some(super::GuiEmbyLatestItems::Loaded(items)) => {
                    let rows = items
                        .iter()
                        .map(|item| {
                            feed_row(item, latest_row_date(item), programme_of(item, &folders))
                        })
                        .collect();
                    (false, rows)
                }
                Some(super::GuiEmbyLatestItems::Failed) => (false, Vec::new()),
                Some(super::GuiEmbyLatestItems::Loading) | None => (true, Vec::new()),
            }
        };
        let selected = self.gui_emby_selected(lib_idx, &rows);
        let hero = selected
            .as_ref()
            .and_then(|selected| self.gui_emby_item(lib_idx, selected))
            .filter(|item| self.gui_emby_hero_item(lib_idx, item))
            .map(|item| self.gui_emby_hero(&item));
        GuiEmbyProjection {
            library_name: self.gui_emby_library_name(lib_idx),
            pills: self.gui_emby_feed_pills(lib_idx),
            collection_names: vec!["New".to_string()],
            feed_group: true,
            loading,
            failed: false,
            rows,
            selected,
            hero,
        }
    }

    /// The All and folder pills' projection (row 8.1): the selected group's
    /// items in the server's date order with the gutter and no rail, and the
    /// selected row's Hero.
    fn gui_emby_group_projection(&mut self, lib_idx: usize) -> GuiEmbyProjection {
        let selection = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .map(|state| state.selection.clone())
            .unwrap_or_default();
        let pills = self.gui_emby_feed_pills(lib_idx);
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
        let rows: Vec<GuiEmbyRow> = {
            let folders = self.gui_root_folder_names(lib_idx);
            self.feed_home_video_selected_items(lib_idx)
                .iter()
                .map(|item| feed_row(item, feed_row_date(item), programme_of(item, &folders)))
                .collect()
        };
        let selected = self.gui_emby_selected(lib_idx, &rows);
        let hero = selected
            .as_ref()
            .and_then(|selected| self.gui_emby_item(lib_idx, selected))
            .filter(|item| self.gui_emby_hero_item(lib_idx, item))
            .map(|item| self.gui_emby_hero(&item));
        GuiEmbyProjection {
            library_name: self.gui_emby_library_name(lib_idx),
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

    /// The feed-group mode's pill row (row 8.1): New first, then All, then
    /// each root folder whose video count is positive, in listing order — the
    /// TUI's pill row's membership rule (`EmbyLibraryContent`'s selector
    /// row). The New pill is selected while the GUI's own Latest view is
    /// active; otherwise the selected pill is the feed state's current
    /// selection, and a folder the counts no longer admit falls back to All,
    /// as the TUI's pill index clamps there.
    fn gui_emby_feed_pills(&self, lib_idx: usize) -> Vec<GuiEmbyPill> {
        let latest = self
            .gui_emby_libs
            .get(&lib_idx)
            .is_some_and(|selection| selection.feed_latest);
        let selection = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .map(|state| state.selection.clone())
            .unwrap_or_default();
        let mut pills = vec![
            GuiEmbyPill {
                group: GuiEmbyFeedGroup::Latest,
                name: "New".to_string(),
                selected: latest,
            },
            GuiEmbyPill {
                group: GuiEmbyFeedGroup::All,
                name: "All".to_string(),
                selected: !latest && selection == FeedHomeVideoSelection::All,
            },
        ];
        pills.extend(self.counted_folder_iter(lib_idx).map(|folder| GuiEmbyPill {
            group: GuiEmbyFeedGroup::Folder(folder.id.clone()),
            name: folder.name.clone(),
            selected: matches!(&selection, FeedHomeVideoSelection::Folder(id) if folder.id == *id),
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
    /// New pill's Latest items, the feed state's selected group's items in
    /// the feed-group mode, the root-list store's rows in plain mode. `None`
    /// when no row carries the id, so a stale command plays nothing.
    pub(in crate::app::gui_library) fn gui_emby_item(
        &self,
        lib_idx: usize,
        item_id: &str,
    ) -> Option<EmbyItem> {
        if self.is_feed_home_video_library(lib_idx) {
            if self
                .gui_emby_libs
                .get(&lib_idx)
                .is_some_and(|selection| selection.feed_latest)
            {
                return self
                    .gui_stored_emby_latest_items(lib_idx)
                    .find(|item| item.id == item_id)
                    .cloned();
            }
            return self
                .feed_home_video_selected_items(lib_idx)
                .into_iter()
                .find(|item| item.id == item_id);
        }
        self.gui_stored_emby_root_items(lib_idx)
            .find(|item| item.id == item_id)
            .cloned()
    }

    /// Whether the selected row paints a Hero (row 8.1): the TUI's own gate
    /// (`hero_item`, `crates/mbv-components/src/emby_library_content.rs`). A
    /// folder paints none, because the Hero describes an item and a folder is
    /// a container; a Movies library additionally admits only a `Movie`, so a
    /// `BoxSet` row — a collection — paints no Hero either.
    fn gui_emby_hero_item(&self, lib_idx: usize, item: &EmbyItem) -> bool {
        if item.is_folder {
            return false;
        }
        let movies = self
            .libs
            .get(lib_idx)
            .is_some_and(|lib| lib.library.collection_type == "movies");
        !movies || item.item_type == "Movie"
    }

    /// The selected row's Hero facts (row 8.1): the shared Emby hero
    /// producer's facts, with the artwork fetched through the image cache at
    /// the artwork policy's declared chain — the same facts and fetch the
    /// TUI's hero paints. No Workspace rides a plain Emby-library Hero.
    fn gui_emby_hero(&mut self, item: &EmbyItem) -> GuiEmbyHero {
        let content = hero_content_emby(item);
        let art = self.gui_emby_hero_art(content.facts.artwork.source.as_ref());
        GuiEmbyHero {
            title: content.facts.title.clone(),
            meta_rows: content.facts.meta_rows.clone(),
            duration_row: content.facts.duration_row,
            progress_row: content.facts.progress_row,
            overview: content.overview.clone(),
            art_shape: artwork_shape(
                self.gui_hero_shape(content.facts.artwork.shape, art.as_ref()),
            ),
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

/// One row of the feed-group lists (row 8.1): the TUI row's primary label,
/// the programme (root folder) the item sits in underneath it — the user
/// rule of 2026-12-30, so a flat All row names the programme it belongs to
/// — the gutter date (the New list's date added, the All and folder lists'
/// production year), and no rail letter.
fn feed_row(item: &EmbyItem, date: Option<String>, programme: Option<String>) -> GuiEmbyRow {
    GuiEmbyRow {
        id: item.id.clone(),
        title: row_title(item),
        subtitle: programme,
        date,
        sort_letter: None,
        kind: row_kind(item),
    }
}

/// The All and folder lists' gutter date (row 8.1): the production year, as
/// the TUI's feed-group rows paint it.
fn feed_row_date(item: &EmbyItem) -> Option<String> {
    (item.production_year > 0).then(|| item.production_year.to_string())
}

/// The New list's gutter date (row 8.1): the provider date added, short —
/// the TUI's Latest rows' own gutter (`latest_row_projection`,
/// `crates/mbv-components/src/emby_library_content.rs`).
fn latest_row_date(item: &EmbyItem) -> Option<String> {
    let item = QueueItem::Emby(Box::new(item.clone()));
    mbv_ui_model::home_latest::provider_timestamp_secs(&item)
        .map(mbv_ui_model::ui_util::fmt_publish_date_short)
        .filter(|date| !date.is_empty())
}

/// The programme (root folder) an item sits in (row 8.1): the root listing's
/// folder whose id is the item's containing item's id. `None` for a loose
/// root video — whose containing item is the library itself — and for an
/// item whose folder the listing does not hold.
fn programme_of(item: &EmbyItem, folders: &[(String, String)]) -> Option<String> {
    folders
        .iter()
        .find(|(folder_id, _)| *folder_id == item.parent_id)
        .map(|(_, name)| name.clone())
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
