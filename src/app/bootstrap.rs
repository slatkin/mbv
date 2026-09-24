use crate::app::state::types::player_tab::PlayerTab;

pub(super) fn bootstrap_legacy_queue(
    items: Vec<mbv_core::api::EmbyItem>,
    cursor: usize,
    source: crate::config::QueueSource,
) -> LocalDaemonBootstrap {
    LocalDaemonBootstrap {
        player_tab: PlayerTab::from_emby_items(items, cursor),
        queue_source: source,
        last_played_item_id: None,
        last_played_completed: false,
    }
}

pub(super) fn bootstrap_unified_queue(
    state: &mbv_core::ctrl::UnifiedQueueStateData,
) -> LocalDaemonBootstrap {
    LocalDaemonBootstrap {
        player_tab: PlayerTab::from_unified_state(state),
        queue_source: state.source.clone(),
        last_played_item_id: None,
        last_played_completed: false,
    }
}

pub(super) struct LocalDaemonBootstrap {
    pub(super) player_tab: PlayerTab,
    pub(super) queue_source: crate::config::QueueSource,
    pub(super) last_played_item_id: Option<String>,
    pub(super) last_played_completed: bool,
}
