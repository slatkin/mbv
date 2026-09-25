use super::widgets::RENDER_FILTER;
use crate::app::images::{
    audiobookshelf_book_cover_cache_key, audiobookshelf_cover_cache_key, QUEUE_CARD_PLACEHOLDER_KEY,
};
use crate::app::{palette, App};
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

/// The queue visual slot's image projection (task 3.4, design D9): the queue
/// projection issues every fetch for the now-playing item and projects the
/// slot the painter consumes; painting reads it only — no fetch, no source
/// resolution. The shell's image-cache authority resolves the slot's
/// protocol handle for the painter (the same seam `paint_home_image` uses);
/// the painter itself takes plain data only.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct QueueCardProjection {
    /// The artwork slot's cache key; `None` paints the bundled placeholder
    /// (empty queue, a source without artwork, or a resolved-empty fetch).
    pub(in crate::app) cache_key: Option<String>,
    /// Terminal images are off: the slot reserves its last painted geometry
    /// and paints nothing.
    pub(in crate::app) images_enabled: bool,
    /// The visualizer is selected: painting stays App-coupled (it reads the
    /// shell's sample window and panel focus) until the visual slot moves
    /// into the Queue playback panel (task 3.5).
    pub(in crate::app) visualizer: bool,
}

fn card_image_types(item_type: &str) -> &'static [&'static str] {
    match item_type {
        "MusicAlbum" => super::widgets::MUSIC_ALBUM_IMAGE_TYPES,
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

/// The rectangle the queue card reserves for artwork: the last rendered
/// image/visualizer size, or the full reserved slot (capped like the artwork
/// height) before anything has rendered. Used by both the blank reservations
/// and the visualizer so `v` never moves the queue list.
/// The now-playing image's height cap: 12 rows under 40 rows of terminal
/// height, 16 under 50, 24 otherwise. Kept small enough that the queue
/// list below keeps the title separator and a few rows.
pub(in crate::app) fn queue_card_height_cap(height: u16) -> u16 {
    if height < 40 {
        12
    } else if height < 50 {
        16
    } else {
        24
    }
}

pub(in crate::app) fn queue_card_reserved_rect(
    last_card: (u16, u16),
    terminal_height: u16,
    area: Rect,
    left_align: bool,
) -> Rect {
    let (last_height, last_width) = last_card;
    let max_h = area.height.min(queue_card_height_cap(terminal_height));
    let height = if last_height == 0 {
        // The fallback slot is two terminal cells wide per row, matching
        // square artwork at the terminal's cell aspect. Constrain both axes:
        // reserving max_h without its matching width makes a narrow column
        // start tall and then shrink when the real image is measured.
        let width_limited = area.width.div_ceil(2);
        let height_limited = if max_h == area.height {
            // Keep the queue title separator and one queue row visible when
            // the card area is shorter than its normal image cap.
            max_h.saturating_sub(2)
        } else {
            max_h
        };
        height_limited.min(width_limited)
    } else {
        last_height
    };
    let width = if last_width == 0 {
        if left_align {
            height.saturating_mul(2).min(area.width)
        } else {
            area.width
        }
    } else {
        last_width
    };
    let x = if left_align {
        area.x
    } else {
        area.x + (area.width.saturating_sub(width)) / 2
    };
    Rect {
        x,
        y: area.y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

type CardImageProtocol = ratatui_image::thread::ThreadProtocol;

/// The queue visual slot's painter (task 3.4, design D9): a free function
/// over projected state with no `App` access. `image` is the shell-resolved
/// protocol handle for the slot's key (`None` while not ready); `loading`
/// reserves the loading rectangle for an in-flight fetch. Returns
/// `(rows_used, cols_used, image_loading)`.
pub(in crate::app) fn render_card_painting(
    f: &mut Frame,
    area: Rect,
    left_align: bool,
    projection: &QueueCardProjection,
    // `true` while a fetch for the slot's key is in flight: the painter
    // reserves the loading rectangle instead of leaving the slot blank.
    loading: bool,
    image: Option<&mut CardImageProtocol>,
    last_card: (u16, u16),
    terminal_height: u16,
) -> (u16, u16, bool) {
    // The visualizer never reaches this painter (the `App` adapter paints it
    // from the shell's sample window); degenerate input reserves geometry.
    if projection.visualizer || !projection.images_enabled {
        let rect = queue_card_reserved_rect(last_card, terminal_height, area, left_align);
        return (rect.height, rect.width, false);
    }
    // The bundled placeholder slot caps its height at the same tier cap as
    // the real image (24 rows at full height), like the compact banner's
    // poster placeholder.
    let placeholder_slot = projection.cache_key.is_none();
    let cap = queue_card_height_cap(terminal_height);
    let max_h = if placeholder_slot {
        area.height.min(24)
    } else {
        area.height
    }
    .min(cap);
    let mut image = image;
    let actual_size = image.as_deref_mut().and_then(|state| {
        let avail = ratatui::layout::Size {
            width: area.width,
            height: max_h,
        };
        // `size_for` returns `None` while the resize+encode is still
        // in-flight on the worker thread (ThreadProtocol has taken its
        // inner protocol to send it off). Fall through to the
        // loading/placeholder path below for that frame; the next
        // frame after the response arrives will have a size again.
        state
            .size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail)
            .map(|actual| (actual.height, actual.width, actual))
    });
    if let Some((height, width, _actual)) = actual_size {
        let img_x = if left_align {
            area.x
        } else {
            area.x + (area.width.saturating_sub(width)) / 2
        };
        let img_rect = Rect {
            x: img_x,
            y: area.y,
            width,
            height,
        };
        f.render_stateful_widget(
            ratatui_image::StatefulImage::default()
                .resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
            img_rect,
            image.expect("image handle present"),
        );
        return (height, width, false);
    }
    // No image loaded yet. If a fetch is in-flight and we have never rendered
    // a card before, reserve the full height cap so the queue panel doesn't
    // expand then collapse when the first image arrives; otherwise the last
    // painted geometry holds the slot steady.
    let (last_height, last_width) = last_card;
    let reservation = if last_height == 0 && loading {
        queue_card_reserved_rect(last_card, terminal_height, area, left_align)
    } else {
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: last_height,
        }
    };
    let placeholder_w = if last_width == 0 && loading {
        reservation.width
    } else {
        last_width
    };
    // The reserved area above was otherwise left visually blank while
    // loading -- paint a dim block over it instead, matching the
    // compact movie banner's own poster placeholder. Image aspect
    // ratios vary too widely here (backdrop, poster, album art,
    // thumbnail) to estimate a tighter width the way the banner does
    // for posters specifically, so this fills the full reserved area.
    if loading && reservation.height > 0 {
        f.render_widget(
            Block::default().style(Style::default().bg(
                palette::surface_colors(palette::Surface::ArtworkLoadingPlaceholder, false).fill,
            )),
            reservation,
        );
    }
    if placeholder_slot && reservation.height == 0 && last_width == 0 && !loading {
        // An empty queue with no previous artwork geometry still reserves its
        // fallback rectangle so toggling `v` never moves the queue list.
        let rect = queue_card_reserved_rect(last_card, terminal_height, area, left_align);
        return (rect.height, rect.width, false);
    }
    (reservation.height, placeholder_w, loading)
}

impl App {
    fn render_card_visualizer(
        &mut self,
        f: &mut Frame,
        area: Rect,
        left_align: bool,
    ) -> (u16, u16, bool) {
        let rect = queue_card_reserved_rect(
            (self.last_card_height, self.last_card_width),
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
                (self.last_card_height, self.last_card_width),
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
            !self
                .card_image_states
                .get(key)
                .is_some_and(|entry| entry.img.is_none())
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
        let loading = !placeholder_slot && self.card_image_loading.contains(key);
        let last_card = (self.last_card_height, self.last_card_width);
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
        self.last_card_height = height;
        self.last_card_width = width;
        (height, width, loading)
    }

    fn queue_card_emby_source(
        &self,
        playback: crate::app::PlaybackState,
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
        playback: crate::app::PlaybackState,
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
            Some(QueueItem::Audiobookshelf(ep)) => Some((ep.library_item_id, false)),
            Some(QueueItem::AudiobookshelfBook(book)) => Some((book.library_item_id, true)),
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
            images_enabled: self.images_enabled(),
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
