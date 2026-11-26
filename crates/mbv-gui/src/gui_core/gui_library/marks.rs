//! The GUI core's deadline and playhead reads (`gui-change-driven-sections`
//! design D3, rules 3 and 4). The core compares them on each tick and marks the
//! gated Library section when one changes. Only the GUI core calls them; the
//! TUI never does, so they stay out of its tick.
//!
//! The deadline and playhead facts are core reads — they read no GUI state —
//! so they are free functions over the `App` a caller holds. The GUI core
//! calls them from its own `impl GuiCore` block.

use std::time::Instant;

use mbv_queue::{AudiobookshelfItem, QueueItem};

use super::feeds_projection::feed_entry_row_id;
use crate::gui_core::GuiCore;
use mbv_client_core::App;

/// The playhead facts whose change marks the Library section (design D3 rule
/// 4). The current chapter is not here: the window derives it from the
/// position (design D5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiPlayheadMarks {
    /// The Emby item playing at the playhead (`gui_playing_track_id`).
    pub music_track: Option<String>,
    /// The Audiobooks book playing at the playhead. `None` while the active
    /// slot is pending, so a pending book marks nothing (`playing_book_chapter`'s
    /// rule).
    pub audiobook: Option<String>,
    /// The encoded feed entry playing at the playhead, in the form the Feeds
    /// rows carry (`GuiFeedEntryId::encode`). `None` while no feed entry plays.
    pub feed_entry: Option<String>,
}

impl GuiCore {
    /// Whether the navigation-idle gate is open (design D3 rule 3): the core
    /// watches the change from not idle to idle; that change is the passed
    /// deadline.
    #[must_use]
    pub fn gui_navigation_idle(&self) -> bool {
        navigation_idle(&self.app)
    }

    /// Whether an interrupted art fetch's retry time falls in `(after, until]`
    /// (design D3 rule 3, invariant 27). The core keeps the window's start.
    #[must_use]
    pub fn gui_art_retry_due(&self, after: Instant, until: Instant) -> bool {
        art_retry_due(&self.app, after, until)
    }

    /// The playing Music track and the playing Audiobooks book (design D3 rule
    /// 4). Both change only with the active slot, so the core compares them on
    /// each tick and marks on a change.
    #[must_use]
    pub fn gui_playhead_marks(&self) -> GuiPlayheadMarks {
        playhead_marks(&self.app)
    }
}

/// Whether the navigation-idle gate is open (design D3 rule 3): the
/// warm-up of the Hero's neighbours waits on it. The core watches the
/// change from not idle to idle; that change is the passed deadline.
#[must_use]
pub fn navigation_idle(app: &App) -> bool {
    app.navigation_idle()
}

/// Whether an interrupted art fetch's retry time falls in `(after, until]`
/// (design D3 rule 3, invariant 27). The core keeps the window's start: each
/// tick asks for the span since its previous tick, so a retry is reported
/// once.
#[must_use]
pub fn art_retry_due(app: &App, after: Instant, until: Instant) -> bool {
    app.image_cache().retry_passed_between(after, until)
}

/// The playing Music track and the playing Audiobooks book (design D3 rule
/// 4). Both change only with the active slot, so the core compares them on
/// each tick and marks on a change.
#[must_use]
pub fn playhead_marks(app: &App) -> GuiPlayheadMarks {
    GuiPlayheadMarks {
        music_track: super::playing_track_id_for_shell(app),
        audiobook: playing_book_id(app),
        feed_entry: playing_feed_entry_id(app),
    }
}

/// The queue item at the displayed playhead, or `None` while idle. Each
/// playing-identity read below names its own kind of item from it.
fn playhead_item(app: &App) -> Option<&QueueItem> {
    let state = app.displayed_playback_state();
    if !state.active {
        return None;
    }
    app.queue_item_at_playhead(state)
}

/// The id of the Emby episode at the playhead (design D6), or `None` while no
/// episode plays. A Shows row carries this id to mark itself playing.
pub(crate) fn playing_episode_id(app: &App) -> Option<String> {
    match playhead_item(app) {
        Some(QueueItem::Emby(item)) if item.item_type == "Episode" => Some(item.id.clone()),
        Some(_) | None => None,
    }
}

/// The encoded id of the feed entry at the playhead (design D6), in the form
/// the Feeds rows carry, or `None` while no feed entry plays.
pub(crate) fn playing_feed_entry_id(app: &App) -> Option<String> {
    match playhead_item(app) {
        Some(QueueItem::Feed(entry)) => Some(feed_entry_row_id(entry)),
        Some(_) | None => None,
    }
}

/// The library item id of the Audiobooks book at the playhead, or `None`
/// while no book plays or the active slot is pending. The playhead marks
/// sample it (design D3 rule 4), and the Audiobooks projection carries it
/// into the window's detail as the `playing_book_id` fact (design D5).
pub(crate) fn playing_book_id(app: &App) -> Option<String> {
    let state = app.displayed_playback_state();
    if !state.active {
        return None;
    }
    if app.pending_playback_slot().is_some() && state != app.effective_playback_state() {
        return None;
    }
    match app.queue_item_at_playhead(state) {
        Some(QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book))) => {
            Some(book.library_item_id.clone())
        }
        Some(_) | None => None,
    }
}
