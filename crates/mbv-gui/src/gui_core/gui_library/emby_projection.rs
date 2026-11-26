//! The GUI Library's plain Emby-library projection (add-gui-library-panel
//! design D12, the USER-APPROVED GUI Emby-library design, row 8.1): a plain
//! library's flat list — the server's name order under the Title sort, else
//! the date order — with its rail letters under a name sort and no pills, a
//! feed-view library's interim pill row of All and each folder with the
//! checked folders' merged list in the library's list sort, the core's
//! selected row, and the selected row's Hero — the shared Emby hero producer's
//! facts with no Workspace. The row and label rules are the TUI's own
//! (`row_for` and the All view's gutter,
//! `crates/mbv-components/src/emby_library_content.rs`), so the two Clients
//! cannot drift, and the projection reads `App` data only. Each row carries
//! the shared play state the one Emby derivation names
//! (`MediaSemanticState::from_emby`, gui-emby-row-played-marks design D1):
//! collections included, and music ordinary through the derivation's music
//! exception.

use std::collections::HashMap;
use std::collections::HashSet;

use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_ui_model::feed::FeedHomeVideoTarget;
use mbv_ui_model::home_latest::emby_date_added_secs;
use mbv_ui_model::library_hero::ArtworkSource;
use mbv_ui_model::library_hero::producers::hero_content_emby;
use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::ui_util::fmt_duration_short;

use super::GuiLibraryArtwork;
use super::GuiListSort;
use super::GuiListSortFrame;
use super::GuiSortTarget;
use super::artwork::{GuiRowArt, GuiRowMediaKind, emby_media_kind, emby_row_source, row_art};
use super::collection::{GuiCollectionSet, GuiCollectionTarget, GuiCollections};
use super::hero_facts::{GuiHeroCastMember, GuiHeroFacts, hero_cast_emby, hero_facts_emby};
use super::home::{GuiArtShape, artwork_shape};
use super::list_sort::gui_title_sort_key;
use crate::gui_core::GuiCore;
use crate::gui_core::gui_client_state::GuiClientState;
use mbv_client_core::App;
use mbv_ui_model::sort_filter::effective_sort_str;

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
/// The row carries the shared play state (gui-emby-row-played-marks design
/// D1): the one derivation the TUI's rows use
/// (`MediaSemanticState::from_emby`), collections included, and music
/// ordinary through `from_emby`'s music exception.
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
    /// The row's rail letter (design D6): the first letter of the
    /// article-stripped sort name, or `#`. Set exactly while the list's sort
    /// is a name sort, so a date-ordered list shows no rail.
    pub sort_letter: Option<char>,
    /// The row's kind.
    pub kind: GuiEmbyRowKind,
    /// The row's art (gui-delineated-rows design D4).
    pub art: GuiRowArt,
    /// The media kind the row shows when it has no image (design D4).
    pub media: GuiRowMediaKind,
    /// The row's shared play state (gui-emby-row-played-marks design D1):
    /// the one derivation the TUI's rows use, collections included, music
    /// ordinary through the derivation's exception.
    pub state: MediaSemanticState,
}

/// The Hero's facts for the selected row (row 8.1): the item's facts as the
/// D1 builders split them — header pills and labelled rows — the movie's
/// cast, and the artwork at the artwork policy's chain. A plain Emby-library
/// Hero has no Workspace, so none rides here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiEmbyHero {
    /// The item's kind word for the Hero's kicker (design D17): the item
    /// type's lowercase word — `movie`, `video` — the kind's own word, so a
    /// library of mixed kinds names each item's.
    pub kind: String,
    /// The hero's title.
    pub title: String,
    /// The item's facts (gui-hero-design-polish design D1): the header
    /// band's pills and the detail block's labelled rows, each in paint
    /// order.
    pub facts: GuiHeroFacts,
    /// The cleaned overview text, when the item's payload carries one.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves: the artwork policy's declared
    /// shape (`emby_artwork_policy`).
    pub art_shape: GuiArtShape,
    /// The Hero's artwork, fetched through the image cache at the artwork
    /// policy's chain. `None` when the item declares no image source; the
    /// window content paints its own placeholder there.
    pub art: Option<GuiLibraryArtwork>,
    /// The movie's cast (gui-hero-design-polish design D3): the people that
    /// are not directors, in the order Emby gives. Empty when the item is
    /// not a movie or carries no people.
    pub cast: Vec<GuiHeroCastMember>,
}

