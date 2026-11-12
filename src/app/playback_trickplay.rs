//! The scrubber preview's trickplay tile sheets (`add-tauri-gui-client` task
//! 6.7, design D4a): the sheet a hovered position falls in, fetched by the
//! active queue item's identity and cached with a bound. The data is
//! presentation decoration only — no Owner, ctrl, or mpv change — and absent
//! data (no trickplay source, a failed fetch, a server without the sheet)
//! yields no preview frame and never fails the frame.
//!
//! The fetch issues from a Client's frame path, exactly as the artwork and
//! item-metadata fetches do: one worker per key, deduped on
//! `trickplay_sheets_loading`, landing in `trickplay_sheets` through
//! [`App::drain_trickplay_sheet_events`]. A `404` records a sheet the server
//! does not hold, so a hover does not retry it every frame. Any other failure
//! records nothing — the sheet may exist — and holds that key out of the
//! retry path until the hover names a different sheet or the hover ends.

use std::sync::Arc;

use mbv_queue::QueueItemContentId;

use crate::app::App;
use crate::app::queue_playback_projection::{TrickplayGrid, TrickplayTile};

/// The number of tile sheets the cache holds. One sheet carries a hundred
/// thumbnails — some sixteen minutes of media at Emby's usual ten-second
/// interval — so four sheets cover just over an hour of one item's hover
/// range at a few hundred kilobytes each; a fifth sheet evicts the oldest
/// insertion. A hover sweeps a region, not the whole bar, so the bound keeps
/// the crossings a scrub makes without holding a feature's every sheet.
pub const TRICKPLAY_SHEET_CACHE_BOUND: usize = 4;

/// The identity of one cached tile sheet: the item, the grid's thumbnail
/// width, and the sheet's ordinal — the three parts of the fetch's URL.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TrickplaySheetKey {
    /// The Emby item the sheet belongs to.
    pub item_id: String,
    /// The thumbnail pixel width, the grid the sheet answers.
    pub width: u32,
    /// The sheet's ordinal in the grid's series.
    pub sheet: u32,
}

impl TrickplaySheetKey {
    /// The key a [`Self::wire_key`] names, the inverse of `wire_key`: the item
    /// id may hold colons, so the width and the sheet ordinal parse from the
    /// right. `None` for a wire key without the three parts or with a
    /// non-numeric width or ordinal.
    #[must_use]
    pub fn from_wire_key(wire: &str) -> Option<Self> {
        let (left, sheet) = wire.rsplit_once(':')?;
        let (item_id, width) = left.rsplit_once(':')?;
        Some(Self {
            item_id: item_id.to_owned(),
            width: width.parse().ok()?,
            sheet: sheet.parse().ok()?,
        })
    }

    /// The key as the frame's opaque identity: two frames name the same
    /// sheet exactly when their keys are equal.
    #[must_use]
    pub fn wire_key(&self) -> String {
        format!("{}:{}:{}", self.item_id, self.width, self.sheet)
    }
}

/// One hovered trickplay tile as a Client's frame carries it (task 6.7): the
/// thumbnail's place in its sheet, the thumbnail's pixel size, and the sheet's
/// JPEG bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrickplayPreviewTile {
    /// The sheet this tile lives in.
    pub key: TrickplaySheetKey,
    /// The thumbnail's pixel width.
    pub width: u32,
    /// The thumbnail's pixel height.
    pub height: u32,
    /// The sheet's thumbnail columns (`tile_width`).
    pub columns: u32,
    /// The thumbnail's column in the sheet.
    pub column: u32,
    /// The thumbnail's row in the sheet.
    pub row: u32,
    /// The sheet's JPEG bytes.
    pub jpeg: Arc<[u8]>,
}

/// What one tile-sheet fetch produced (task 6.7): the sheet's bytes, the
/// server's definitive absence, or a failure that says nothing about whether
/// the sheet exists.
#[derive(Debug)]
pub(in crate::app) enum TrickplaySheetFetch {
    /// The server answered with the sheet's JPEG bytes.
    Loaded(Vec<u8>),
    /// The server answered `404`: it holds no such sheet, so the absence is
    /// cached and a hover never re-asks.
    Absent,
    /// The fetch failed in transport — a timeout, a dropped connection, a
    /// `5xx`, a body read error: the sheet may exist, so nothing is cached and
    /// the key waits for the hover to name a different sheet before it is
    /// asked again.
    Failed,
}

