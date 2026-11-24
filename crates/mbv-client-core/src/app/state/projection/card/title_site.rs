//! The title-site decision: why this sync's eligibility gates stop the artwork
//! from carrying the now-playing title, and what the header does about it
//! (invariant 16). It builds no terminal value, so it lives in `state`; the
//! slot's paint is the shell's (`shell::queue_card`).

use crate::app::App;
use mbv_queue::QueueItem;
use mbv_ui_model::playback::QueueCardProjection;

/// Why the title-site decision stops this sync, and what the header does
/// about it. `ForceHeader` gates are stable presentation facts in which the
/// header is the title's home (idle, images off, no art will ever exist, a
/// font that cannot render the title); `Transient` gates are pipeline churn
/// — a resize drag, a not-yet-measured protocol size — that must not
/// resurrect the header: the site carries over until the churn passes.
pub(in crate::app) enum TitleSiteGate {
    ForceHeader(&'static str),
    Transient(&'static str),
}

pub(in crate::app) fn title_site_skip_reason(
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
        // Invariant 20: a resize drag would build a new Lanczos3 overlay
        // variant for every fitted width on the tick thread. While the hold
        // is on the card paints plain base art; the settled width composes
        // the overlay once the hold lifts. The header stays hidden across
        // the drag.
        Some(TitleSiteGate::Transient("ResizeDragHold"))
    } else if app.halfblock_configured() {
        Some(TitleSiteGate::ForceHeader("HalfblockConfigured"))
    } else if !app.visual_slot_shown() || projection.cache_key.is_none() {
        Some(TitleSiteGate::ForceHeader("NoSlot"))
    } else if slotless_active && !slotless_title_present {
        Some(TitleSiteGate::ForceHeader("NoTitle"))
    } else if !(slotless_active || item.is_some() && playback.active_idx.is_some()) {
        Some(TitleSiteGate::ForceHeader("NoActiveItem"))
    } else if projection.cache_key.as_deref().is_some_and(|key| {
        app.images.is_fetch_interrupted(key)
            || app.images.image(key).is_some_and(|entry| !entry.has_art())
    }) {
        // The fetch resolved empty, or got no answer and may never: no art
        // carries the title, so the header stays its home. A pending fetch
        // (entry still absent) falls through — eligibility is decided before
        // the art arrives.
        Some(TitleSiteGate::ForceHeader("NoBaseArt"))
    } else if height == 0 || width == 0 {
        // The protocol's cell size is not measured yet (before the first
        // card paint). Churn, not a presentation fact — the site carries.
        Some(TitleSiteGate::Transient("NoBaseProtocolSize"))
    } else {
        None
    }
}

pub(in crate::app) fn item_kind(item: &QueueItem) -> &str {
    match item {
        QueueItem::Emby(item) => item.item_type.as_str(),
        QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(_)) => {
            "AudiobookshelfEpisode"
        }
        QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Book(_)) => "AudiobookshelfBook",
        QueueItem::Feed(_) => "Feed",
        QueueItem::Twitch(_) => "Twitch",
    }
}
