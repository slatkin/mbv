use crate::app::images::{
    audiobookshelf_book_cover_cache_key, audiobookshelf_cover_cache_key, QUEUE_CARD_PLACEHOLDER_KEY,
};
use crate::app::render::components::card::{queue_card_reserved_rect, render_card_painting};
use crate::app::render::components::widgets::MUSIC_ALBUM_IMAGE_TYPES;
use crate::app::{palette, App};
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::ui_model::queue_card::QueueCardProjection;

fn card_image_types(item_type: &str) -> &'static [&'static str] {
    match item_type {
        "MusicAlbum" => MUSIC_ALBUM_IMAGE_TYPES,
        "Audio" => &["Primary"],
        "Movie" => &["Backdrop", "Primary", "Logo"],
        // `Thumb` before the poster chain: home videos (and other non-Movie
        // video items) often carry only a landscape `Thumb`.
        _ => &["Primary", "Thumb", "Backdrop", "Logo"],
    }
}

fn card_cache_key(item: &EmbyItem) -> String {
    if item.item_type == "Audio" && !item.album_id.is_empty() {
        format!("{}:P", item.album_id)
    } else {
        card_cache_key_for_id(&item.id)
    }
}

/// The artwork cache key for an Emby item id held without an `EmbyItem` (a
/// watched remote Session's now-playing item).
fn card_cache_key_for_id(item_id: &str) -> String {
    format!("{item_id}:P")
}

impl App {
    fn render_card_visualizer(
        &mut self,
        f: &mut Frame,
        area: Rect,
        left_align: bool,
    ) -> (u16, u16, bool) {
        let rect = queue_card_reserved_rect(
            (self.images.last_card_height, self.images.last_card_width),
            self.terminal_height,
            area,
            left_align,
        );
        // The row is fixed (it paints the playback panel's band), so the
        // focus bit is not read; the literal keeps that visible at the site.
        let bg = palette::surface_colors(palette::Surface::QueueCardVisualizer, false).fill;
        self.render_visualizer(f, rect, bg);
        (rect.height, rect.width, false)
    }

    /// Renders the Queue playback panel's visual slot into `area` and
    /// returns `(rows_used, cols_used, image_loading)` (task 3.5, D10: the
    /// slot moved from the base frame's `render_card` into the panel's
    /// paint path; the shell helper resolves the projected slot plus the
    /// shell-resolved image protocol handle — no fetch, no source
    /// resolution. The projection owns every fetch (task 3.4, D9)).
    pub(in crate::app) fn render_queue_playback_slot(
        &mut self,
        f: &mut Frame,
        area: Rect,
        left_align: bool,
    ) -> (u16, u16, bool) {
        let mut projection = self.queue_card_projection.clone();
        if projection.visualizer {
            return self.render_card_visualizer(f, area, left_align);
        }
        if !projection.images_enabled {
            let rect = queue_card_reserved_rect(
                (self.images.last_card_height, self.images.last_card_width),
                self.terminal_height,
                area,
                left_align,
            );
            return (rect.height, rect.width, false);
        }
        // The slot's key: the projected artwork key, or the bundled
        // placeholder when the fetch resolved empty or the slot is the
        // placeholder itself. Resolved from the image cache's authority —
        // a read, never a fetch.
        let artwork_key = projection.cache_key.clone().filter(|key| {
            self.images
                .card_image_states
                .get(key)
                .is_none_or(|entry| entry.img.is_some())
        });
        let placeholder_slot = artwork_key.is_none();
        if placeholder_slot {
            self.ensure_placeholder_card_image();
            // The painter derives `placeholder_slot` from
            // `projection.cache_key`; hand it the resolved-empty fact (a
            // `Some` key whose fetch resolved empty means the placeholder)
            // so the painter's fallback reservation matches the adapter's
            // derivation (review of tasks 3.1-3.4).
            projection.cache_key = None;
        }
        let key = artwork_key.as_deref().unwrap_or(QUEUE_CARD_PLACEHOLDER_KEY);
        let loading = !placeholder_slot && self.images.card_image_loading.contains(key);
        let last_card = (self.images.last_card_height, self.images.last_card_width);
        let terminal_height = self.terminal_height;
        let image = self.cached_image_protocol_mut(key);
        let (height, width, loading) = render_card_painting(
            f,
            area,
            left_align,
            &projection,
            loading,
            image,
            last_card,
            terminal_height,
        );
        self.images.last_card_height = height;
        self.images.last_card_width = width;
        (height, width, loading)
    }

    fn queue_card_emby_source(
        &self,
        playback: crate::app::ui_model::playback::PlaybackState,
    ) -> Option<(usize, EmbyItem)> {
        let slotless_active = playback.active && playback.active_idx.is_none();
        let active = if playback.active {
            playback.active_idx.and_then(|idx| {
                self.playback_queue()
                    .emby_item_at(idx)
                    .cloned()
                    .map(|item| (idx, item))
            })
        } else {
            None
        };
        active.or_else(|| {
            if slotless_active {
                return None;
            }
            let queue = self.displayed_queue();
            queue
                .clone_emby_item_at(queue.queue_cursor)
                .map(|item| (queue.queue_cursor, item))
        })
    }

