use super::types_browse::{AlbumIndex, AlbumIndexState, BrowseResting};
use super::types_events::NavigateLanding;
use super::ui_util::sort_episodes;
use super::{AlbumPathPart, AlbumSearchEntry, App, BrowseLevel, LibEvent, LibraryTab, PAGE_SIZE};
use mbv_core::api::{EmbyClient, EmbyItem};

/// D1 (change `per-destination-item-navigation`): the resolved reveal target.
/// `Chain` keeps the built ancestor-chain nav stack (Movie/generic);
/// `Series`/`Album` name the single reveal item the App lands per kind at
/// drain time. An unresolvable kind is a resolve failure (task 4.2), never a
/// silent misroute.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RevealTarget {
    Chain,
    Series(String),
    Album(String),
}

pub(super) fn retain_grouped_music_items(items: &mut Vec<EmbyItem>, grouped_music: bool) {
    if grouped_music {
        items.retain(|item| !(item.is_folder && item.child_count == Some(0)));
    }
}

pub(super) fn retain_grouped_music_level_items(level: &mut BrowseLevel, grouped_music: bool) {
    let fetched_rows = level.items.len();
    retain_grouped_music_items(&mut level.items, grouped_music);
    level.fetched_rows = fetched_rows;
    level.resting = BrowseResting::new(
        level
            .resting()
            .cursor()
            .min(level.items.len().saturating_sub(1)),
        level.resting().scroll(),
    );
}

/// D1 reveal-item table, pure over the item's own back-references and its
/// ancestor chain (nearest→root, the `get_ancestors` order) so the table
/// test covers the item_type → reveal mapping without a server. `ancestors`
/// is `None` when the kind's own back-reference already decided (the worker
/// skips the round trip).
pub(super) fn resolve_reveal_target(
    item_type: &str,
    item: &EmbyItem,
    ancestors: Option<&[EmbyItem]>,
) -> Result<RevealTarget, String> {
    match item_type {
        // Episode/Season reveal their owning Series: the item's own
        // `series_id` decides without an ancestors round trip; the chain is
        // the fallback.
        "Episode" | "Season" => {
            if !item.series_id.is_empty() {
                return Ok(RevealTarget::Series(item.series_id.clone()));
            }
            owning_ancestor(ancestors, "Series")
                .map(|a| RevealTarget::Series(a.id.clone()))
                .ok_or_else(|| "Could not resolve the item's series".to_string())
        }
        // A track reveals its album: `album_id` first, ancestors fallback.
        "Audio" => {
            if !item.album_id.is_empty() {
                return Ok(RevealTarget::Album(item.album_id.clone()));
            }
            owning_ancestor(ancestors, "MusicAlbum")
                .map(|a| RevealTarget::Album(a.id.clone()))
                .ok_or_else(|| "Could not resolve the item's album".to_string())
        }
        "MusicAlbum" => Ok(RevealTarget::Album(item.id.clone())),
        // An artist does not land (D1): it has no single owning album, and a
        // plain artist browse chain does not render on a grouped Music
        // surface (real-tick render check). The kind resolves to the
        // pre-U2 failure, flashing and leaving the active view unchanged.
        "MusicArtist" => Err("Could not resolve the artist's album".to_string()),
        "Series" => Ok(RevealTarget::Series(item.id.clone())),
        // Movie/generic: the ancestor-chain rebuild is already correct.
        _ => Ok(RevealTarget::Chain),
    }
}

/// The nearest ancestor of `item_type` in a nearest→root ancestor chain.
fn owning_ancestor<'a>(ancestors: Option<&'a [EmbyItem]>, item_type: &str) -> Option<&'a EmbyItem> {
    ancestors.and_then(|chain| chain.iter().find(|a| a.item_type == item_type))
}

/// Fetch one item by id; a miss (empty result, server error) is a resolve
/// failure (deleted item, task 4.2).
fn fetch_reveal_item(client: &EmbyClient, item_id: &str) -> Result<EmbyItem, String> {
    client
        .get_items_by_ids(&[item_id.to_string()])?
        .into_iter()
        .next()
        .ok_or_else(|| format!("Item {item_id} no longer exists"))
}

/// D1+D2: resolve the reveal target for `item` and build the landing payload
/// the App applies at drain time. Every failure mode (no ancestors,
/// unresolvable kind, fetch error, deleted item, unexpected fetched item
/// type) returns the error that `LibEvent::Error` flashes, leaving the
/// active tab unchanged (task 4.2).
fn build_navigate_landing(
    client: &EmbyClient,
    item_id: &str,
    item_type: &str,
    lib_id: &str,
    levels: &[String],
    cached_album_index: Option<&AlbumIndex>,
) -> Result<NavigateLanding, String> {
    // The item's own record supplies the back-references (D1: no ancestors
    // round trip when present); the fetch doubles as the deleted-item check.
    let item = fetch_reveal_item(client, item_id)?;
    // Ancestors are ordered nearest→root: [Season, Series, physical_folder, AggregateFolder].
    // The pure table decides first from the item's own back-references; the
    // round trip is paid only when a kind's fallback needs the chain.
    let reveal = resolve_reveal_target(item_type, &item, None).or_else(|_| {
        let ancestors = client.get_ancestors(item_id)?;
        log::debug!(target:"navigate", "ancestors: {:?}", ancestors.iter().map(|a| format!("{}({})", a.name, a.id)).collect::<Vec<_>>());
        resolve_reveal_target(item_type, &item, Some(&ancestors))
    })?;
    landing_for_target(client, &item, reveal, lib_id, levels, cached_album_index)
}

