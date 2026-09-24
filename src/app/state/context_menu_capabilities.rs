//! Capability derivation for multi-item context menus.
use mbv_core::playback_queue::{QueueItem, QueueItemKind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ItemCapabilities {
    pub playable: bool,
    pub queue_admissible: bool,
    pub removable: bool,
    pub played_state_capable: bool,
}

impl ItemCapabilities {
    pub(crate) fn intersect(self, other: Self) -> Self {
        Self {
            playable: self.playable && other.playable,
            queue_admissible: self.queue_admissible && other.queue_admissible,
            removable: self.removable && other.removable,
            played_state_capable: self.played_state_capable && other.played_state_capable,
        }
    }
}

pub(crate) fn queue_item_capabilities(item: &QueueItem) -> ItemCapabilities {
    ItemCapabilities {
        playable: item.is_audio() || item.is_video(),
        queue_admissible: true,
        removable: true,
        played_state_capable: item.kind() == QueueItemKind::Emby,
    }
}

pub(crate) fn emby_item_capabilities(item: &mbv_core::api::EmbyItem) -> ItemCapabilities {
    let playable = crate::app::ui_util::is_playable(item) && !item.is_folder;
    let played_state_capable = item.media_type != "Audio" && item.item_type != "Audio";
    ItemCapabilities {
        playable,
        queue_admissible: playable,
        removable: false,
        played_state_capable,
    }
}

pub(crate) fn intersect<I>(items: I) -> Option<ItemCapabilities>
where
    I: IntoIterator<Item = ItemCapabilities>,
{
    let mut iter = items.into_iter();
    let first = iter.next()?;
    Some(iter.fold(first, ItemCapabilities::intersect))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intersection_drops_capabilities_not_shared() {
        let all = ItemCapabilities {
            playable: true,
            queue_admissible: true,
            removable: true,
            played_state_capable: true,
        };
        let no_played = ItemCapabilities {
            played_state_capable: false,
            ..all
        };
        let result = intersect([all, no_played]).unwrap();
        assert!(!result.played_state_capable);
        assert!(result.removable);
    }

    #[test]
    fn folder_or_non_playable_selection_suppresses_playback_actions() {
        let playable = ItemCapabilities {
            playable: true,
            queue_admissible: true,
            ..Default::default()
        };
        let folder = ItemCapabilities::default();
        let result = intersect([playable, folder]).unwrap();
        assert!(!result.playable);
        assert!(!result.queue_admissible);
    }

    #[test]
    fn mixed_queue_selection_keeps_remove_but_drops_played_state() {
        let emby = ItemCapabilities {
            removable: true,
            played_state_capable: true,
            ..Default::default()
        };
        let feed = ItemCapabilities {
            removable: true,
            ..Default::default()
        };
        let result = intersect([emby, feed]).unwrap();
        assert!(result.removable);
        assert!(!result.played_state_capable);
    }
}