/// The plain Emby-library projection the GUI Library panel paints (row 8.1):
/// the feed pills in the feed-group mode, the active list's rows, the core's
/// selected row, and its Hero.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiEmbyProjection {
    /// The library's display name: the list header's title, so the column
    /// names the library it holds as the TUI's panel header does.
    pub library_name: String,
    /// The destination's collection selector (gui-collection-dropdown design
    /// D6): `Some` in the feed-group mode, with All, every root folder in
    /// listing order with its checked flag, the Folders noun, and the list's
    /// date order; `None` in plain mode, whose rail replaces the collection
    /// selector.
    pub collections: Option<GuiCollections>,
    /// The selected collection's name the Library Status bar's location
    /// reads (design D10), as the Shows projection carries its mode's name:
    /// the selection's name in the feed-group mode, and nothing in plain
    /// mode, whose list is the whole library.
    pub collection_names: Vec<String>,
    /// Whether the library is a feed-view library in the feed-group mode
    /// (`feed_view_libraries`): the mode the selector, the row order, and the
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
    /// The active list's rows in the list's order: the store's root list in
    /// plain mode — the server's `SortName` order under the Title sort, else
    /// the date order — and the selected group's items under the feed-group
    /// mode's sort.
    pub rows: Vec<GuiEmbyRow>,
    /// The active list's sort control (design D7): the target a press names,
    /// its active sort, and the choices the library offers. The plain mode
    /// and the feed-group mode share it.
    pub sort: Option<GuiListSortFrame>,
    /// The core's selected row (design D2, row 8.1): the row the select
    /// command last named while the list still carries it, else the first
    /// row — the Hero default, so the Hero paints as soon as a list lands.
    /// `None` while the list holds no row.
    pub selected: Option<String>,
    /// The selected row's Hero facts; `None` while no row is selected.
    pub hero: Option<GuiEmbyHero>,
}

impl GuiCore {
    /// The plain Emby-library projection the GUI Library panel paints (row
    /// 8.1): the feed pills, the active list's rows, the selected row, and
    /// its Hero. A `lib_idx` that names no plain Emby library projects the
    /// empty projection, as the frame's gate excludes that destination
    /// anyway.
    #[must_use]
    pub fn gui_emby_projection(&mut self, lib_idx: usize) -> GuiEmbyProjection {
        if !super::emby::emby_library(&self.app, lib_idx) {
            return GuiEmbyProjection::default();
        }
        let projection = if self.app.is_feed_home_video_library(lib_idx) {
            self.emby_feed_projection(lib_idx)
        } else {
            self.emby_plain_projection(lib_idx)
        };
        self.warm_emby_hero_neighbours(lib_idx, &projection);
        projection
    }

    /// Warm the Hero artwork of the rows neighbouring the picked one in the
    /// active list's order, through the shared navigation-idle gate (the TUI's
    /// cursor-neighbour rule, `App::fetch_nearby_movie_posters`, on the keys
    /// this Client's Hero paints instead of the list's). The pick's own art is
    /// the Hero's fetch, so the picked row is never warmed here.
    fn warm_emby_hero_neighbours(&mut self, lib_idx: usize, projection: &GuiEmbyProjection) {
        let Some(picked) = projection.selected.as_deref() else {
            return;
        };
        let ids: Vec<&str> = projection.rows.iter().map(|row| row.id.as_str()).collect();
        let Some(picked_index) = ids.iter().position(|id| *id == picked) else {
            return;
        };
        let gui = &self.gui;
        super::artwork::warm_hero_neighbours(&mut self.app, &ids, picked_index, |app, id| {
            emby_hero_source(app, gui, lib_idx, id)
        });
    }

