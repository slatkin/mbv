//! The queue card's artwork rules: the cache keys and fetch chains for a queue
//! item, the logo owner for the title overlay, and the title overlay's
//! log-dedup memo. These build no ratatui value, so the core keeps them; the
//! projection that reads them is the shell's (`queue_card.rs`), and the GUI
//! Home projection re-reads the same chains (design D12).

use crate::app::App;
use mbv_emby_model::EmbyItem;
use mbv_ui_model::playback::MUSIC_ALBUM_IMAGE_TYPES;

/// Log-dedup memo for the title overlay's decision and paint lines; shell
/// state, not presentation.
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct TitleLogGate {
    decision: Option<(String, &'static str)>,
    paint: Option<(String, &'static str)>,
}

impl App {
    /// The artwork log gate, read and written by every title-overlay path.
    pub(in crate::app) fn title_log_gate(&mut self) -> &mut TitleLogGate {
        &mut self.title_log_gate
    }

    /// The decision reason the last logged decision recorded.
    pub(in crate::app) fn title_log_decision_reason(&self) -> Option<&'static str> {
        self.title_log_gate.last_decision_reason()
    }
}

impl TitleLogGate {
    pub(in crate::app) fn last_decision_reason(&self) -> Option<&'static str> {
        self.decision.as_ref().map(|(_, reason)| *reason)
    }

    pub(in crate::app) fn log_paint(
        &mut self,
        identity: &str,
        outcome: &'static str,
        reason: &'static str,
    ) {
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

/// The artwork chain the fetch issues for `item` (the queue card's
/// conventions), re-read by the GUI Home projection's own art (design D12:
/// the same chain the now-playing card fetches, so a Home row whose item
/// plays shares its cache entry).
pub(in crate::app) fn card_image_types(item: &EmbyItem) -> &'static [&'static str] {
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
pub(in crate::app) struct OverlayLogoSource {
    pub(in crate::app) cache_key: String,
    pub(in crate::app) item_id: String,
    pub(in crate::app) series_id: String,
}

/// The key rules both the queue-item path and the Emby Session output's
/// slotless path share (remote-session-overlay-parity, task 3.1): a Movie's
/// logo key carries its image etag, an Episode's show logo keys on the series
/// id alone. `logo_etag` is empty when the payload carries none, which leaves
/// a Movie with no resolvable logo reference.
pub(in crate::app) fn overlay_logo_source_for(
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

pub(in crate::app) fn overlay_logo_source(item: &EmbyItem) -> Option<OverlayLogoSource> {
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
/// The artwork cache key the card fetch issues for `item` (the queue card's
/// conventions), re-read by the GUI Home projection's own art (design D12).
pub(in crate::app) fn card_cache_key(item: &EmbyItem) -> String {
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
pub(in crate::app) fn card_cache_key_for_id(item_id: &str, item_type: Option<&str>) -> String {
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
pub(in crate::app) fn slotless_card_image_types(
    item_type: Option<&str>,
) -> &'static [&'static str] {
    match item_type {
        Some("Movie") => &["Backdrop", "Primary"],
        Some("Episode") => &["Primary", "Thumb", "Backdrop", "Logo"],
        _ => &["Primary"],
    }
}