impl App {
    /// The cached trickplay sheet `key` names (design D7 of
    /// `gui-change-driven-sections`): the read-only read the GUI core answers
    /// `mbv-art` sheet requests with. It starts no fetch —
    /// `trickplay_preview_tile` owns those — so a request answers only what
    /// the cache already holds, and a sheet the server does not hold answers
    /// nothing, exactly as a key the cache never saw does.
    #[must_use]
    pub fn trickplay_sheet_bytes(&self, key: &TrickplaySheetKey) -> Option<Arc<[u8]>> {
        self.trickplay_sheets.get(key).cloned().flatten()
    }

    /// The active item's trickplay tile at `ticks` (task 6.7): the smallest
    /// offered grid that has a tile for the position, the tile's place in
    /// its sheet, and the sheet's bytes when the cache holds them. Issues the
    /// sheet's fetch when it does not, so the next frame carries it.
    ///
    /// `None` while no Emby item plays, while the item's metadata fetch has
    /// not landed, when the item offers no usable grid, while the sheet is in
    /// flight, and for a sheet the server does not hold — every kind of
    /// absence paints the preview's placeholder instead.
    #[must_use]
    pub fn trickplay_preview_tile(&mut self, ticks: i64) -> Option<TrickplayPreviewTile> {
        let grid = self.trickplay_grid_for(ticks)?;
        let tile = grid.grid.tile_at(ticks)?;
        let key = TrickplaySheetKey {
            item_id: grid.item_id,
            width: grid.grid.width,
            sheet: tile.sheet,
        };
        // The hover naming a different sheet retires a transient failure's
        // hold, so a return to the failed sheet asks again (task 6.7 fix).
        if self
            .trickplay_sheet_failed
            .as_ref()
            .is_some_and(|failed| failed != &key)
        {
            self.trickplay_sheet_failed = None;
        }
        let cached = self.trickplay_sheets.get(&key).cloned();
        if let Some(jpeg) = cached {
            return jpeg.map(|jpeg| preview(&grid.grid, &key, tile, jpeg));
        }
        self.issue_trickplay_sheet_fetch(&key, grid.grid.media_source_id);
        None
    }

    /// End the hover episode: the pointer left the scrubber bar. A transient
    /// failure's hold belongs to one hover, so the next hover of the same
    /// sheet asks again (task 6.7 fix). The frame path calls this every frame
    /// the pointer is away, so it does nothing when no hold is set.
    pub fn trickplay_hover_ended(&mut self) {
        self.trickplay_sheet_failed = None;
    }

    /// The active Emby item's grid that holds a tile for `ticks`, with the
    /// item's id: the smallest thumbnail width that answers, since the
    /// preview paints a thumbnail-sized box and the sheet is the preview's
    /// largest payload.
    fn trickplay_grid_for(&self, ticks: i64) -> Option<TrickplayGridChoice> {
        let key = self
            .queue_item_at_playhead(self.displayed_playback_state())
            .map(mbv_queue::QueueItem::content_id)?;
        let QueueItemContentId::Emby(item_id) = &key else {
            return None;
        };
        let grids = self.playback_metadata_cache.get(&key)?.trickplay.as_ref()?;
        let (grid, _) = grids
            .iter()
            .filter_map(|grid| grid.tile_at(ticks).map(|tile| (grid, tile)))
            .min_by_key(|(grid, _)| grid.width)?;
        Some(TrickplayGridChoice {
            item_id: item_id.clone(),
            grid: grid.clone(),
        })
    }

