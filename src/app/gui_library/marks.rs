//! The GUI core's deadline and playhead reads (`gui-change-driven-sections`
//! design D3, rules 3 and 4). The core compares them on each tick and marks the
//! gated Library section when one changes. Only the GUI core calls them; the
//! TUI never does, so they stay out of its tick.

use std::time::Instant;

use mbv_queue::{AudiobookshelfItem, QueueItem};

use crate::app::App;

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
}

impl App {
    /// Whether the navigation-idle gate is open (design D3 rule 3): the
    /// warm-up of the Hero's neighbours waits on it. The core watches the
    /// change from not idle to idle; that change is the passed deadline.
    #[must_use]
    pub fn gui_navigation_idle(&self) -> bool {
        self.navigation_idle()
    }

    /// Whether an interrupted art fetch's retry time falls in `(after, until]`
    /// (design D3 rule 3, invariant 27). The core keeps the window's start: each
    /// tick asks for the span since its previous tick, so a retry is reported
    /// once.
    #[must_use]
    pub fn gui_art_retry_due(&self, after: Instant, until: Instant) -> bool {
        self.images.retry_passed_between(after, until)
    }

    /// The playing Music track and the playing Audiobooks book (design D3 rule
    /// 4). Both change only with the active slot, so the core compares them on
    /// each tick and marks on a change.
    #[must_use]
    pub fn gui_playhead_marks(&self) -> GuiPlayheadMarks {
        GuiPlayheadMarks {
            music_track: self.gui_playing_track_id(),
            audiobook: self.gui_playing_book_id(),
        }
    }

    /// The library item id of the Audiobooks book at the playhead, or `None`
    /// while no book plays or the active slot is pending. The playhead marks
    /// sample it (design D3 rule 4), and the Audiobooks projection carries it
    /// into the window's detail as the `playing_book_id` fact (design D5).
    pub(in crate::app) fn gui_playing_book_id(&self) -> Option<String> {
        let state = self.displayed_playback_state();
        if !state.active {
            return None;
        }
        if self.pending_playback_slot().is_some() && state != self.effective_playback_state() {
            return None;
        }
        match self.queue_item_at_playhead(state) {
            Some(QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book))) => {
                Some(book.library_item_id.clone())
            }
            Some(_) | None => None,
        }
    }
}
