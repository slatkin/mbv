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

/// Log-dedup memo for the title overlay's decision and paint lines; shell
/// state, not presentation.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct TitleLogGate {
    decision: Option<(String, &'static str)>,
    paint: Option<(String, &'static str)>,
}

impl TitleLogGate {
    pub(in crate::app) fn last_decision_reason(&self) -> Option<&'static str> {
        self.decision.as_ref().map(|(_, reason)| *reason)
    }

    fn log_paint(&mut self, identity: &str, outcome: &'static str, reason: &'static str) {
        let paint = (identity.to_owned(), outcome);
        if self.paint.as_ref() == Some(&paint) {
            return;
        }
        self.paint = Some(paint);
        tracing::debug!(
            name: "queue.title_overlay.paint",
            target: "queue_art",
            item = identity,
            outcome,
            reason,
            "queue title overlay paint"
        );
    }

    pub(in crate::app) fn log_decision(
        &mut self,
        identity: &str,
        item_kind: &str,
        reason: &'static str,
        base_dimensions: Option<(u32, u32)>,
    ) {
        let decision = (identity.to_owned(), reason);
        if self.decision.as_ref() == Some(&decision) {
            return;
        }
        self.decision = Some(decision);
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

/// Why the title-site decision stops this sync, and what the header does
/// about it. `ForceHeader` gates are stable presentation facts in which the
/// header is the title's home (idle, images off, no art will ever exist, a
/// font that cannot render the title); `Transient` gates are pipeline churn
/// — a resize drag, a not-yet-measured protocol size — that must not
/// resurrect the header: the site carries over until the churn passes.
enum TitleSiteGate {
    ForceHeader(&'static str),
    Transient(&'static str),
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
    column_resizing: bool,
) -> Option<TitleSiteGate> {
    if !playback.active {
        Some(TitleSiteGate::ForceHeader("NotActive"))
    } else if projection.visualizer {
        Some(TitleSiteGate::ForceHeader("Visualizer"))
    } else if !projection.images_enabled {
        Some(TitleSiteGate::ForceHeader("ImagesOff"))
    } else if column_resizing {
        // User-reported regression: a queue-column resize drag builds a new
        // Lanczos3 overlay variant for every fitted width on the tick thread.
        // While the drag is active the card paints plain base art; the drag's
        // final width composes the overlay once the `DragEnd` clears the gate.
        // The header stays hidden across the drag.
        Some(TitleSiteGate::Transient("ColumnResizing"))
    } else if app.images.is_halfblock_configured() {
        Some(TitleSiteGate::ForceHeader("HalfblockConfigured"))
    } else if !app.visual_slot_shown() || projection.cache_key.is_none() {
        Some(TitleSiteGate::ForceHeader("NoSlot"))
    } else if slotless_active && !slotless_title_present {
        Some(TitleSiteGate::ForceHeader("NoTitle"))
    } else if !(slotless_active || item.is_some() && playback.active_idx.is_some()) {
        Some(TitleSiteGate::ForceHeader("NoActiveItem"))
    } else if projection.cache_key.as_deref().is_some_and(|key| {
        app.images
            .image(key)
            .is_some_and(|entry| entry.img.is_none())
    }) {
        // The fetch resolved empty: no art will ever exist to carry the
        // title, so the header stays its home. A pending fetch (entry still
        // absent) falls through — eligibility is decided before the art
        // arrives.
        Some(TitleSiteGate::ForceHeader("NoBaseArt"))
    } else if height == 0 || width == 0 {
        // The protocol's cell size is not measured yet (before the first
        // card paint). Churn, not a presentation fact — the site carries.
        Some(TitleSiteGate::Transient("NoBaseProtocolSize"))
    } else {
        None
    }
}

fn item_kind(item: &QueueItem) -> &str {
    match item {
        QueueItem::Emby(item) => item.item_type.as_str(),
        QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(_)) => {
            "AudiobookshelfEpisode"
        }
        QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(_)) => "AudiobookshelfBook",
        QueueItem::Feed(_) => "Feed",
    }
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

/// The key rules both the queue-item path and the watched remote session's
/// slotless path share (remote-session-overlay-parity, task 3.1): a Movie's
/// logo key carries its image etag, an Episode's show logo keys on the series
/// id alone. `logo_etag` is empty when the payload carries none, which leaves
/// a Movie with no resolvable logo reference.
fn overlay_logo_source_for(
    item_type: &str,
    item_id: &str,
    series_id: &str,
    logo_etag: &str,
) -> Option<OverlayLogoSource> {
    match item_type {
        "Movie" if !logo_etag.is_empty() => Some(OverlayLogoSource {
            cache_key: format!("{item_id}:Logo:{logo_etag}"),
            item_id: item_id.to_owned(),
            series_id: String::new(),
        }),
        "Episode" if !series_id.is_empty() => Some(OverlayLogoSource {
            cache_key: format!("{series_id}:Logo"),
            item_id: item_id.to_owned(),
            series_id: series_id.to_owned(),
        }),
        _ => None,
    }
}

fn overlay_logo_source(item: &EmbyItem) -> Option<OverlayLogoSource> {
    match item.item_type.as_str() {
        "Movie" | "Episode" => overlay_logo_source_for(
            item.item_type.as_str(),
            &item.id,
            &item.series_id,
            &item.image_tags.logo,
        ),
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
        let mut out = (height, width, loading);
        if !painted && let Some(fallback_key) = fallback_key {
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
            out = (height, width, loading);
        }
        let (height, width, loading) = out;
        // A dimmed backdrop paints through the halfblock fallback (#451),
        // whose hardcoded font grid measures cells differently than the
        // configured protocol's detected font. Recording that paint's size
        // would move the geometry checkpoint — resizing the reservation and
        // pushing the queue rows below the slot — every time a modal or
        // context menu opens, and back when it closes. The checkpoint stays
        // on the configured protocol's last paint; closing the modal
        // repaints the warm configured protocol at that same size.
        if !self.dim_halfblock_forced() {
            self.images.record_card_size(height, width);
        }
        if painted
            && let Some(key) = artwork_key
                .as_deref()
                .filter(|key| key.contains(mbv_images::title_overlay::DERIVED_SEP))
        {
            self.title_log_gate
                .log_paint(key, "overlay_recorded", "OverlayPainted");
        } else if plain_fallback_painted {
            let key = artwork_key.as_deref().unwrap_or("<none>");
            let reason = self
                .title_log_gate
                .last_decision_reason()
                .unwrap_or("OverlayNotReady");
            self.title_log_gate.log_paint(key, "plain_fallback", reason);
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
        // The site carries over from the previous sync: transient pipeline
        // churn (a resize drag, an unmeasured protocol size, an encode in
        // flight) must not resurrect the header — only the mode gates below
        // force it back to `Header` (invariant 16's transient class).
        let mut projection = QueueCardProjection {
            cache_key: None,
            plain_cache_key: None,
            images_enabled: self.images.images_enabled(),
            visualizer: self.visualizer_enabled,
            title_site: self.queue_card_projection.title_site,
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
            column_resizing,
        );
        match reason {
            Some(TitleSiteGate::ForceHeader(reason)) => {
                self.title_log_gate
                    .log_decision(&identity, &item_kind, reason, base_dimensions);
                projection.title_site = NowPlayingTitleSite::Header;
                return;
            }
            Some(TitleSiteGate::Transient(reason)) => {
                // Pipeline churn: the site carries over, the header does not
                // resurrect. The overlay rebuilds when the churn passes.
                self.title_log_gate
                    .log_decision(&identity, &item_kind, reason, base_dimensions);
                return;
            }
            None => {}
        }
        let Some(parts) = parts else {
            self.title_log_gate
                .log_decision(&identity, &item_kind, "NoTitle", base_dimensions);
            return;
        };
        // The logo owner is resolved before the overlay builds so its fetch
        // starts as early as the overlay path itself (design D7); a pending
        // or failed fetch simply leaves `ready_logo_key` empty below. The
        // slotless path derives the same owner from the watched session's
        // now-playing payload (remote-session-overlay-parity, task 3.2).
        let logo_owner = if slotless_active {
            self.connected_session_state.as_ref().and_then(|session| {
                overlay_logo_source_for(
                    session.now_playing_item_type.as_deref().unwrap_or(""),
                    session.now_playing_item_id.as_deref().unwrap_or(""),
                    session.now_playing_series_id.as_deref().unwrap_or(""),
                    session.now_playing_logo_etag.as_deref().unwrap_or(""),
                )
            })
        } else {
            item.and_then(|item| match item {
                QueueItem::Emby(emby) => overlay_logo_source(emby),
                _ => None,
            })
        };
        let logo_cache_key = logo_owner.map(|logo| {
            self.fetch_card_image(
                logo.cache_key.clone(),
                logo.item_id,
                logo.series_id,
                &["Logo"],
            );
            logo.cache_key
        });
        self.queue_title_overlay(projection, &parts, &item_kind, logo_cache_key.as_deref());
    }

    fn queue_title_overlay(
        &mut self,
        projection: &mut QueueCardProjection,
        parts: &mbv_queue::PlaybackTitleParts,
        item_kind: &str,
        logo_cache_key: Option<&str>,
    ) {
        let (height, width) = self.images.last_card_size();
        let Some(key) = projection.cache_key.clone() else {
            self.title_log_gate
                .log_decision("<none>", item_kind, "NoSlot", None);
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
        // The site's covers gate is the strict, logo-free rule: both parts'
        // glyphs must be renderable by the embedded font. It is
        // size- and logo-arrival-independent, so the decision cannot flip
        // when a logo fetch lands mid-playback — that flip was a header
        // disappearance flash. Whether the composed variant then draws a
        // logo instead of a text row is the painter's business (design D7),
        // not the site's.
        let title_covers = mbv_images::title_overlay::covers(text.title)
            && text.context.is_none_or(mbv_images::title_overlay::covers);
        if !title_covers {
            self.title_log_gate
                .log_decision(&key, item_kind, "UncoveredGlyph", base_dimensions);
            // A font that cannot render the title is a stable fact: the
            // header is the title's home for as long as it holds.
            projection.title_site = NowPlayingTitleSite::Header;
            return;
        }
        // The site is decided here, in the sync pass, before the panel first
        // displays, so the header's brand row and the artwork's overlay agree
        // from the first frame of eligible playback (invariant 16). A variant
        // that is not encoded yet (art still fetching, encode in flight)
        // paints plain art for those frames while the header already shows
        // the playing brand row.
        projection.title_site = NowPlayingTitleSite::Artwork;
        if let Some(variant_key) = self.ensure_title_overlay_protocol(
            &key,
            ratatui::layout::Size { width, height },
            parts,
            logo_cache_key,
            item_kind,
        ) {
            projection.plain_cache_key = Some(key.clone());
            projection.cache_key = Some(variant_key);
        }
        self.title_log_gate
            .log_decision(&key, item_kind, "Artwork", base_dimensions);
    }
}

#[cfg(test)]
mod queue_slot_tests;

#[cfg(test)]
mod title_site_tests;