    /// Issue the sheet's fetch when the cache lacks it, none is in flight,
    /// and its last fetch did not fail in transport (task 6.7). A no-op while
    /// no Emby client is installed, so the next frame retries a transient
    /// absence instead of caching it.
    fn issue_trickplay_sheet_fetch(&mut self, key: &TrickplaySheetKey, media_source_id: String) {
        if self.trickplay_sheets_loading.contains(key)
            || self.trickplay_sheet_failed.as_ref() == Some(key)
        {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        self.trickplay_sheets_loading.insert(key.clone());
        let tx = self.channels.trickplay_tx.clone();
        let key = key.clone();
        std::thread::spawn(move || {
            let fetch = fetch_trickplay_sheet(&client, &key, &media_source_id);
            let _ = tx.send((key, fetch));
        });
    }

    /// Drain landed tile sheets into the cache, evicting the oldest entry
    /// past [`TRICKPLAY_SHEET_CACHE_BOUND`]. A definitive absence is cached;
    /// a transient failure is held out of the retry path instead, so it is
    /// never mistaken for a sheet the server does not hold. Returns whether
    /// anything landed, so a Client's tick frames the cache's new state.
    pub(in crate::app) fn drain_trickplay_sheet_events(&mut self) -> bool {
        let mut produced = false;
        while let Ok((key, fetch)) = self.channels.trickplay_rx.try_recv() {
            produced = true;
            self.trickplay_sheets_loading.remove(&key);
            match fetch {
                TrickplaySheetFetch::Loaded(jpeg) => {
                    self.release_trickplay_sheet_failure(&key);
                    self.cache_trickplay_sheet(key, Some(Arc::from(jpeg)));
                }
                TrickplaySheetFetch::Absent => {
                    self.release_trickplay_sheet_failure(&key);
                    self.cache_trickplay_sheet(key, None);
                }
                TrickplaySheetFetch::Failed => self.trickplay_sheet_failed = Some(key),
            }
        }
        produced
    }

    /// Cache one landed sheet, evicting the oldest insertion past
    /// [`TRICKPLAY_SHEET_CACHE_BOUND`].
    fn cache_trickplay_sheet(&mut self, key: TrickplaySheetKey, jpeg: Option<Arc<[u8]>>) {
        self.trickplay_sheets.insert(key, jpeg);
        if self.trickplay_sheets.len() > TRICKPLAY_SHEET_CACHE_BOUND {
            self.trickplay_sheets.shift_remove_index(0);
        }
    }

    /// Release the transient-failure hold on `key`: it answered, so the cache
    /// carries its outcome and the hold has nothing left to withhold.
    fn release_trickplay_sheet_failure(&mut self, key: &TrickplaySheetKey) {
        if self.trickplay_sheet_failed.as_ref() == Some(key) {
            self.trickplay_sheet_failed = None;
        }
    }
}

/// One sheet's fetch, classified for the cache (task 6.7): the bytes, the
/// server's definitive absence, or a transient failure the next hover over
/// the sheet may retry.
fn fetch_trickplay_sheet(
    client: &mbv_emby::EmbyClient,
    key: &TrickplaySheetKey,
    media_source_id: &str,
) -> TrickplaySheetFetch {
    match client.get_trickplay_tile(&key.item_id, key.width, key.sheet, media_source_id) {
        Ok(jpeg) => TrickplaySheetFetch::Loaded(jpeg),
        Err(error) => {
            let absent = error.is_not_found();
            tracing::debug!(
                name: "playback_trickplay.sheet_failed",
                target: "playback_trickplay",
                item = %key.item_id,
                width = key.width,
                sheet = key.sheet,
                absent,
                error = %error,
                "trickplay sheet fetch failed; the preview paints no frame"
            );
            if absent {
                TrickplaySheetFetch::Absent
            } else {
                TrickplaySheetFetch::Failed
            }
        }
    }
}

/// The grid a position resolved to, with the item it belongs to.
struct TrickplayGridChoice {
    item_id: String,
    grid: TrickplayGrid,
}

fn preview(
    grid: &TrickplayGrid,
    key: &TrickplaySheetKey,
    tile: TrickplayTile,
    jpeg: Arc<[u8]>,
) -> TrickplayPreviewTile {
    TrickplayPreviewTile {
        key: key.clone(),
        width: grid.width,
        height: grid.height,
        columns: grid.tile_width,
        column: tile.column,
        row: tile.row,
        jpeg,
    }
}

// The module's own tests: the preview's grid and sheet resolution, and the
// cache's bound.
#[cfg(test)]
mod tests;
