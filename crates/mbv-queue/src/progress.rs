use super::{AudiobookshelfItem, QueueItem};
use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};

const MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS: i64 = 30 * TICKS_PER_SECOND;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressObservation {
    Completed { position_ticks: i64, played: bool },
    Stopped { position_ticks: i64, played: bool },
}

impl ProgressObservation {
    #[must_use]
    pub fn position_to_record(self, item: &QueueItem) -> i64 {
        let (position_ticks, played) = match self {
            Self::Completed {
                position_ticks,
                played,
            } => {
                if played {
                    return 0;
                }
                if position_ticks >= MEANINGFUL_TRACK_COMPLETED_PROGRESS_TICKS && !item.is_audio() {
                    return position_ticks;
                }
                return item.playback_position_ticks();
            }
            Self::Stopped {
                position_ticks,
                played,
            } => (position_ticks, played),
        };
        if played {
            0
        } else if position_ticks > 0 && !item.is_audio() {
            position_ticks
        } else {
            item.playback_position_ticks()
        }
    }

    #[must_use]
    pub fn played(self) -> bool {
        match self {
            Self::Completed { played, .. } | Self::Stopped { played, .. } => played,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SlotProgress {
    pub position_ticks: i64,
    pub played: bool,
}

impl SlotProgress {
    #[must_use]
    pub fn from_item(item: &EmbyItem) -> Self {
        Self {
            position_ticks: item.playback_position_ticks,
            played: item.played,
        }
    }

    /// Progress from any item kind. Feed and Audiobookshelf slots carry
    /// their local position and played state without entering Emby sync.
    pub(crate) fn from_queue_item(item: &QueueItem) -> Self {
        match item {
            QueueItem::Emby(emby) => Self::from_item(emby),
            QueueItem::Feed(entry) => Self {
                position_ticks: entry.position_ticks,
                played: entry.played,
            },
            QueueItem::Audiobookshelf(item) => Self {
                position_ticks: item.playback_position_ticks(),
                played: item.played(),
            },
        }
    }
}

/// Writes a resolved position/played pair into whichever kind `item` is.
/// Called by queue progress mutations and refresh reconciliation.
pub(crate) fn apply_progress_to_queue_item(
    item: &mut QueueItem,
    position_ticks: i64,
    played: bool,
) {
    match item {
        QueueItem::Emby(emby) => {
            emby.playback_position_ticks = position_ticks;
            emby.played = played;
        }
        QueueItem::Feed(entry) => {
            entry.position_ticks = position_ticks;
            entry.played = played;
        }
        QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(episode)) => {
            episode.position_ticks = position_ticks;
            episode.played = played;
            episode.is_finished = played;
        }
        QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book)) => {
            book.position_ticks = position_ticks;
            book.played = played;
            book.is_finished = played;
        }
    }
}