    /// The plain mode's projection (row 8.1): the root-list store's rows —
    /// the server's `SortName` order under the Title sort, else the date
    /// order — each with its rail letter only while the sort is a name sort,
    /// and no pill, and the selected row's Hero.
    fn emby_plain_projection(&mut self, lib_idx: usize) -> GuiEmbyProjection {
        let app = &self.app;
        let gui = &self.gui;
        let (items, loading, failed) = match gui.emby_roots.get(&lib_idx) {
            Some(super::GuiEmbyRootItems::Loaded(items)) => (items.as_slice(), false, false),
            Some(super::GuiEmbyRootItems::Loading) | None => (&[][..], true, false),
            Some(super::GuiEmbyRootItems::Failed) => (&[][..], false, true),
        };
        let sort_frame =
            super::list_sort::list_sort_frame(app, gui, GuiSortTarget::Library(lib_idx));
        let sort = sort_frame.active;
        let mut items: Vec<&EmbyItem> = items.iter().collect();
        // Under the Title sort the store's own `SortName` order stands
        // (design D6); the date sorts order the same items here, so no
        // fetch backs a sort change.
        if !sort.is_name_sort() {
            items.sort_by(|left, right| {
                sort.compare_date(emby_date_added_secs(left), emby_date_added_secs(right))
                    .then_with(|| left.id.cmp(&right.id))
            });
        }
        let rows: Vec<GuiEmbyRow> = items
            .into_iter()
            .map(|item| plain_row(app, item, sort.rail_letter(effective_sort_str(item))))
            .collect();
        let selected = self.emby_selected(lib_idx, &rows);
        let hero = self.emby_hero_for(lib_idx, selected.as_deref());
        GuiEmbyProjection {
            library_name: self.emby_library_name(lib_idx),
            collections: None,
            collection_names: Vec::new(),
            feed_group: false,
            loading,
            failed,
            rows,
            sort: Some(sort_frame),
            selected,
            hero,
        }
    }

    /// The feed-group mode's projection (row 8.1; gui-collection-dropdown
    /// design D2, D3, D4): the All-and-folders pill row over the GUI's
    /// effective set, the checked folders' merged items in the stored date
    /// order with the gutter and no rail, and the selected row's Hero. The
    /// effective set is the frame's one read — the items and the pill names
    /// read the set it borrows — and the programme lookup's id-to-name map is
    /// built once, so a row's programme is one hash probe.
    fn emby_feed_projection(&mut self, lib_idx: usize) -> GuiEmbyProjection {
        let set = super::emby::emby_feed_effective_set(&self.app, &self.gui, lib_idx);
        let folders = root_folder_names(&self.app, lib_idx);
        let collections = super::collection_command::collections(
            &self.app,
            &self.gui,
            GuiCollectionTarget::EmbyFeed(lib_idx),
        );
        let collection_names = feed_collection_names(set.as_ref(), &folders);
        let programmes: HashMap<&str, &str> =
            folders.iter().map(|(id, name)| (*id, *name)).collect();
        let sort_frame = super::list_sort::list_sort_frame(
            &self.app,
            &self.gui,
            GuiSortTarget::Library(lib_idx),
        );
        let sort = sort_frame.active;
        let (items, loading) = feed_items(&self.app, lib_idx, set.as_ref(), sort);
        let app = &self.app;
        let rows: Vec<GuiEmbyRow> = items
            .iter()
            .map(|item| {
                feed_row(
                    app,
                    item,
                    feed_row_date(item),
                    programme_of(item, &programmes),
                    sort.rail_letter(effective_sort_str(item)),
                )
            })
            .collect();
        let selected = self.emby_selected(lib_idx, &rows);
        let hero = self.emby_hero_for(lib_idx, selected.as_deref());
        GuiEmbyProjection {
            library_name: self.emby_library_name(lib_idx),
            collections: Some(collections),
            collection_names,
            feed_group: true,
            loading,
            failed: false,
            rows,
            sort: Some(sort_frame),
            selected,
            hero,
        }
    }