    /// A watched remote Session names an item outside the local queue. Its own
    /// Primary image is used (an episode's still lives there), never the selected row.
    fn project_slotless_session(&mut self, projection: &mut QueueCardProjection) -> bool {
        let Some(item_id) = self
            .connected_session_state
            .as_ref()
            .and_then(|session| session.now_playing_item_id.clone())
        else {
            return true;
        };
        let cache_key = card_cache_key_for_id(&item_id);
        self.fetch_card_image(cache_key.clone(), item_id, String::new(), &["Primary"]);
        projection.cache_key = Some(cache_key);
        true
    }

    /// The active/selected slot holds a non-Emby item (or the queue is empty).
    fn project_audiobookshelf_cover(
        &mut self,
        playback: crate::app::ui_model::playback::PlaybackState,
        projection: &mut QueueCardProjection,
    ) -> bool {
        let slotless_active = playback.active && playback.active_idx.is_none();
        let raw_item = if playback.active {
            playback
                .active_idx
                .and_then(|idx| self.playback_queue().item_at(idx).cloned())
        } else {
            None
        }
        .or_else(|| {
            if slotless_active {
                return None;
            }
            let queue = self.displayed_queue();
            queue.item_at(queue.queue_cursor).cloned()
        });
        let cover_id = match raw_item {
            Some(QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(ep))) => {
                Some((ep.library_item_id, false))
            }
            Some(QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(book))) => {
                Some((book.library_item_id, true))
            }
            _ => None,
        };
        let Some((item_id, is_book)) = cover_id else {
            return true;
        };
        let Some(server_url) = self
            .config
            .lock()
            .unwrap()
            .audiobookshelf_setup
            .as_ref()
            .map(|setup| setup.server_url.clone())
        else {
            return true;
        };
        if is_book {
            self.fetch_audiobookshelf_book_cover(server_url.clone(), item_id.clone());
        } else {
            self.fetch_audiobookshelf_cover(server_url.clone(), item_id.clone());
        }
        let cache_key = if is_book {
            audiobookshelf_book_cover_cache_key(
                &server_url,
                &item_id,
                self.current_protocol_suffix(),
            )
        } else {
            audiobookshelf_cover_cache_key(&server_url, &item_id, self.current_protocol_suffix())
        };
        projection.cache_key = Some(cache_key);
        true
    }

    fn prefetch_card_images(&mut self, cursor: usize) {
        // Collect data first (releasing the borrow on queue) then call fetch (&mut self).
        const PREFETCH_AHEAD: usize = 3;
        const PREFETCH_BEHIND: usize = 1;
        let queue_ref = self.playback_queue();
        let n = queue_ref.total_queue_len();
        let start = cursor.saturating_sub(PREFETCH_BEHIND).min(n);
        let end = (cursor + PREFETCH_AHEAD + 1).min(n);
        let prefetch: Vec<(String, String, String, String)> = queue_ref.slots()[start..end]
            .iter()
            .enumerate()
            .filter(|(i, _)| start + i != cursor)
            .filter_map(|(_, slot)| slot.item.as_emby())
            .map(|item| {
                (
                    card_cache_key(item),
                    item.id.clone(),
                    item.series_id.clone(),
                    item.item_type.clone(),
                )
            })
            .collect();
        for (key, id, series_id, item_type) in prefetch {
            self.fetch_list_card_image_when_idle(key, id, series_id, card_image_types(&item_type));
        }
    }

    /// The queue projection issues every fetch for the now-playing item and
    /// projects the slot the painter consumes. Active-first, then viewed selection.
    pub(in crate::app) fn refresh_queue_card_image(&mut self) {
        let mut projection = QueueCardProjection {
            cache_key: None,
            images_enabled: self.images.images_enabled(),
            visualizer: self.visualizer_enabled,
        };
        if projection.visualizer || !projection.images_enabled {
            self.queue_card_projection = projection;
            return;
        }

        // Presentation follows a selected-but-unconfirmed slot, switching artwork with its highlight.
        let playback = self.displayed_playback_state();
        let slotless_active = playback.active && playback.active_idx.is_none();
        let Some((cursor, item)) = self.queue_card_emby_source(playback) else {
            if slotless_active {
                self.project_slotless_session(&mut projection);
            } else {
                self.project_audiobookshelf_cover(playback, &mut projection);
            }
            self.queue_card_projection = projection;
            return;
        };

        let img_types = card_image_types(&item.item_type);
        let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
        let cache_key = card_cache_key(&item);
        self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
        self.prefetch_card_images(cursor);
        projection.cache_key = Some(cache_key);
        self.queue_card_projection = projection;
    }
}
