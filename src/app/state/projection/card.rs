use crate::app::App;
use mbv_emby_model::EmbyItem;
use mbv_images::{
    QUEUE_CARD_PLACEHOLDER_KEY, audiobookshelf_book_cover_cache_key, audiobookshelf_cover_cache_key,
};
use mbv_queue::QueueItem;
use mbv_render::components::card::{queue_card_reserved_rect, render_card_painting};
use mbv_render::components::widgets::MUSIC_ALBUM_IMAGE_TYPES;
use ratatui::Frame;
use ratatui::layout::Rect;

use mbv_ui_model::playback::{NowPlayingTitleSite, QueueCardProjection};

#[derive(Clone, Copy)]
struct TitleSiteFacts {
    playback: TitleSitePlayback,
    art: TitleArtFacts,
    slot: TitleSlotFacts,
    covered: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TitleSitePlayback {
    Active,
    Paused,
    Idle,
}

#[derive(Clone, Copy)]
struct TitleArtFacts {
    protocol: bool,
    halfblock: bool,
    images: bool,
}

#[derive(Clone, Copy)]
struct TitleSlotFacts {
    painted_box: bool,
    visualizer: bool,
    visual_slot_shown: bool,
}

impl App {
    fn log_title_paint(
        &mut self,
        projection: &mut QueueCardProjection,
        identity: &str,
        outcome: &'static str,
        reason: &'static str,
    ) {
        let paint = (identity.to_owned(), outcome);
        if projection.last_title_paint.as_ref() == Some(&paint) {
            return;
        }
        projection.last_title_paint = Some(paint.clone());
        self.queue_card_projection.last_title_paint = Some(paint);
        tracing::debug!(
            name: "queue.title_overlay.paint",
            target: "queue_art",
            item = identity,
            outcome,
            reason,
            "queue title overlay paint"
        );
    }

    pub(in crate::app) fn log_title_decision(
        projection: &mut QueueCardProjection,
        identity: &str,
        item_kind: &str,
        reason: &'static str,
        base_dimensions: Option<(u32, u32)>,
    ) {
        let decision = (identity.to_owned(), reason);
        if projection.last_title_decision.as_ref() == Some(&decision) {
            return;
        }
        projection.last_title_decision = Some(decision);
        let (base_width, base_height) = base_dimensions.unwrap_or_default();
        tracing::debug!(
            name: "queue.title_overlay.decision",
            target: "queue_art",
            item = identity,
            item_kind,
            reason,
            base_width,
            base_height,
            "queue title site decision"
        );
    }
}

fn title_site_facts(
    app: &App,
    projection: &QueueCardProjection,
    playback: mbv_ui_model::playback::PlaybackState,
    covered: bool,
    height: u16,
    width: u16,
) -> TitleSiteFacts {
    TitleSiteFacts {
        playback: if playback.paused {
            TitleSitePlayback::Paused
        } else {
            TitleSitePlayback::Active
        },
        art: TitleArtFacts {
            protocol: app.images.protocol_enabled(),
            halfblock: app.images.is_halfblock_configured(),
            images: projection.images_enabled,
        },
        slot: TitleSlotFacts {
            painted_box: height > 0 && width > 0,
            visualizer: projection.visualizer,
            visual_slot_shown: app.visual_slot_shown(),
        },
        covered,
    }
}

#[derive(Clone, Copy)]
enum ActiveTitleSource {
    Local,
    Slotless,
    NoTitle,
    NoActiveItem,
}

fn active_title_source_skip_reason(source: ActiveTitleSource) -> Option<&'static str> {
    match source {
        ActiveTitleSource::Local | ActiveTitleSource::Slotless => None,
        ActiveTitleSource::NoTitle => Some("NoTitle"),
        ActiveTitleSource::NoActiveItem => Some("NoActiveItem"),
    }
}

