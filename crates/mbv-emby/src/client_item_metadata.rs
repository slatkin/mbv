//! The per-item metadata endpoints a Client's playback decoration reads
//! (`add-tauri-gui-client` design D4a): the item DTO's `Chapters`, the
//! `MediaSegments` plugin's detected segment bounds, the Trickplay tile
//! grids' source description, and its tile sheets' bytes. Each is a
//! decoration fetch keyed on an item id; none carries a caller-visible
//! failure mode beyond its `Err`. A payload member this client cannot parse
//! logs a warn and projects no decoration for it, so a changed payload shape
//! leaves a trace rather than reading as "no decoration".

use super::EmbyClient;
use mbv_emby_model::{EmbyChapter, EmbyMediaSegment, EmbyTrickplayGrid};
use serde_json::Value;

impl EmbyClient {
    /// The item DTO's `Chapters` array (`Name`, `StartPositionTicks`),
    /// fetched by `ItemId` through the user item view with `Fields=Chapters`.
    /// An item with no chapters answers an empty array, as does an item the
    /// response carries no `Chapters` member for.
    pub fn get_item_chapters(&self, item_id: &str) -> Result<Vec<EmbyChapter>, crate::EmbyError> {
        let resp: Value = self
            .get(&format!(
                "/Users/{}/Items/{}",
                mbv_net::encode_path_segment(&self.user_id),
                mbv_net::encode_path_segment(item_id)
            ))
            .query("Fields", "Chapters")
            .call()?
            .body_mut()
            .read_json()?;
        Ok(
            serde_json::from_value(resp["Chapters"].clone()).unwrap_or_else(|error| {
                tracing::warn!(
                    name: "emby.item_chapters.parse_failed",
                    target: "api",
                    item = %item_id,
                    error = %error,
                    "the item's Chapters did not parse; projecting none"
                );
                Vec::new()
            }),
        )
    }

    /// `GET /MediaSegments/{itemId}`: the detected Intro/Outro (and other)
    /// segment bounds. A server without detection data — or without the
    /// plugin — answers with no `Items`, which maps to an empty result.
    pub fn get_media_segments(
        &self,
        item_id: &str,
    ) -> Result<Vec<EmbyMediaSegment>, crate::EmbyError> {
        let resp: Value = self
            .get(&format!(
                "/MediaSegments/{}",
                mbv_net::encode_path_segment(item_id)
            ))
            .call()?
            .body_mut()
            .read_json()?;
        Ok(
            serde_json::from_value(resp["Items"].clone()).unwrap_or_else(|error| {
                tracing::warn!(
                    name: "emby.media_segments.parse_failed",
                    target: "api",
                    item = %item_id,
                    error = %error,
                    "the media segments' Items did not parse; projecting no marks"
                );
                Vec::new()
            }),
        )
    }

    /// `GET /Videos/{itemId}/Trickplay`: the trickplay source description —
    /// a map keyed on each available thumbnail width whose values describe
    /// one tile grid. An item with no trickplay data answers an empty map.
    pub fn get_trickplay(&self, item_id: &str) -> Result<Vec<EmbyTrickplayGrid>, crate::EmbyError> {
        let resp: Value = self
            .get(&format!(
                "/Videos/{}/Trickplay",
                mbv_net::encode_path_segment(item_id)
            ))
            .call()?
            .body_mut()
            .read_json()?;
        let Some(map) = resp.as_object() else {
            tracing::warn!(
                name: "emby.trickplay.shape_failed",
                target: "api",
                item = %item_id,
                "trickplay payload was not an object; projecting no source"
            );
            return Ok(Vec::new());
        };
        Ok(map
            .values()
            .filter_map(|grid| match serde_json::from_value(grid.clone()) {
                Ok(parsed) => Some(parsed),
                Err(error) => {
                    tracing::warn!(
                        name: "emby.trickplay.grid_parse_failed",
                        target: "api",
                        item = %item_id,
                        error = %error,
                        "a trickplay grid did not parse; projecting no grid for it"
                    );
                    None
                }
            })
            .collect())
    }

    /// `GET /Videos/{itemId}/Trickplay/{width}/{index}.jpg`: one tile sheet's
    /// JPEG bytes — the `tile_width × tile_height` thumbnail grid of one
    /// `width` grid, `index` being the sheet's ordinal (the thumbnail's
    /// ordinal divided by the sheet's capacity). A sheet the server does not
    /// hold answers a non-success status, which maps to the request's `Err`.
    /// `media_source_id` qualifies an alternate media version; an empty id is
    /// the item's default source and sends no query.
    pub fn get_trickplay_tile(
        &self,
        item_id: &str,
        width: u32,
        index: u32,
        media_source_id: &str,
    ) -> Result<Vec<u8>, crate::EmbyError> {
        let mut request = self.get(&format!(
            "/Videos/{}/Trickplay/{width}/{index}.jpg",
            mbv_net::encode_path_segment(item_id)
        ));
        if !media_source_id.is_empty() {
            request = request.query("MediaSourceId", media_source_id);
        }
        Ok(request.call()?.body_mut().read_to_vec()?)
    }
}