    /// The core's selected row (design D2, row 8.1): the stored pick while
    /// the list still carries it, else the first row — the Hero default.
    fn emby_selected(&self, lib_idx: usize, rows: &[GuiEmbyRow]) -> Option<String> {
        let stored = self
            .gui
            .emby_libs
            .get(&lib_idx)
            .and_then(|selection| selection.picked_row.clone());
        stored
            .filter(|stored| rows.iter().any(|row| &row.id == stored))
            .or_else(|| rows.first().map(|row| row.id.clone()))
    }

    /// The library's display name (row 8.1): the list header's title.
    fn emby_library_name(&self, lib_idx: usize) -> String {
        self.app
            .emby_library_name(lib_idx)
            .map(str::to_string)
            .unwrap_or_default()
    }

    /// The selected row's Hero, when the row paints one (row 8.1): the
    /// resolved row's own Hero gate, and the artwork fetched through the image
    /// cache at the producer's declared chain.
    fn emby_hero_for(&mut self, lib_idx: usize, selected: Option<&str>) -> Option<GuiEmbyHero> {
        let item = emby_item(&self.app, &self.gui, lib_idx, selected?)?;
        hero_item(&self.app, lib_idx, &item).then(|| self.hero(&item))
    }

    /// The selected row's Hero facts (row 8.1): the item's facts as the D1
    /// builders split them, with the artwork fetched through the image cache
    /// at the artwork policy's declared chain and the movie's cast built
    /// through the shared cast helper.
    fn hero(&mut self, item: &EmbyItem) -> GuiEmbyHero {
        let content = hero_content_emby(item);
        let art =
            super::artwork::emby_hero_art(&mut self.app, content.facts.artwork.source.as_ref());
        let cast = hero_cast_emby(&mut self.app, item);
        GuiEmbyHero {
            kind: item.item_type.to_lowercase(),
            title: content.facts.title.clone(),
            facts: hero_facts_emby(item),
            overview: content.overview.clone(),
            art_shape: artwork_shape(super::artwork::hero_shape(
                &self.app,
                content.facts.artwork.shape,
                art.as_ref(),
            )),
            art,
            cast,
        }
    }
}

/// The Emby item a command names (row 8.1): the active list's rows — the
/// feed-group mode's merged checked-folder items, the root-list store's rows in
/// plain mode. `None` when no row carries the id, so a stale command plays
/// nothing. The feed-group arm walks the merged items without the date sort and
/// clones only the match, so a play names one clone. It reads only the core's
/// Emby library state, so it is a core function the Hero's neighbour warm and
/// the play and select paths reach through the App they already have.
pub(crate) fn emby_item(
    app: &App,
    gui: &GuiClientState,
    lib_idx: usize,
    item_id: &str,
) -> Option<EmbyItem> {
    if app.is_feed_home_video_library(lib_idx) {
        let set = super::emby::emby_feed_effective_set(app, gui, lib_idx);
        let (items, _) = feed_gathered(app, lib_idx, set.as_ref());
        return items.into_iter().find(|item| item.id == item_id).cloned();
    }
    super::emby::stored_emby_root_items(gui, lib_idx)
        .find(|item| item.id == item_id)
        .cloned()
}

/// Whether the selected row paints a Hero (row 8.1): the TUI's own gate
/// (`hero_item`, `crates/mbv-components/src/emby_library_content.rs`). A folder
/// paints none, because the Hero describes an item and a folder is a
/// container; a Movies library additionally admits only a `Movie`, so a
/// `BoxSet` row — a collection — paints no Hero either. It reads only the
/// core's Emby library state, so it is a core function the Hero's neighbour
/// warm reaches through the App it already has.
fn hero_item(app: &App, lib_idx: usize, item: &EmbyItem) -> bool {
    if item.is_folder {
        return false;
    }
    let movies = app.emby_library_collection_type(lib_idx) == Some("movies");
    !movies || item.item_type == "Movie"
}