fn title_site_skip_reason(
    app: &App,
    projection: &QueueCardProjection,
    playback: mbv_ui_model::playback::PlaybackState,
    item: Option<&QueueItem>,
    slotless_active: bool,
    slotless_title_present: bool,
    height: u16,
    width: u16,
    base_dimensions: Option<(u32, u32)>,
    column_resizing: bool,
) -> Option<&'static str> {
    if !playback.active {
        Some("NotActive")
    } else if projection.visualizer {
        Some("Visualizer")
    } else if !projection.images_enabled {
        Some("ImagesOff")
    } else if column_resizing {
        // User-reported regression: a queue-column resize drag builds a new
        // Lanczos3 overlay variant for every fitted width on the tick thread.
        // While the drag is active the card paints plain base art; the drag's
        // final width composes the overlay once the `DragEnd` clears the gate.
        Some("ColumnResizing")
    } else if app.images.is_halfblock_configured() {
        Some("HalfblockConfigured")
    } else if !app.visual_slot_shown() || projection.cache_key.is_none() {
        Some("NoSlot")
    } else if let Some(reason) = active_title_source_skip_reason(if slotless_active {
        if slotless_title_present {
            ActiveTitleSource::Slotless
        } else {
            ActiveTitleSource::NoTitle
        }
    } else if item.is_some() && playback.active_idx.is_some() {
        ActiveTitleSource::Local
    } else {
        ActiveTitleSource::NoActiveItem
    }) {
        Some(reason)
    } else if base_dimensions.is_none() {
        Some("NoBaseArt")
    } else if height == 0 || width == 0 || !app.images.protocol_enabled() {
        Some("NoBaseProtocolSize")
    } else {
        None
    }
}

fn item_kind(item: &QueueItem) -> &str {
    match item {
        QueueItem::Emby(item) => match item.item_type.as_str() {
            "Movie" => "Movie",
            "Episode" => "Episode",
            "Audio" => "Audio",
            kind => kind,
        },
        QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(_)) => {
            "AudiobookshelfEpisode"
        }
        QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(_)) => "AudiobookshelfBook",
        QueueItem::Feed(_) => "Feed",
    }
}

fn resolve_title_site(
    facts: TitleSiteFacts,
    variant_key: &str,
    painted_key: Option<&str>,
) -> NowPlayingTitleSite {
    let eligible = facts.playback != TitleSitePlayback::Idle
        && facts.art.protocol
        && facts.slot.painted_box
        && !facts.art.halfblock
        && !facts.slot.visualizer
        && facts.slot.visual_slot_shown
        && facts.art.images
        && facts.covered;
    if eligible && painted_key == Some(variant_key) {
        NowPlayingTitleSite::Artwork
    } else {
        NowPlayingTitleSite::Header
    }
}

fn overlay_or_plain_key(plain_key: &str, variant_key: Option<String>) -> String {
    variant_key.unwrap_or_else(|| plain_key.to_owned())
}

fn painted_overlay_key(key: Option<&str>, painted: bool) -> Option<&str> {
    painted
        .then_some(key)
        .flatten()
        .filter(|key| key.contains(":t:"))
}

fn card_image_types(item: &EmbyItem) -> &'static [&'static str] {
    match item.item_type.as_str() {
        "MusicAlbum" => MUSIC_ALBUM_IMAGE_TYPES,
        "Audio" => &["Primary"],
        "Movie" => &["Backdrop", "Primary"],
        // Every other non-music kind fetches its own images poster-first:
        // `Primary` is the item's own still (an episode's still lives on the
        // episode; the server only redirects Thumb/Backdrop/Logo to the
        // series). Guard: user-reported regression, an episode's queue card
        // must show the episode's own art, never the series'.
        _ => &["Primary", "Thumb", "Backdrop", "Logo"],
    }
}

/// The logo owner for the title overlay (design D7): a Movie draws its own
/// Logo, keyed like the Library hero's `movie_logo_source`; an Episode draws
/// its show's Logo. The cache key is independent of any protocol suffix, and
/// the fetch owner keeps the Movie's Logo on the item and the Episode's on the
/// series. Every other item has no logo and keeps the text row.
struct OverlayLogoSource {
    cache_key: String,
    item_id: String,
    series_id: String,
}