/// The navigable ancestors inside the library: `get_ancestors` is
/// nearest→root and its last two entries are the physical library folder and
/// the AggregateFolder root, which are never browse levels of their own.
fn ancestors_inside_library(ancestors: &[EmbyItem]) -> &[EmbyItem] {
    &ancestors[..ancestors.len().saturating_sub(2)]
}

/// D1+D2: the landing kind is chosen from the RESOLVED `RevealTarget` kind.
/// An unexpected fetched item type (e.g. a `series_id` resolving to a
/// non-Series record) is a resolve failure (`LibEvent::Error`), never a
/// silent misroute.
fn landing_for_target(
    client: &EmbyClient,
    item: &EmbyItem,
    reveal: RevealTarget,
    lib_id: &str,
    levels: &[String],
    cached_album_index: Option<&AlbumIndex>,
) -> Result<NavigateLanding, String> {
    match reveal {
        RevealTarget::Chain => build_chain_nav_stack(client, item, lib_id)
            .map(|nav_stack| NavigateLanding::Chain { nav_stack }),
        RevealTarget::Series(series_id) => {
            // The item's own record already in hand doubles as the reveal
            // item when it IS the series; otherwise fetch it and verify the
            // type before handing it to the App.
            let series = if item.id == series_id && item.item_type == "Series" {
                item.clone()
            } else {
                fetch_reveal_item(client, &series_id)?
            };
            if series.item_type != "Series" {
                return Err(format!("Item {series_id} is not a Series"));
            }
            // Deep selection (task 6.1, design D6): an Episode reveal rides
            // its own id on the Series landing; a Season reveal stays
            // show-level (default selection).
            let episode_id = (item.item_type == "Episode").then(|| item.id.clone());
            Ok(NavigateLanding::Series {
                reveal: Box::new(series),
                episode_id,
            })
        }
        RevealTarget::Album(album_id) => {
            let album = if item.id == album_id && (item.item_type == "MusicAlbum" || item.is_folder)
            {
                item.clone()
            } else {
                fetch_reveal_item(client, &album_id)?
            };
            // Unmatched album folders come back as plain "Folder" records
            // (the album index builds its terminal level by `is_folder` for
            // the same reason), so a folder record is a valid album reveal.
            if album.item_type != "MusicAlbum" && !album.is_folder {
                return Err(format!("Item {album_id} is not an album"));
            }
            // The recursive activation consumes the configured album-index
            // entry shape (D7): the album plus the root→album folder chain
            // defined by `music.levels`, NOT the raw `get_ancestors` depth.
            // A walk miss is a configured-path failure (flash), never a
            // silent no-op.
            let ancestors =
                configured_album_ancestors(client, lib_id, levels, &album, cached_album_index)?;
            // Deep selection (task 6.2, design D6): an Audio-track reveal
            // rides its own id on the Album landing.
            let track_id = item.is_audio().then(|| item.id.clone());
            Ok(NavigateLanding::Album {
                reveal: Box::new(album),
                ancestors,
                track_id,
            })
        }
    }
}

/// D7: the root→album ancestor path through the configured album-index
/// shape. When `levels` names `album` as its terminal level, the same walk
/// Grouped Music and Inline Search use (`build_album_index_with`) builds every
/// configured level, filters each to folders, and yields the album's
/// canonical path — so the activated nav-stack depth always matches the
/// `is_viewing_album_folders` configured position. A library with no
/// configured album level keeps the legacy raw physical chain (unconfigured
/// music is out of the grouped landing's scope).
fn configured_album_ancestors(
    client: &EmbyClient,
    library_id: &str,
    levels: &[String],
    album: &EmbyItem,
    cached_album_index: Option<&AlbumIndex>,
) -> Result<Vec<AlbumPathPart>, String> {
    if levels.last().map(String::as_str) != Some("album") {
        let ancestors = client.get_ancestors(&album.id)?;
        let inside = ancestors_inside_library(&ancestors);
        return Ok(inside
            .iter()
            .rev()
            .map(|a| AlbumPathPart {
                id: a.id.clone(),
                name: a.display_name(),
            })
            .collect());
    }
    if let Some(index) = cached_album_index {
        if let Some(entry) = index.get(&album.id) {
            return Ok(entry.ancestors.clone());
        }
    }
    let mut fetch = |parent_id: &str, start: usize, limit: usize| {
        client.get_items_sorted(
            parent_id,
            None,
            false,
            start,
            limit,
            "SortName",
            "Ascending",
        )
    };
    let entries = build_album_index_with(library_id, levels, &mut fetch)?;
    entries
        .into_iter()
        .find(|entry| entry.album.id == album.id)
        .map(|entry| entry.ancestors)
        .ok_or_else(|| {
            format!(
                "Could not reach '{}' through the configured music levels",
                album.display_name()
            )
        })
}

