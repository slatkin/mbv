use super::types_player_tab::PlayerTab;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;

pub(super) fn bootstrap_unified_queue(
    state: &mbv_core::ctrl::UnifiedQueueStateData,
) -> LocalDaemonBootstrap {
    LocalDaemonBootstrap {
        player_tab: PlayerTab::from_unified_state(state),
        queue_source: state.source.clone(),
        last_played_item_id: None,
        last_played_completed: false,
        adopt_queue: None,
        positions: Default::default(),
    }
}

pub(super) struct LocalDaemonBootstrap {
    pub(super) player_tab: PlayerTab,
    pub(super) queue_source: crate::config::QueueSource,
    pub(super) last_played_item_id: Option<String>,
    pub(super) last_played_completed: bool,
    pub(super) adopt_queue: Option<(Vec<QueueItem>, usize, crate::config::QueueSource)>,
    /// Per-item resume positions carried over from the saved queue snapshot
    /// (see `QueueState::positions`), so the same best-effort enrichment that
    /// `restore_queue_state` performs for plain local playback also happens
    /// for a cold daemon adopting a saved queue. Empty when there's nothing
    /// to enrich (remote-populated queue, or no saved state).
    pub(super) positions: std::collections::HashMap<String, i64>,
}

pub(super) fn bootstrap_local_daemon_queue(
    remote_items: Vec<EmbyItem>,
    remote_cursor: usize,
    remote_source: crate::config::QueueSource,
    saved_state: Option<crate::config::QueueState>,
) -> LocalDaemonBootstrap {
    if !remote_items.is_empty() {
        let queue_items: Vec<QueueItem> = remote_items
            .into_iter()
            .map(|i| QueueItem::Emby(Box::new(i)))
            .collect();
        return LocalDaemonBootstrap {
            player_tab: PlayerTab::new(queue_items.clone(), remote_cursor),
            queue_source: remote_source,
            last_played_item_id: None,
            last_played_completed: false,
            adopt_queue: None,
            positions: Default::default(),
        };
    }

    let Some(state) = saved_state.filter(|state| !state.items.is_empty()) else {
        return LocalDaemonBootstrap {
            player_tab: PlayerTab::new(Vec::new(), 0),
            queue_source: remote_source,
            last_played_item_id: None,
            last_played_completed: false,
            adopt_queue: None,
            positions: Default::default(),
        };
    };

    let queue_items = state.items;
    let cursor = super::actions::queue_restore_cursor(
        &queue_items,
        state.cursor,
        state.last_played_item_id.as_deref(),
        state.last_played_completed,
    );
    LocalDaemonBootstrap {
        player_tab: PlayerTab::new(queue_items.clone(), cursor),
        queue_source: state.source.clone(),
        last_played_item_id: state.last_played_item_id.clone(),
        last_played_completed: state.last_played_completed,
        adopt_queue: Some((queue_items, cursor, state.source)),
        positions: state.positions,
    }
}