fn overlay_logo_source(item: &EmbyItem) -> Option<OverlayLogoSource> {
    match item.item_type.as_str() {
        "Movie" if !item.image_tags.logo.is_empty() => Some(OverlayLogoSource {
            cache_key: format!("{}:Logo:{}", item.id, item.image_tags.logo),
            item_id: item.id.clone(),
            series_id: String::new(),
        }),
        "Episode" if !item.series_id.is_empty() => Some(OverlayLogoSource {
            cache_key: format!("{}:Logo", item.series_id),
            item_id: item.id.clone(),
            series_id: item.series_id.clone(),
        }),
        _ => None,
    }
}

/// The artwork cache key for a queue card. The landscape `{id}:QB` key is
/// Movie-only: the artwork policy also classifies an Episode as Landscape
/// through its series tags, but an episode's card shows the episode's own
/// still, so it keeps the portrait `{id}:P` key (user-reported regression).
fn card_cache_key(item: &EmbyItem) -> String {
    if item.item_type == "Audio" {
        mbv_images::emby_card_cache_key(&item.id, &item.album_id)
    } else if item.item_type == "Movie" {
        mbv_images::emby_queue_landscape_cache_key(&item.id)
    } else {
        mbv_images::emby_card_cache_key(&item.id, "")
    }
}

/// The artwork cache key for an Emby item id held without an `EmbyItem` (a
/// watched remote Session's now-playing item). The landscape key is Movie-only,
/// matching [`card_cache_key`].
fn card_cache_key_for_id(item_id: &str, item_type: Option<&str>) -> String {
    if item_type == Some("Movie") {
        mbv_images::emby_queue_landscape_cache_key(item_id)
    } else {
        mbv_images::emby_card_cache_key(item_id, "")
    }
}

/// The fetch chain for a watched remote Session's now-playing item, which is
/// held without an `EmbyItem` and so cannot consult the artwork policy. Only a
/// Movie is fetched landscape; an Episode fetches its own poster-first chain
/// (`Primary` is the episode's own still, not the series thumb).
fn slotless_card_image_types(item_type: Option<&str>) -> &'static [&'static str] {
    match item_type {
        Some("Movie") => &["Backdrop", "Primary"],
        Some("Episode") => &["Primary", "Thumb", "Backdrop", "Logo"],
        _ => &["Primary"],
    }
}

impl App {
    fn render_card_visualizer(
        &mut self,
        f: &mut Frame,
        area: Rect,
        left_align: bool,
    ) -> (u16, u16, bool) {
        let rect = queue_card_reserved_rect(
            self.images.last_card_size(),
            self.terminal_height,
            area,
            left_align,
        );
        // The row is fixed (it paints the playback panel's band), so the
        // focus bit is not read; the literal keeps that visible at the site.
        let bg = mbv_theme::surface_colors(mbv_theme::Surface::QueueCardVisualizer, false).fill;
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
                self.images.last_card_size(),
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
                .image(key)
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
        let loading = !placeholder_slot && self.images.is_loading(key);
        let last_card = self.images.last_card_size();
        let terminal_height = self.terminal_height;
        let image = self.cached_image_protocol_mut(key);
        let (height, width, loading, painted) = render_card_painting(
            f,
            area,
            left_align,
            &projection,
            loading,
            image,
            last_card,
            terminal_height,
        );
        let fallback_key = projection.plain_cache_key.as_deref().filter(|key| {
            self.images
                .image(key)
                .is_some_and(|entry| entry.img.is_some())
        });
        let mut plain_fallback_painted = false;
        let (height, width, loading) = if painted {
            (height, width, loading)
        } else if let Some(fallback_key) = fallback_key {
            let fallback_loading = self.images.is_loading(fallback_key);
            let fallback_image = self.cached_image_protocol_mut(fallback_key);
            projection.cache_key = Some(fallback_key.to_owned());
            let (height, width, loading, fallback_painted) = render_card_painting(
                f,
                area,
                left_align,
                &projection,
                fallback_loading,
                fallback_image,
                last_card,
                terminal_height,
            );
            plain_fallback_painted = fallback_painted;
            (height, width, loading)
        } else {
            (height, width, loading)
        };
        self.images.record_card_size(height, width);
        if painted && let Some(key) = painted_overlay_key(artwork_key.as_deref(), painted) {
            self.images
                .record_painted_title_overlay(Some(key.to_owned()));
            self.log_title_paint(&mut projection, key, "overlay_recorded", "OverlayPainted");
        } else if plain_fallback_painted {
            let key = artwork_key.as_deref().unwrap_or("<none>");
            let reason = projection
                .last_title_decision
                .as_ref()
                .map_or("OverlayNotReady", |(_, reason)| *reason);
            self.log_title_paint(&mut projection, key, "plain_fallback", reason);
        }
        (height, width, loading)
    }

