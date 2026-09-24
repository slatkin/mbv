use super::search::build_album_index_with;
use crate::app::infra::ui_util::sort_episodes;
use crate::app::state::types::browse::{AlbumIndex, AlbumIndexState, BrowseResting};
use crate::app::state::types::events::NavigateLanding;
use crate::app::{AlbumPathPart, App, BrowseLevel, LibEvent};
use mbv_core::api::{EmbyClient, EmbyItem};

/// D1 (change `per-destination-item-navigation`): the resolved reveal target.
/// `Chain` keeps the built ancestor-chain nav stack (Movie/generic);
/// `Series`/`Album` name the single reveal item the App lands per kind at
/// drain time. An unresolvable kind is a resolve failure (task 4.2), never a
/// silent misroute.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::app) enum RevealTarget {
    Chain,
    Series(String),
    Album(String),
}
/// D1 reveal-item table, pure over the item's own back-references and its
/// ancestor chain (nearest→root, the `get_ancestors` order) so the table
/// test covers the item_type → reveal mapping without a server. `ancestors`
/// is `None` when the kind's own back-reference already decided (the worker
/// skips the round trip).
pub(in crate::app) fn resolve_reveal_target(
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

impl App {
    pub(in crate::app) fn spawn_navigate_to_item(
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
}
