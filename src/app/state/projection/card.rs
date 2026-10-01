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
    let album_id = if item.item_type == "Audio" {
        item.album_id.as_str()
    } else {
        ""
    };
    mbv_images::emby_card_cache_key(&item.id, album_id)
}

/// The artwork cache key for an Emby item id held without an `EmbyItem` (a
/// watched remote Session's now-playing item).
fn card_cache_key_for_id(item_id: &str) -> String {
    mbv_images::emby_card_cache_key(item_id, "")
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
        self.images.record_card_size(height, width);
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
            title_site: mbv_ui_model::playback::NowPlayingTitleSite::Header,
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
            projection.title_site = self.queue_title_site(&projection, playback);
            self.queue_card_projection = projection;
            return;
        };

        let img_types = card_image_types(&item.item_type);
        let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
        let cache_key = card_cache_key(&item);
        self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
        self.prefetch_card_images(cursor);
        projection.cache_key = Some(cache_key);
        projection.title_site = self.queue_title_site(&projection, playback);
        self.queue_card_projection = projection;
    }

    fn queue_title_site(
        &mut self,
        projection: &QueueCardProjection,
        playback: mbv_ui_model::playback::PlaybackState,
    ) -> mbv_ui_model::playback::NowPlayingTitleSite {
        let (height, width) = self.images.last_card_size();
        let Some(key) = projection.cache_key.as_deref() else {
            return NowPlayingTitleSite::Header;
        };
        let Some(item) = playback
            .active_idx
            .and_then(|index| self.playback_queue().item_at(index))
        else {
            return NowPlayingTitleSite::Header;
        };
        let parts = self.playback_title_parts(item);
        let text = mbv_images::title_overlay::TitleOverlayText {
            context: parts.context.as_ref().map(|part| part.text.as_str()),
            title: &parts.title.text,
        };
        let title_covers = mbv_images::title_overlay::covers(text.title)
            && text.context.is_none_or(mbv_images::title_overlay::covers);
        let facts = TitleSiteFacts {
            playback: match (playback.active, playback.paused) {
                (true, true) => TitleSitePlayback::Paused,
                (true, false) => TitleSitePlayback::Active,
                (false, _) => TitleSitePlayback::Idle,
            },
            art: TitleArtFacts {
                protocol: self.images.protocol_enabled(),
                halfblock: self.images.is_halfblock_configured(),
                images: projection.images_enabled,
            },
            slot: TitleSlotFacts {
                painted_box: height > 0 && width > 0,
                visualizer: projection.visualizer,
                visual_slot_shown: self.visual_slot_shown(),
            },
            covered: title_covers,
        };
        if facts.playback == TitleSitePlayback::Idle
            || !facts.art.protocol
            || !facts.slot.painted_box
            || facts.art.halfblock
            || facts.slot.visualizer
            || !facts.slot.visual_slot_shown
            || !facts.art.images
            || !facts.covered
        {
            return NowPlayingTitleSite::Header;
        }
        let Some(variant_key) = self.ensure_title_overlay_protocol(
            key,
            ratatui::layout::Size { width, height },
            &parts,
        ) else {
            return NowPlayingTitleSite::Header;
        };
        resolve_title_site(facts, &variant_key, self.images.painted_title_overlay_key())
    }
}

#[cfg(test)]
mod title_site_tests {
    use super::{
        NowPlayingTitleSite, TitleArtFacts, TitleSiteFacts, TitleSitePlayback, TitleSlotFacts,
        resolve_title_site,
    };

    const KEY: &str = "art:t:8x4:1";

    fn facts(
        playback: TitleSitePlayback,
        [
            protocol,
            box_ready,
            halfblock,
            visualizer,
            slot_shown,
            images,
            covered,
        ]: [bool; 7],
    ) -> TitleSiteFacts {
        TitleSiteFacts {
            playback,
            art: TitleArtFacts {
                protocol,
                halfblock,
                images,
            },
            slot: TitleSlotFacts {
                painted_box: box_ready,
                visualizer,
                visual_slot_shown: slot_shown,
            },
            covered,
        }
    }

    #[rstest::rstest]
    #[case::active_and_painted(TitleSitePlayback::Active, [true, true, false, false, true, true, true], Some(KEY), NowPlayingTitleSite::Artwork)]
    #[case::paused(TitleSitePlayback::Paused, [true, true, false, false, true, true, true], Some(KEY), NowPlayingTitleSite::Artwork)]
    #[case::not_yet_painted(TitleSitePlayback::Active, [true, true, false, false, true, true, true], None, NowPlayingTitleSite::Header)]
    #[case::halfblock(TitleSitePlayback::Active, [true, true, true, false, true, true, true], Some(KEY), NowPlayingTitleSite::Header)]
    #[case::visualizer(TitleSitePlayback::Active, [true, true, false, true, true, true, true], Some(KEY), NowPlayingTitleSite::Header)]
    #[case::idle_slot(TitleSitePlayback::Idle, [true, true, false, false, false, true, true], Some(KEY), NowPlayingTitleSite::Header)]
    #[case::hidden_slot(TitleSitePlayback::Active, [true, true, false, false, false, true, true], Some(KEY), NowPlayingTitleSite::Header)]
    #[case::zero_slot(TitleSitePlayback::Active, [true, false, false, false, true, true, true], Some(KEY), NowPlayingTitleSite::Header)]
    #[case::no_images(TitleSitePlayback::Active, [false, true, false, false, true, false, true], Some(KEY), NowPlayingTitleSite::Header)]
    #[case::uncovered_glyph(TitleSitePlayback::Active, [true, true, false, false, true, true, false], Some(KEY), NowPlayingTitleSite::Header)]
    fn chooses_site_from_eligibility_and_painted_fact(
        #[case] playback: TitleSitePlayback,
        #[case] conditions: [bool; 7],
        #[case] painted_key: Option<&str>,
        #[case] expected: NowPlayingTitleSite,
    ) {
        assert_eq!(
            resolve_title_site(facts(playback, conditions), KEY, painted_key),
            expected
        );
    }

    #[test]
    fn dim_backdrop_suffix_does_not_change_emby_or_audiobookshelf_identity() {
        use mbv_images::title_overlay::{TitleOverlayText, title_overlay_cache_key};
        let title = TitleOverlayText {
            context: None,
            title: "title",
        };
        let emby = title_overlay_cache_key("item:P", 8, 4, title);
        assert!(emby.starts_with("item:P:t:8x4:"));

        let kitty = title_overlay_cache_key("audiobookshelf:server:cover:item:kitty", 8, 4, title);
        let halfblock =
            title_overlay_cache_key("audiobookshelf:server:cover:item:halfblock", 8, 4, title);
        assert_eq!(kitty, halfblock);
        assert!(kitty.starts_with("audiobookshelf:server:cover:item:t:8x4:"));
    }
}