/// Movie/generic ancestor-chain rebuild (D2: the Chain arm keeps this
/// pre-change shape verbatim): lib_id first, then inside ancestors from
/// root→item, cursors resting on the next level's target.
fn build_chain_nav_stack(
    client: &EmbyClient,
    item: &EmbyItem,
    lib_id: &str,
) -> Result<Vec<BrowseLevel>, String> {
    // Drop the last two ancestors (physical library folder + AggregateFolder
    // root); everything before those is navigable content inside the library.
    let ancestors = client.get_ancestors(&item.id)?;
    let inside = ancestors_inside_library(&ancestors);

    // Build nav levels: lib_id first, then inside ancestors from root→item, then item itself.
    // inside is nearest→root order; we need root→item, so iterate reversed.
    let mut parents: Vec<String> = vec![lib_id.to_string()];
    for a in inside.iter().rev() {
        parents.push(a.id.clone());
    }

    // targets[i] is the item we want the cursor on inside parents[i]
    let mut targets: Vec<String> = inside.iter().rev().skip(1).map(|a| a.id.clone()).collect();
    if let Some(a) = inside.first() {
        targets.push(a.id.clone());
    } // last inside level → first inside ancestor
    targets.push(item.id.clone()); // deepest level → the item itself

    let mut nav_stack: Vec<BrowseLevel> = Vec::new();
    for (parent_id, target_id) in parents.into_iter().zip(targets) {
        let (mut items, total_count) =
            client.get_items_sorted(&parent_id, None, false, 0, 500, "SortName", "Ascending")?;
        if items
            .first()
            .map(|it| it.item_type == "Episode")
            .unwrap_or(false)
        {
            sort_episodes(&mut items);
        }
        let cursor = items.iter().position(|it| it.id == target_id).unwrap_or(0);
        log::debug!(target:"navigate", "level parent={parent_id} target={target_id} cursor={cursor}/{}", items.len());
        nav_stack.push(BrowseLevel {
            parent_id: parent_id.clone(),
            title: String::new(),
            fetched_rows: items.len(),
            items,
            total_count,
            resting: BrowseResting::new(cursor, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,

            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        });
    }
    Ok(nav_stack)
}

type BrowseRefresh = (
    usize,
    String,
    Option<String>,
    bool,
    String,
    String,
    usize,
    Option<super::render::LetterFilter>,
    Option<mbv_core::config::TvContentMode>,
);
type AlbumIndexFetch<'a> =
    dyn FnMut(&str, usize, usize) -> Result<(Vec<EmbyItem>, usize), String> + 'a;
const ALBUM_INDEX_PAGE_SIZE: usize = 200;

trait TvLatestSource {
    fn get_latest_episodes(&self, view_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String>;
}

impl TvLatestSource for EmbyClient {
    fn get_latest_episodes(&self, view_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String> {
        EmbyClient::get_latest_episodes(self, view_id, limit)
    }
}

trait TvUpcomingSource {
    fn get_upcoming(&self, parent_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String>;
}

impl TvUpcomingSource for EmbyClient {
    fn get_upcoming(&self, parent_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String> {
        EmbyClient::get_upcoming(self, parent_id, limit)
    }
}

fn build_tv_latest_level<S: TvLatestSource>(
    source: &S,
    parent_id: String,
    title: String,
) -> Result<BrowseLevel, String> {
    let items = source.get_latest_episodes(&parent_id, 30)?;
    let total_count = items.len();
    Ok(BrowseLevel {
        parent_id,
        title,
        items,
        fetched_rows: total_count,
        total_count,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Episode".into()),
        unplayed_only: false,
        sort_by: "DateCreated".into(),
        sort_order: "Descending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    })
}

fn build_tv_upcoming_level<S: TvUpcomingSource>(
    source: &S,
    parent_id: String,
    title: String,
) -> Result<BrowseLevel, String> {
    let items = source.get_upcoming(&parent_id, 30)?;
    let total_count = items.len();
    Ok(BrowseLevel {
        parent_id,
        title,
        items,
        fetched_rows: total_count,
        total_count,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Episode".into()),
        unplayed_only: false,
        sort_by: "PremiereDate".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    })
}

// Visibility bump: private -> `pub(super)`. Exercised directly by
// `actions_tests.rs` (a submodule of `actions.rs`, so it needs an explicit
// `crate::app::library_browse_actions::...` import once this lives outside
// `actions.rs`'s own namespace).
pub(super) fn recursive_album_search_eligible(collection_type: &str, levels: &[String]) -> bool {
    collection_type == "music"
        && levels.len() > 1
        && levels.last().is_some_and(|level| level == "album")
}

/// The correct fetch `limit` for an unfiltered whole-library fetch, used by
/// `spawn_all_items_prefetch`/`spawn_search_items_load` so `all_items` (the
/// set `/`-search runs over) always spans the entire library. `lvl.total_count`
/// alone is NOT enough: with a letter-range pill active it's the FILTERED
/// range's count (e.g. ~40 for `A–C` out of a 3,000-item library), which
/// would silently truncate `all_items` to the active range and make search
/// miss everything outside it. `lib.library_total` (the true count captured
/// on the library's first, unfiltered load) is the right number; `.max` is
/// just a defensive fallback for the moment before it's been captured.
// Visibility bump: private -> `pub(super)`. Same reason as
// `recursive_album_search_eligible` above -- exercised directly by
// `actions_tests.rs`.
pub(super) fn full_library_fetch_limit(lib: &LibraryTab, lvl: &BrowseLevel) -> usize {
    lib.library_total
        .unwrap_or(lvl.total_count)
        .max(lvl.total_count)
}

pub(super) fn fetch_all_album_index_items(
    parent_id: &str,
    fetch: &mut AlbumIndexFetch<'_>,
) -> Result<Vec<EmbyItem>, String> {
    let mut items = Vec::new();
    loop {
        let (page, total) = fetch(parent_id, items.len(), ALBUM_INDEX_PAGE_SIZE)?;
        if page.is_empty() {
            break;
        }
        items.extend(page);
        if items.len() >= total {
            break;
        }
    }
    Ok(items)
}

// Visibility bump: private -> `pub(super)`. Same reason as
// `recursive_album_search_eligible` above -- exercised directly by
// `actions_tests.rs`.
pub(super) fn build_album_index_with(
    library_id: &str,
    levels: &[String],
    fetch: &mut AlbumIndexFetch<'_>,
) -> Result<Vec<AlbumSearchEntry>, String> {
    fn visit(
        parent_id: &str,
        depth: usize,
        levels: &[String],
        ancestors: &mut Vec<AlbumPathPart>,
        entries: &mut Vec<AlbumSearchEntry>,
        fetch: &mut AlbumIndexFetch<'_>,
    ) -> Result<(), String> {
        let items = fetch_all_album_index_items(parent_id, fetch)?;
        if depth + 1 == levels.len() {
            // The terminal configured level is "album" by position, not by
            // Emby's `Type` field: unidentified/unmatched album folders come
            // back as plain "Folder" items rather than "MusicAlbum", so
            // filtering on the type string silently dropped them from the
            // index. `is_folder` is the same criterion the intermediate
            // levels above already use.
            for album in items.into_iter().filter(|item| item.is_folder) {
                entries.push(AlbumSearchEntry::from_chain(album, ancestors.clone()));
            }
            return Ok(());
        }

        for item in items.into_iter().filter(|item| item.is_folder) {
            ancestors.push(AlbumPathPart {
                id: item.id.clone(),
                name: item.display_name(),
            });
            visit(&item.id, depth + 1, levels, ancestors, entries, fetch)?;
            ancestors.pop();
        }
        Ok(())
    }

    let mut entries = Vec::new();
    visit(library_id, 0, levels, &mut Vec::new(), &mut entries, fetch)?;
    Ok(entries)
}

impl App {
    pub(super) fn ensure_lib_loaded_for(&mut self, idx: usize) {
        if idx >= self.libs.len() {
            return;
        }
        if self.tab.emby_library_index() == Some(idx) && self.is_feed_home_video_library(idx) {
            self.ensure_feed_home_video_root_loaded(idx);
            return;
        }
        if self.libs[idx].nav_stack.is_empty() {
            if let Some(saved) = self.saved_library_position(idx) {
                if let Some(root) = saved.levels.first() {
                    let filter_kind = super::render::LetterFilterKind::from_collection_type(
                        self.libs[idx].library.collection_type.as_str(),
                    );
                    self.libs[idx].library_total = root.library_total;
                    self.libs[idx].tv_content_mode =
                        (self.libs[idx].library.collection_type == "tvshows").then(|| {
                            super::render::resolve_tv_content_mode(
                                root.library_total.unwrap_or_default(),
                                root.tv_content_mode.as_ref(),
                            )
                        });
                    self.libs[idx].nav_stack.push(BrowseLevel {
                        parent_id: root.parent_id.clone(),
                        title: root.title.clone(),
                        items: Vec::new(),
                        fetched_rows: 0,
                        total_count: 0,
                        resting: BrowseResting::new(0, 0),
                        item_types: root.item_types.clone(),
                        unplayed_only: root.unplayed_only,
                        sort_by: root.sort_by.clone(),
                        sort_order: root.sort_order.clone(),
                        loading: true,

                        all_items: None,
                        letter_filter: root.letter_filter_index.and_then(|index| {
                            if filter_kind == super::render::LetterFilterKind::Tv
                                && index >= super::render::LetterFilter::count_for_kind(filter_kind)
                            {
                                None
                            } else {
                                super::render::LetterFilter::for_index_for_kind(index, filter_kind)
                            }
                        }),
                        tv_content_mode: root.tv_content_mode.clone(),
                        music_grouping: None,
                    });
                    self.spawn_restore_library_position(idx, saved);
                    return;
                }
            }
            let lib_id = self.libs[idx].library.id.clone();
            let lib_name = self.libs[idx].library.name.clone();
            let is_feed_view = {
                let c = self.config.lock().unwrap();
                c.feed_view_libraries.contains(&lib_name.to_lowercase())
            };
            let (item_types, unplayed_only, sort_by, sort_order) =
                match self.libs[idx].library.collection_type.as_str() {
                    "movies" => (Some("Movie".to_string()), false, "SortName", "Ascending"),
                    "tvshows" => (Some("Series".to_string()), false, "SortName", "Ascending"),
                    _ if is_feed_view => {
                        (Some("Video".to_string()), true, "DateCreated", "Ascending")
                    }
                    _ => (None, false, "SortName", "Ascending"),
                };
            self.libs[idx].nav_stack.push(BrowseLevel {
                parent_id: lib_id.clone(),
                title: lib_name.clone(),
                items: vec![],
                fetched_rows: 0,
                total_count: 0,
                resting: BrowseResting::new(0, 0),
                item_types: item_types.clone(),
                unplayed_only,
                sort_by: sort_by.into(),
                sort_order: sort_order.into(),
                loading: true,

                all_items: None,
                letter_filter: None,
                tv_content_mode: None,
                music_grouping: None,
            });
            self.spawn_browse(
                idx,
                lib_id,
                lib_name,
                item_types,
                unplayed_only,
                sort_by.into(),
                sort_order.into(),
            );
        }
    }

    pub(super) fn spawn_restore_library_position(
        &self,
        lib_idx: usize,
        saved: crate::config::LibraryPosition,
    ) {
        let visible_rows = self.lib_page_size();
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        let filter_kind = super::render::LetterFilterKind::from_collection_type(
            self.libs[lib_idx].library.collection_type.as_str(),
        );
        std::thread::spawn(move || {
            let restored = super::restore_library_position_with_fetched_rows_for_kind(
                &saved,
                visible_rows,
                filter_kind,
                |saved_level| {
                    let tv_mode = (filter_kind == super::render::LetterFilterKind::Tv).then(|| {
                        super::render::resolve_tv_content_mode(
                            saved_level.library_total.unwrap_or_default(),
                            saved_level.tv_content_mode.as_ref(),
                        )
                    });
                    if matches!(tv_mode, Some(mbv_core::config::TvContentMode::Latest)) {
                        let items = client.get_latest_episodes(&saved_level.parent_id, 30)?;
                        let total_count = items.len();
                        return Ok((items, total_count, total_count));
                    }
                    if matches!(tv_mode, Some(mbv_core::config::TvContentMode::Upcoming)) {
                        let items = client.get_upcoming(&saved_level.parent_id, 30)?;
                        let total_count = items.len();
                        return Ok((items, total_count, total_count));
                    }
                    let letter_filter = match tv_mode {
                        Some(mbv_core::config::TvContentMode::Range(index)) => {
                            super::render::LetterFilter::for_index_for_kind(index, filter_kind)
                        }
                        _ => saved_level.letter_filter_index.and_then(|index| {
                            super::render::LetterFilter::for_index_for_kind(index, filter_kind)
                        }),
                    };
                    let (name_ge, name_lt) = letter_filter
                        .as_ref()
                        .map(|f| (f.name_ge, f.name_lt))
                        .unwrap_or((None, None));
                    let (items, total_count) = client.get_items_sorted_ranged(
                        &saved_level.parent_id,
                        saved_level.item_types.as_deref(),
                        saved_level.unplayed_only,
                        0,
                        PAGE_SIZE,
                        &saved_level.sort_by,
                        &saved_level.sort_order,
                        name_ge,
                        name_lt,
                    )?;
                    let fetched_rows = items.len();
                    if total_count > fetched_rows {
                        let (items, total_count) = client.get_items_sorted_ranged(
                            &saved_level.parent_id,
                            saved_level.item_types.as_deref(),
                            saved_level.unplayed_only,
                            0,
                            total_count,
                            &saved_level.sort_by,
                            &saved_level.sort_order,
                            name_ge,
                            name_lt,
                        )?;
                        let fetched_rows = items.len();
                        Ok((items, total_count, fetched_rows))
                    } else {
                        Ok((items, total_count, fetched_rows))
                    }
                },
            );
            match restored {
                Ok(Some((position, nav_stack))) => {
                    let _ = tx.send(LibEvent::RestoreLibraryPosition {
                        lib_idx,
                        requested_position: saved,
                        position,
                        nav_stack,
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    pub(super) fn refresh_after_stop(&mut self) {
        if let Ok(content) = self.fetch_home() {
            // The fetch runs synchronously (order-sensitive side
            // effects); the computed content travels to Model-owned
            // `home_content` via lib_tx (task 5.3d).
            let _ = self
                .lib_tx
                .send(LibEvent::HomeContentRefreshed(Box::new(content)));
        }
        if self.last_played_completed {
            if let Some(ref item_id) = self.last_played_item_id.clone() {
                for lib_idx in 0..self.libs.len() {
                    if self.is_feed_home_video_group_view(lib_idx)
                        || self.is_feed_home_video_library(lib_idx)
                    {
                        self.remove_item_from_feed_home_video_cache(lib_idx, item_id);
                        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                            state.loading = true;
                        }
                        self.log_feed_home_video_state(lib_idx, "refresh_after_stop_completed");
                    }
                }
            }
        }
        let fetches: Vec<BrowseRefresh> = self
            .libs
            .iter()
            .enumerate()
            .filter_map(|(i, lib)| {
                lib.nav_stack.last().map(|lvl| {
                    (
                        i,
                        lvl.parent_id.clone(),
                        lvl.item_types.clone(),
                        lvl.unplayed_only,
                        lvl.sort_by.clone(),
                        lvl.sort_order.clone(),
                        lvl.items.len(),
                        lvl.letter_filter.clone(),
                        (lib.library.collection_type == "tvshows" && lib.nav_stack.len() == 1)
                            .then(|| {
                                lvl.tv_content_mode
                                    .clone()
                                    .or_else(|| lib.tv_content_mode.clone())
                            })
                            .flatten(),
                    )
                })
            })
            .collect();
        for (
            lib_idx,
            parent_id,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            loaded_count,
            letter_filter,
            tv_content_mode,
        ) in fetches
        {
            match tv_content_mode {
                Some(mbv_core::config::TvContentMode::Latest) => {
                    self.spawn_tv_latest(
                        lib_idx,
                        parent_id,
                        self.libs[lib_idx].library.name.clone(),
                    );
                }
                Some(mbv_core::config::TvContentMode::Upcoming) => {
                    self.spawn_tv_upcoming(
                        lib_idx,
                        parent_id,
                        self.libs[lib_idx].library.name.clone(),
                    );
                }
                Some(mbv_core::config::TvContentMode::All)
                | Some(mbv_core::config::TvContentMode::Range(_))
                | None => self.spawn_refresh(
                    lib_idx,
                    parent_id,
                    item_types,
                    unplayed_only,
                    sort_by,
                    sort_order,
                    loaded_count,
                    letter_filter,
                ),
            }
        }
    }

    fn spawn_tv_content<F>(&self, lib_idx: usize, parent_id: String, title: String, build: F)
    where
        F: FnOnce(&EmbyClient, String, String) -> Result<BrowseLevel, String> + Send + 'static,
    {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || match build(&client, parent_id.clone(), title) {
            Ok(level) => {
                let _ = tx.send(LibEvent::Loaded {
                    lib_idx,
                    parent_id,
                    level: Box::new(level),
                });
            }
            Err(e) => {
                let _ = tx.send(LibEvent::Error(e));
            }
        });
    }

    pub(super) fn spawn_tv_latest(&self, lib_idx: usize, parent_id: String, title: String) {
        self.spawn_tv_content(
            lib_idx,
            parent_id,
            title,
            build_tv_latest_level::<EmbyClient>,
        );
    }

    pub(super) fn spawn_destination_latest_snapshot(&self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if matches!(
            lib.library.collection_type.as_str(),
            "tvshows" | "playlists" | "music"
        ) {
            return;
        }
        self.spawn_emby_latest_snapshot(lib.library.id.clone(), lib.library.name.clone());
    }

    pub(super) fn spawn_emby_latest_snapshot(&self, library_id: String, title: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let items = client.get_latest(&library_id, 30);
            match items {
                Ok(items) => {
                    let _ = tx.send(LibEvent::EmbyLatestSnapshotFetched {
                        library_id,
                        title,
                        items,
                    });
                }
                Err(error) => {
                    let _ = tx.send(LibEvent::Error(error));
                }
            }
        });
    }

    pub(super) fn spawn_tv_upcoming(&self, lib_idx: usize, parent_id: String, title: String) {
        self.spawn_tv_content(
            lib_idx,
            parent_id,
            title,
            build_tv_upcoming_level::<EmbyClient>,
        );
    }

    pub(super) fn spawn_browse(
        &self,
        lib_idx: usize,
        parent_id: String,
        title: String,
        item_types: Option<String>,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
    ) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        let spawn_started = std::time::Instant::now();
        std::thread::spawn(move || {
            match client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                PAGE_SIZE,
                &sort_by,
                &sort_order,
            ) {
                Ok((items, total_count)) => {
                    let fetched_rows = items.len();
                    log::info!(target: "browse", "Loaded lib_idx={lib_idx} parent={parent_id} total={total_count} got={} thread_total={}ms first3={:?}",
                        items.len(),
                        spawn_started.elapsed().as_millis(),
                        items.iter().take(3).map(|i| format!("{}:{}", i.id, i.name)).collect::<Vec<_>>());
                    let _ = tx.send(LibEvent::Loaded {
                        lib_idx,
                        parent_id: parent_id.clone(),
                        level: Box::new(BrowseLevel {
                            parent_id,
                            title,
                            items,
                            fetched_rows,
                            total_count,
                            resting: BrowseResting::new(0, 0),
                            item_types,
                            unplayed_only,
                            sort_by,
                            sort_order,
                            loading: false,

                            all_items: None,
                            letter_filter: None,
                            tv_content_mode: None,
                            music_grouping: None,
                        }),
                    });
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    pub(super) fn spawn_navigate_to_item(
        &self,
        item_id: String,
        item_type: String,
        libs: Vec<(usize, String, String)>,
    ) {
        let Some(client) = self.emby_snapshot() else {
            // An unavailable Emby is a resolve failure (task 4.2), never a
            // silent drop: the 4.2 flash fires and the active tab is unchanged.
            let _ = self
                .lib_tx
                .send(LibEvent::Error("Emby is unavailable".into()));
            return;
        };
        let tx = self.lib_tx.clone();
        let music_levels = self.music_levels.clone();
        let target_ctype = match item_type.as_str() {
            "Series" | "Episode" | "Season" => "tvshows",
            "Movie" => "movies",
            "Audio" | "MusicAlbum" | "MusicArtist" => "music",
            _ => "",
        };
        let cached_album_index = libs
            .iter()
            .find(|(_, _, ctype)| ctype == target_ctype)
            .and_then(|(_, library_id, _)| self.album_indexes.get(library_id))
            .and_then(|state| match state {
                AlbumIndexState::Ready(index) => Some(index.clone()),
                _ => None,
            });
        std::thread::spawn(move || {
            // Match library by collection_type since CollectionFolder IDs never appear in ancestors
            let (lib_idx, lib_id) = match libs.iter().find(|(_, _, ctype)| ctype == target_ctype) {
                Some((idx, id, _)) => (*idx, id.clone()),
                None => {
                    let _ = tx.send(LibEvent::Error(
                        "No matching library for this item type".into(),
                    ));
                    return;
                }
            };

            // D1: resolve the reveal target and build the per-kind landing
            // before anything else. Any resolution failure sends the flash
            // path (task 4.2) and leaves the active tab unchanged.
            let event = match build_navigate_landing(
                &client,
                &item_id,
                &item_type,
                &lib_id,
                &music_levels,
                cached_album_index.as_deref(),
            ) {
                Ok(landing) => LibEvent::NavigateTo {
                    lib_idx,
                    landing,
                    switch_tab: true,
                },
                Err(e) => LibEvent::Error(e),
            };
            let _ = tx.send(event);
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn spawn_browse_page_sized(
        &self,
        lib_idx: usize,
        parent_id: String,
        start_index: usize,
        item_types: Option<String>,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
        letter_filter: Option<super::render::LetterFilter>,
        limit: usize,
    ) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        let (name_ge, name_lt) = letter_filter
            .as_ref()
            .map(|f| (f.name_ge, f.name_lt))
            .unwrap_or((None, None));
        std::thread::spawn(move || {
            match client.get_items_sorted_ranged(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                start_index,
                limit,
                &sort_by,
                &sort_order,
                name_ge,
                name_lt,
            ) {
                Ok((items, total_count)) => {
                    let _ = tx.send(LibEvent::PageAppended {
                        lib_idx,
                        parent_id,
                        items,
                        total_count,
                    });
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    // Visibility bump: private -> `pub(super)`. Called from
    // `handle_lib_loaded`/`handle_lib_page_appended`, which stay behind in
    // `actions.rs`.
    pub(super) fn spawn_all_items_prefetch(&self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return,
        };
        // `lvl.is_fully_loaded()` compares server rows consumed (`fetched_rows`)
        // against the active range's `total_count` -- with a letter-range pill
        // active, that count is the FILTERED range's total, not the whole
        // library's, so a fully-loaded small range (e.g. 40 items in `A–C`)
        // would wrongly read as "nothing more to prefetch" while `all_items`
        // (which backs whole-library search) is still absent. `all_items` must
        // never be satisfied by just the active range.
        if lvl.letter_filter.is_none() && lvl.is_fully_loaded() {
            return;
        }
        let parent_id = lvl.parent_id.clone();
        let total_count = full_library_fetch_limit(lib, lvl);
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            if let Ok((items, _)) = client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                total_count,
                &sort_by,
                &sort_order,
            ) {
                let _ = tx.send(LibEvent::AllItemsPrefetched {
                    lib_idx,
                    parent_id,
                    items,
                });
            }
        });
    }

    pub(super) fn spawn_search_items_load(&self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return,
        };
        let parent_id = lvl.parent_id.clone();
        // See `spawn_all_items_prefetch` above: always fetch the WHOLE
        // library unfiltered so search covers everything, not just an
        // active letter-range pill's slice.
        let total_count = full_library_fetch_limit(lib, lvl);
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            if let Ok((items, _)) = client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                total_count,
                &sort_by,
                &sort_order,
            ) {
                let _ = tx.send(LibEvent::SearchItemsLoaded {
                    lib_idx,
                    parent_id,
                    items,
                });
            }
        });
    }
}

#[cfg(test)]
mod tv_latest_tests {
    use super::*;
    use rstest::rstest;
    use std::cell::RefCell;

    struct FakeLatestSource {
        request: RefCell<Option<(String, usize)>>,
        items: Vec<EmbyItem>,
    }

    impl TvLatestSource for FakeLatestSource {
        fn get_latest_episodes(
            &self,
            view_id: &str,
            limit: usize,
        ) -> Result<Vec<EmbyItem>, String> {
            *self.request.borrow_mut() = Some((view_id.into(), limit));
            Ok(self.items.clone())
        }
    }

    struct FakeUpcomingSource {
        request: RefCell<Option<(String, usize)>>,
        items: Vec<EmbyItem>,
    }

    impl TvUpcomingSource for FakeUpcomingSource {
        fn get_upcoming(&self, parent_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String> {
            *self.request.borrow_mut() = Some((parent_id.into(), limit));
            Ok(self.items.clone())
        }
    }

    #[test]
    fn latest_library_fetch_uses_home_feed_request_without_home_state() {
        let mut episode = crate::app::tests::make_item("Feed episode", "Episode");
        episode.id = "feed-episode".into();
        let source = FakeLatestSource {
            request: RefCell::new(None),
            items: vec![episode.clone()],
        };

        let level = build_tv_latest_level(&source, "view-id".into(), "TV".into())
            .expect("fake Latest request succeeds");

        assert_eq!(
            source.request.borrow().as_ref(),
            Some(&("view-id".into(), 30))
        );
        assert_eq!(level.items, vec![episode]);
        assert_eq!(level.item_types.as_deref(), Some("Episode"));
    }

    #[test]
    fn upcoming_library_fetch_uses_library_parent_and_flat_episode_rows() {
        let mut episode = crate::app::tests::make_item("Upcoming episode", "Episode");
        episode.id = "upcoming-episode".into();
        let source = FakeUpcomingSource {
            request: RefCell::new(None),
            items: vec![episode.clone()],
        };

        let level = build_tv_upcoming_level(&source, "library-id".into(), "TV".into())
            .expect("fake Upcoming request succeeds");

        assert_eq!(
            source.request.borrow().as_ref(),
            Some(&("library-id".into(), 30))
        );
        assert_eq!(level.items, vec![episode]);
        assert_eq!(level.item_types.as_deref(), Some("Episode"));
    }

    #[rstest]
    #[case::latest(mbv_core::config::TvContentMode::Latest)]
    #[case::upcoming(mbv_core::config::TvContentMode::Upcoming)]
    fn refresh_after_stop_reloads_selected_tv_mode(#[case] mode: mbv_core::config::TvContentMode) {
        let mut app = crate::app::tests::make_app_stub();
        let config = crate::config::Config {
            server_url: "http://127.0.0.1:1".into(),
            ..crate::config::Config::default()
        };
        let http = mbv_core::mock_http::MockHttp::new();
        let client = mbv_core::api::EmbyClient::new(config).with_test_agent(http.agent());
        app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
            std::sync::Mutex::new(client),
        ));

        let mut library = crate::app::tests::make_item("Shows", "CollectionFolder");
        library.id = "tv-library".into();
        library.collection_type = "tvshows".into();
        let mut level_item = crate::app::tests::make_item("Old episode", "Episode");
        level_item.id = "old-episode".into();
        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "tv-library".into(),
                title: "Shows".into(),
                items: vec![level_item],
                fetched_rows: 1,
                total_count: 1,
                resting: BrowseResting::new(0, 0),
                item_types: Some("Episode".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
                tv_content_mode: Some(mode.clone()),
                music_grouping: None,
            }],
            tv_content_mode: Some(mode),
            ..LibraryTab::new(crate::app::tests::make_item("unused", "CollectionFolder"))
        });

        // fetch_home makes three requests; the fourth response is the selected
        // TV mode's request.
        http.respond(
            200,
            r#"[{"ItemId":"tv-library","Name":"Shows","CollectionType":"tvshows"}]"#,
        );
        http.respond(200, r#"{"Items":[]}"#);
        http.respond(200, r#"{"Items":[]}"#);
        http.respond(
            200,
            r#"{"Items":[{"Id":"new-episode","Name":"New episode","Type":"Episode"}],"TotalRecordCount":1}"#,
        );

        app.refresh_after_stop();
        let event = loop {
            match app
                .lib_rx
                .recv()
                .expect("selected TV refresh must complete")
            {
                event @ LibEvent::Loaded { .. } | event @ LibEvent::Refreshed { .. } => {
                    break event
                }
                _ => continue,
            }
        };
        match event {
            LibEvent::Loaded { level, .. } => {
                assert_eq!(level.item_types.as_deref(), Some("Episode"));
                assert_eq!(level.items[0].id, "new-episode");
            }
            LibEvent::Refreshed { .. } => {
                panic!("Latest/Upcoming stop refresh must not use the generic ranged fetch")
            }
            _ => unreachable!(),
        }
    }
}