/// The artwork source the Hero paints for `item_id`: the shared Emby hero
/// producer's policy source, and `None` for a row the Hero gate excludes — a
/// folder, or a non-`Movie` in a Movies library — or an id the App no longer
/// holds. It reads only the core's Emby library state, so it is a core
/// function the Hero's neighbour warm reaches through the App it already has.
fn emby_hero_source(
    app: &App,
    gui: &GuiClientState,
    lib_idx: usize,
    item_id: &str,
) -> Option<ArtworkSource> {
    let item = emby_item(app, gui, lib_idx, item_id)?;
    if !hero_item(app, lib_idx, &item) {
        return None;
    }
    hero_content_emby(&item).facts.artwork.source
}

/// The root listing's folders as borrowed `(id, name)` pairs (row 8.1): the
/// programme lookup's source, the pill row's catalog, and the checked set's
/// retention catalog — the listing the group view already holds. Empty while
/// the listing has not landed. It reads only the core's Emby library state, so
/// it is a core function the projection and the feed-group collection site
/// share.
pub(crate) fn root_folder_names(app: &App, lib_idx: usize) -> Vec<(&str, &str)> {
    app.emby_library_root_level(lib_idx)
        .map(|root| {
            root.items
                .iter()
                .filter(|item| item.is_folder)
                .map(|item| (item.id.as_str(), item.name.as_str()))
                .collect()
        })
        .unwrap_or_default()
}

/// The GUI's feed-group list (design D3, D4, D6): the cached items of `set`'s
/// targets, merged and de-duplicated by item id, and ordered by `sort` — the
/// article-stripped sort name under the Title sort, else the date added
/// through the shared date order with undated items last in both orders.
/// Every sort's last tie-break is the item id. The second value is whether the
/// list is still landing: the root listing pending, or a target with no cache
/// entry yet, which is the fetch the ensure step starts. The items are
/// borrowed from the cache, so a frame clones no item. It reads only the
/// core's Emby library state, so it is a core function the projection and the
/// feed-group collection site share.
pub(crate) fn feed_items<'a>(
    app: &'a App,
    lib_idx: usize,
    set: &GuiCollectionSet,
    sort: GuiListSort,
) -> (Vec<&'a EmbyItem>, bool) {
    let (mut items, landing) = feed_gathered(app, lib_idx, set);
    if sort.is_name_sort() {
        items.sort_by_cached_key(|item| {
            (
                gui_title_sort_key(effective_sort_str(item)),
                item.id.clone(),
            )
        });
        return (items, landing);
    }
    items.sort_by(|left, right| {
        sort.compare_date(emby_date_added_secs(left), emby_date_added_secs(right))
            .then_with(|| left.id.cmp(&right.id))
    });
    (items, landing)
}

/// The cached items of `set`'s targets, merged and de-duplicated by item id in
/// the targets' own order, without the date sort — the membership walks read
/// this form. The second value is whether a target has no cache entry yet,
/// which is the fetch the ensure step starts, or the root listing is pending.
pub(crate) fn feed_gathered<'a>(
    app: &'a App,
    lib_idx: usize,
    set: &GuiCollectionSet,
) -> (Vec<&'a EmbyItem>, bool) {
    let state = app.emby_library_feed_home_video(lib_idx);
    let mut items: Vec<&EmbyItem> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut landing = state.is_none();
    if let Some(state) = state {
        match set {
            GuiCollectionSet::All => {
                landing |= gather_feed_target(
                    state.items_for(FeedHomeVideoTarget::All),
                    &mut seen,
                    &mut items,
                );
            }
            GuiCollectionSet::Checked(keys) => {
                for key in keys {
                    landing |= gather_feed_target(
                        state.items_for(FeedHomeVideoTarget::Folder(key.as_str())),
                        &mut seen,
                        &mut items,
                    );
                }
            }
        }
    }
    (items, landing || emby_root_loading(app, lib_idx))
}

