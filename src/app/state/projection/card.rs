//! The queue card's core part (design D3): the artwork cache keys, the fetch
//! chains, the title overlay's logo owner, the sync-pass fetch issue, and the
//! title-site decision. None of it builds a terminal value, so it stays in
//! `state`; the slot's paint is the shell's (`shell::queue_card`).

use crate::app::App;
use mbv_emby_model::EmbyItem;
use mbv_images::{audiobookshelf_book_cover_cache_key, audiobookshelf_cover_cache_key};
use mbv_queue::QueueItem;
use mbv_ui_model::playback::{NowPlayingTitleSite, QueueCardProjection};

pub(in crate::app) mod artwork;
pub(in crate::app) mod title_site;

use self::artwork::{
    card_cache_key, card_cache_key_for_id, card_image_types, overlay_logo_source,
    overlay_logo_source_for, slotless_card_image_types,
};
use self::title_site::{TitleSiteGate, item_kind, title_site_skip_reason};

fn queue_card_emby_source(
    app: &App,
    playback: mbv_ui_model::playback::PlaybackState,
) -> Option<(usize, EmbyItem)> {
    // The owner's Emby Session output presents content it did not load
    // from `OutputState` alone, never from the active queue slot (design
    // D12). The slotless path owns that card.
    if app.emby_session_now_playing().is_some() {
        return None;
    }
    let slotless_active = playback.active && playback.active_idx.is_none();
    let active = if playback.active {
        playback.active_idx.and_then(|idx| {
            app.playback_queue()
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
        let queue = app.displayed_queue();
        queue
            .emby_item_at(queue.cursor())
            .cloned()
            .map(|item| (queue.cursor(), item))
    })
}

/// Content the owner's Emby Session output presents but the owner did not
/// load names an item outside the local queue, never the selected row.
fn project_slotless_session(app: &mut App, projection: &mut QueueCardProjection) -> bool {
    let Some(now_playing) = app.emby_session_now_playing() else {
        return true;
    };
    let (item_id, item_type, series_id) = (
        now_playing.item_id.clone(),
        now_playing.item_type.clone(),
        now_playing.series_id.clone().unwrap_or_default(),
    );
    let cache_key = card_cache_key_for_id(&item_id, Some(item_type.as_str()));
    let image_types = slotless_card_image_types(Some(item_type.as_str()));
    app.fetch_card_image(cache_key.clone(), item_id, series_id, image_types);
    projection.cache_key = Some(cache_key);
    true
}

/// The active/selected slot holds a non-Emby item (or the queue is empty).
fn project_audiobookshelf_cover(
    app: &mut App,
    playback: mbv_ui_model::playback::PlaybackState,
    projection: &mut QueueCardProjection,
) -> bool {
    let slotless_active = playback.active && playback.active_idx.is_none();
    let raw_item = if playback.active {
        app.queue_item_at_playhead(playback).cloned()
    } else {
        None
    }
    .or_else(|| {
        if slotless_active {
            return None;
        }
        let queue = app.displayed_queue();
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
    let Some(server_url) = app
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
        app.fetch_audiobookshelf_book_cover(server_url.clone(), item_id.clone());
    } else {
        app.fetch_audiobookshelf_cover(server_url.clone(), item_id.clone());
    }
    let cache_key = if is_book {
        audiobookshelf_book_cover_cache_key(&server_url, &item_id, app.image_protocol_suffix())
    } else {
        audiobookshelf_cover_cache_key(&server_url, &item_id, app.image_protocol_suffix())
    };
    projection.cache_key = Some(cache_key);
    true
}

fn prefetch_card_images(app: &mut App, cursor: usize) {
    // Collect data first (releasing the borrow on queue) then call fetch (&mut self).
    const PREFETCH_AHEAD: usize = 3;
    const PREFETCH_BEHIND: usize = 1;
    let queue_ref = app.playback_queue();
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
        app.fetch_list_card_image_when_idle(key, id, series_id, image_types);
    }
}

/// The neutral record this sync's title-site decision leaves for the shell's
/// overlay encoder: the base key, its item, and the parts the shell measures
/// the overlay from. `None` when the site is not `Artwork`.
pub(in crate::app) struct QueueTitleRecord {
    pub base_key: String,
    pub parts: mbv_queue::PlaybackTitleParts,
    pub logo_cache_key: Option<String>,
    pub item_kind: String,
}

/// The item the title site's decision and its parts read: the active slot,
/// else the viewed selection, and nothing on the slotless path.
fn title_site_item(
    app: &App,
    playback: mbv_ui_model::playback::PlaybackState,
    slotless_active: bool,
) -> Option<&QueueItem> {
    if slotless_active {
        return None;
    }
    playback
        .active_idx
        .and_then(|index| app.playback_queue().item_at(index))
        .or_else(|| {
            let queue = app.displayed_queue();
            queue.item_at(queue.cursor())
        })
}

fn queue_title_site(
    app: &mut App,
    projection: &mut QueueCardProjection,
    playback: mbv_ui_model::playback::PlaybackState,
    column_resizing: bool,
) -> Option<QueueTitleRecord> {
    let (height, width) = app.images.last_card_size();
    let slotless_active = (app.emby_session_now_playing().is_some()
        || (playback.active && playback.active_idx.is_none()))
        && projection.cache_key.is_some();
    let item = title_site_item(app, playback, slotless_active);
    let parts = item.map(|item| app.playback_title_parts(item)).or_else(|| {
        slotless_active
            .then(|| app.slotless_playback_title_parts())
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
        .and_then(|key| app.images.image(key))
        .and_then(mbv_images::CachedImage::dimensions);
    let reason = title_site_skip_reason(
        app,
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
            app.title_log_gate
                .log_decision(&identity, &item_kind, reason, base_dimensions);
            projection.title_site = NowPlayingTitleSite::Header;
            return None;
        }
        Some(TitleSiteGate::Transient(reason)) => {
            // Pipeline churn: the site carries over, the header does not
            // resurrect. The overlay rebuilds when the churn passes.
            app.title_log_gate
                .log_decision(&identity, &item_kind, reason, base_dimensions);
            return None;
        }
        None => {}
    }
    let Some(parts) = parts else {
        app.title_log_gate
            .log_decision(&identity, &item_kind, "NoTitle", base_dimensions);
        return None;
    };
    // The logo owner is resolved before the overlay builds so its fetch
    // starts as early as the overlay path itself (design D7); a pending
    // or failed fetch simply leaves `ready_logo_key` empty below. The
    // slotless path derives the same owner from the Emby Session output's
    // now-playing payload (remote-session-overlay-parity, task 3.2;
    // emby-session-owner-output task 7.2).
    let logo_owner = if slotless_active {
        app.emby_session_now_playing().and_then(|now_playing| {
            overlay_logo_source_for(
                &now_playing.item_type,
                &now_playing.item_id,
                now_playing.series_id.as_deref().unwrap_or(""),
                now_playing.logo_tag.as_deref().unwrap_or(""),
            )
        })
    } else {
        item.and_then(|item| match item {
            QueueItem::Emby(emby) => overlay_logo_source(emby),
            _ => None,
        })
    };
    let logo_cache_key = logo_owner.map(|logo| {
        app.fetch_card_image(
            logo.cache_key.clone(),
            logo.item_id,
            logo.series_id,
            &["Logo"],
        );
        logo.cache_key
    });
    queue_title_overlay(
        app,
        projection,
        &parts,
        &item_kind,
        logo_cache_key.as_deref(),
    );
    if projection.title_site != NowPlayingTitleSite::Artwork {
        return None;
    }
    Some(QueueTitleRecord {
        base_key: projection.cache_key.clone()?,
        parts,
        logo_cache_key,
        item_kind,
    })
}

/// The covers gate and the overlay composition: decides the artwork site and,
/// when the glyphs and the measured cell size allow it, composes the overlay
/// variant. It takes the measured cell size as plain numbers, so no terminal
/// value is built here.
fn queue_title_overlay(
    app: &mut App,
    projection: &mut QueueCardProjection,
    parts: &mbv_queue::PlaybackTitleParts,
    item_kind: &str,
    _logo_cache_key: Option<&str>,
) {
    let Some(key) = projection.cache_key.clone() else {
        app.title_log_gate
            .log_decision("<none>", item_kind, "NoSlot", None);
        return;
    };
    let base_dimensions = app
        .images
        .image(&key)
        .and_then(mbv_images::CachedImage::dimensions);
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
        app.title_log_gate
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
    app.title_log_gate
        .log_decision(&key, item_kind, "Artwork", base_dimensions);
}

/// The queue projection issues every fetch for the now-playing item and
/// projects the slot the painter consumes. Active-first, then viewed selection.
///
/// It returns the title-site decision's record. Encoding the overlay variant
/// is the TUI shell's terminal-protocol work, so the shell builds it from this
/// record after the sync (`Model::ensure_queue_card_title_overlay`, design D2
/// rule 3 of `split-tui-gui-clients`), and a Client that paints no protocol
/// ignores it.
pub(in crate::app) fn refresh_queue_card_image(
    app: &mut App,
    column_resizing: bool,
) -> Option<QueueTitleRecord> {
    // The site carries over from the previous sync: transient pipeline
    // churn (a resize drag, an unmeasured protocol size, an encode in
    // flight) must not resurrect the header — only the mode gates below
    // force it back to `Header` (invariant 16's transient class).
    let mut projection = QueueCardProjection {
        cache_key: None,
        plain_cache_key: None,
        images_enabled: app.images.protocol_enabled(),
        visualizer: app.visualizer_enabled,
        title_site: app.queue_card_projection.title_site,
    };
    if projection.visualizer || !projection.images_enabled {
        let record = queue_title_site(
            app,
            &mut projection,
            app.displayed_playback_state(),
            column_resizing,
        );
        app.queue_card_projection = projection;
        return record;
    }

    // Presentation follows a selected-but-unconfirmed slot, switching artwork with its highlight.
    let playback = app.displayed_playback_state();
    // Content the owner's Emby Session output presents but did not load
    // is slotless regardless of the queue's active marker (design D12).
    let slotless_active = app.emby_session_now_playing().is_some()
        || (playback.active && playback.active_idx.is_none());
    let Some((cursor, item)) = queue_card_emby_source(app, playback) else {
        if slotless_active {
            project_slotless_session(app, &mut projection);
        } else {
            project_audiobookshelf_cover(app, playback, &mut projection);
        }
        let record = queue_title_site(app, &mut projection, playback, column_resizing);
        app.queue_card_projection = projection;
        return record;
    };

    let img_types = card_image_types(&item);
    let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
    let cache_key = card_cache_key(&item);
    app.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
    prefetch_card_images(app, cursor);
    projection.cache_key = Some(cache_key);
    let record = queue_title_site(app, &mut projection, playback, column_resizing);
    app.queue_card_projection = projection;
    record
}
