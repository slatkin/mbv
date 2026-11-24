//! The image cache's protocol-independent half (`extract-client-core` design
//! D3): the browser's idle poster prefetch. It reads and writes the cache and
//! its fetch reservations, never a terminal protocol, so the core owns it; the
//! ratatui and `ratatui-image` halves — the placeholder entry's encoding,
//! pickers, painting — stay in the TUI shell's `image_protocol`.

use crate::app::App;

impl App {
    /// Pre-warm nearby movie poster images for the migrated browser owner.
    /// The caller supplies the projected item window and the owner's
    /// authoritative cursor; only a selected, non-folder Movie enables the
    /// surrounding prefetch.
    pub fn fetch_nearby_movie_posters(
        &mut self,
        items: &[mbv_emby_model::EmbyItem],
        cursor: usize,
    ) {
        const PREFETCH_AHEAD: usize = 3;
        const PREFETCH_BEHIND: usize = 1;
        if !items
            .get(cursor)
            .is_some_and(|item| item.item_type == "Movie" && !item.is_folder)
        {
            return;
        }
        let start = cursor.saturating_sub(PREFETCH_BEHIND);
        let end = (cursor + PREFETCH_AHEAD + 1).min(items.len());
        let prefetch: Vec<(String, String, String)> = items[start..end]
            .iter()
            .enumerate()
            .filter(|(i, item)| start + i != cursor && item.item_type == "Movie" && !item.is_folder)
            .map(|(_, item)| {
                (
                    format!("{}:cmp_primary", item.id),
                    item.id.clone(),
                    item.series_id.clone(),
                )
            })
            .collect();
        if self.images.protocol_enabled() {
            for (cache_key, item_id, series_id) in prefetch {
                self.fetch_list_card_image_when_idle(cache_key, item_id, series_id, &["Primary"]);
            }
        }
    }
}