/// Whether the feed-view library's root listing has not landed yet: the folder
/// catalog the checked set resolves against.
fn emby_root_loading(app: &App, lib_idx: usize) -> bool {
    app.emby_library_root_level(lib_idx)
        .is_none_or(|root| root.loading)
}

/// Merge one target's cached items into the de-duplicated gather: the first
/// occurrence of each item id is the one the list keeps, as the owned merge
/// did. `true` when the target has no cache entry yet — the fetch the ensure
/// step starts is owed.
fn gather_feed_target<'a>(
    cached: Option<&'a Vec<EmbyItem>>,
    seen: &mut HashSet<&'a str>,
    items: &mut Vec<&'a EmbyItem>,
) -> bool {
    let Some(cached) = cached else {
        return true;
    };
    for item in cached {
        if seen.insert(item.id.as_str()) {
            items.push(item);
        }
    }
    false
}

/// One row of the plain list (row 8.1): the TUI browse row's primary label,
/// with the year on `subtitle` — the plain list's rail replaces the TUI's
/// letter-range pill row, so the row carries no gutter date.
fn plain_row(app: &App, item: &EmbyItem, sort_letter: Option<char>) -> GuiEmbyRow {
    emby_row(app, item, row_secondary(item), None, sort_letter)
}

/// One row of the feed-group lists (row 8.1): the TUI row's primary label,
/// with the programme (root folder) the item sits in on `subtitle` — the user
/// rule of 2026-12-30, so a flat All row names the programme it belongs to —
/// and the list's production-year date in the gutter.
fn feed_row(
    app: &App,
    item: &EmbyItem,
    date: Option<String>,
    programme: Option<String>,
    sort_letter: Option<char>,
) -> GuiEmbyRow {
    emby_row(app, item, programme, date, sort_letter)
}

/// One row of the active list (row 8.1): the TUI row's primary label and
/// kind, the mode's own subtitle, date, and rail letter, the row art its
/// item's Hero artwork policy declares (gui-delineated-rows design D5), read
/// from the cache alone, and the shared play state the one Emby derivation
/// gives the item.
fn emby_row(
    app: &App,
    item: &EmbyItem,
    subtitle: Option<String>,
    date: Option<String>,
    sort_letter: Option<char>,
) -> GuiEmbyRow {
    GuiEmbyRow {
        id: item.id.clone(),
        title: row_title(item),
        subtitle,
        date,
        sort_letter,
        kind: row_kind(item),
        art: row_art(app, emby_row_source(item).as_ref()),
        media: emby_media_kind(item),
        state: MediaSemanticState::from_emby(item),
    }
}

/// The All and folder lists' gutter date (row 8.1): the production year, as
/// the TUI's feed-group rows paint it.
fn feed_row_date(item: &EmbyItem) -> Option<String> {
    (item.production_year > 0).then(|| item.production_year.to_string())
}

/// The programme (root folder) an item sits in (row 8.1): the root listing's
/// folder whose id is the item's containing item's id. `None` for a loose
/// root video — whose containing item is the library itself — and for an
/// item whose folder the listing does not hold. `programmes` is the id-to-
/// name map the projection built once, so the lookup is one probe.
fn programme_of(item: &EmbyItem, programmes: &HashMap<&str, &str>) -> Option<String> {
    programmes
        .get(item.parent_id.as_str())
        .copied()
        .map(str::to_string)
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

/// The checked collection names the Library Status bar's location reads
/// (design D2, D6): `All`, or the checked folders' names in press order, with
/// a folder the listing no longer names falling back to its stable id.
fn feed_collection_names(set: &GuiCollectionSet, folders: &[(&str, &str)]) -> Vec<String> {
    match set {
        GuiCollectionSet::All => vec!["All".to_string()],
        GuiCollectionSet::Checked(keys) => keys
            .iter()
            .map(|key| {
                folders
                    .iter()
                    .find(|(folder_id, _)| folder_id == key)
                    .map_or_else(|| key.clone(), |(_, name)| (*name).to_string())
            })
            .collect(),
    }
}