    fn queue_card_emby_source(
        &self,
        playback: mbv_ui_model::playback::PlaybackState,
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
                .emby_item_at(queue.cursor())
                .cloned()
                .map(|item| (queue.cursor(), item))
        })
    }

    /// A watched remote Session names an item outside the local queue, never the selected row.
    fn project_slotless_session(&mut self, projection: &mut QueueCardProjection) -> bool {
        let Some((item_id, item_type, series_id)) =
            self.connected_session_state.as_ref().and_then(|session| {
                Some((
                    session.now_playing_item_id.clone()?,
                    session.now_playing_item_type.as_deref(),
                    session.now_playing_series_id.clone().unwrap_or_default(),
                ))
            })
        else {
            return true;
        };
        let cache_key = card_cache_key_for_id(&item_id, item_type);
        let image_types = slotless_card_image_types(item_type);
        self.fetch_card_image(cache_key.clone(), item_id, series_id, image_types);
        projection.cache_key = Some(cache_key);
        true
    }

    /// The active/selected slot holds a non-Emby item (or the queue is empty).
    fn project_audiobookshelf_cover(
        &mut self,
        playback: mbv_ui_model::playback::PlaybackState,
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
            queue.item_at(queue.cursor()).cloned()
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
        let prefetch: Vec<(String, String, String, &'static [&'static str])> = queue_ref.slots()
            [start..end]
            .iter()
            .enumerate()
            .filter(|(i, _)| start + i != cursor)
            .filter_map(|(_, slot)| slot.item.as_emby())
            .map(|item| {
                (
                    card_cache_key(item),
                    item.id.clone(),
                    item.series_id.clone(),
                    card_image_types(item),
                )
            })
            .collect();
        for (key, id, series_id, image_types) in prefetch {
            self.fetch_list_card_image_when_idle(key, id, series_id, image_types);
        }
    }

    /// The queue projection issues every fetch for the now-playing item and
    /// projects the slot the painter consumes. Active-first, then viewed selection.
    pub(in crate::app) fn refresh_queue_card_image(&mut self, column_resizing: bool) {
        let mut projection = QueueCardProjection {
            cache_key: None,
            plain_cache_key: None,
            images_enabled: self.images.images_enabled(),
            visualizer: self.visualizer_enabled,
            title_site: mbv_ui_model::playback::NowPlayingTitleSite::Header,
            last_title_decision: self.queue_card_projection.last_title_decision.clone(),
            last_title_paint: self.queue_card_projection.last_title_paint.clone(),
        };
        if projection.visualizer || !projection.images_enabled {
            self.queue_title_site(
                &mut projection,
                self.displayed_playback_state(),
                column_resizing,
            );
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
            self.queue_title_site(&mut projection, playback, column_resizing);
            self.queue_card_projection = projection;
            return;
        };

        let img_types = card_image_types(&item);
        let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
        let cache_key = card_cache_key(&item);
        self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
        self.prefetch_card_images(cursor);
        projection.cache_key = Some(cache_key);
        self.queue_title_site(&mut projection, playback, column_resizing);
        self.queue_card_projection = projection;
    }

    fn queue_title_site(
        &mut self,
        projection: &mut QueueCardProjection,
        playback: mbv_ui_model::playback::PlaybackState,
        column_resizing: bool,
    ) {
        let (height, width) = self.images.last_card_size();
        let slotless_active =
            playback.active && playback.active_idx.is_none() && projection.cache_key.is_some();
        let item = if slotless_active {
            None
        } else {
            playback
                .active_idx
                .and_then(|index| self.playback_queue().item_at(index))
                .or_else(|| {
                    let queue = self.displayed_queue();
                    queue.item_at(queue.cursor())
                })
        };
        let parts = item
            .map(|item| self.playback_title_parts(item))
            .or_else(|| {
                slotless_active
                    .then(|| self.slotless_playback_title_parts())
                    .flatten()
            });
        let item_kind = if slotless_active {
            "Remote".to_owned()
        } else {
            item.map_or_else(|| "Unknown".to_owned(), |item| item_kind(item).to_owned())
        };
        let identity = projection
            .cache_key
            .as_deref()
            .unwrap_or("<none>")
            .to_owned();
        let base_dimensions = projection
            .cache_key
            .as_deref()
            .and_then(|key| self.images.image(key))
            .and_then(|entry| entry.img.as_ref())
            .map(image::GenericImageView::dimensions);
        let reason = title_site_skip_reason(
            self,
            projection,
            playback,
            item,
            slotless_active,
            parts.is_some(),
            height,
            width,
            base_dimensions,
            column_resizing,
        );
        if let Some(reason) = reason {
            Self::log_title_decision(projection, &identity, &item_kind, reason, base_dimensions);
            return;
        }
        let Some(parts) = parts else {
            Self::log_title_decision(
                projection,
                &identity,
                &item_kind,
                "NoTitle",
                base_dimensions,
            );
            return;
        };
        // The logo owner is resolved before the overlay builds so its fetch
        // starts as early as the overlay path itself (design D7); a pending
        // or failed fetch simply leaves `ready_logo_key` empty below.
        let logo_cache_key = item
            .and_then(|item| match item {
                QueueItem::Emby(emby) => overlay_logo_source(emby),
                _ => None,
            })
            .map(|logo| {
                self.fetch_card_image(
                    logo.cache_key.clone(),
                    logo.item_id,
                    logo.series_id,
                    &["Logo"],
                );
                logo.cache_key
            });
        self.queue_title_overlay(
            projection,
            playback,
            &parts,
            &item_kind,
            logo_cache_key.as_deref(),
        );
    }

    fn queue_title_overlay(
        &mut self,
        projection: &mut QueueCardProjection,
        playback: mbv_ui_model::playback::PlaybackState,
        parts: &mbv_queue::PlaybackTitleParts,
        item_kind: &str,
        logo_cache_key: Option<&str>,
    ) {
        let (height, width) = self.images.last_card_size();
        let Some(key) = projection.cache_key.clone() else {
            Self::log_title_decision(projection, "<none>", item_kind, "NoSlot", None);
            return;
        };
        let base_dimensions = self
            .images
            .image(&key)
            .and_then(|entry| entry.img.as_ref())
            .map(image::GenericImageView::dimensions);
        let text = mbv_images::title_overlay::TitleOverlayText {
            context: parts.context.as_ref().map(|part| part.text.as_str()),
            title: &parts.title.text,
        };
        // The `covers` gate applies to the text rows actually drawn (design
        // D7): a ready logo replaces the top row, and a one-part title then
        // draws no text at all.
        let draws_logo = self.images.ready_logo_key(logo_cache_key).is_some();
        let title_covers = if draws_logo {
            text.context.is_none() || mbv_images::title_overlay::covers(text.title)
        } else {
            mbv_images::title_overlay::covers(text.title)
                && text.context.is_none_or(mbv_images::title_overlay::covers)
        };
        let facts = title_site_facts(self, projection, playback, title_covers, height, width);
        if !title_covers {
            Self::log_title_decision(
                projection,
                &key,
                item_kind,
                "UncoveredGlyph",
                base_dimensions,
            );
            return;
        }
        let Some(variant_key) = self.ensure_title_overlay_protocol(
            &key,
            ratatui::layout::Size { width, height },
            parts,
            logo_cache_key,
            projection,
            item_kind,
        ) else {
            return;
        };
        projection.plain_cache_key = Some(key.clone());
        projection.cache_key = Some(overlay_or_plain_key(&key, Some(variant_key.clone())));
        projection.title_site =
            resolve_title_site(facts, &variant_key, self.images.painted_title_overlay_key());
        Self::log_title_decision(
            projection,
            &key,
            item_kind,
            if projection.title_site == NowPlayingTitleSite::Artwork {
                "Artwork"
            } else {
                "NotYetPainted"
            },
            base_dimensions,
        );
    }
}

#[cfg(test)]
mod title_site_tests;
