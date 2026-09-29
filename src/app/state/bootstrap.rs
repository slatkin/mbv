use crate::app::state::queue_view::QueueView;

pub(in crate::app) fn bootstrap_legacy_queue(
    items: Vec<mbv_emby_model::EmbyItem>,
    cursor: usize,
    source: mbv_queue::QueueSource,
) -> LocalDaemonBootstrap {
    LocalDaemonBootstrap {
        local_view: QueueView::from_emby_items(items, cursor),
        queue_source: source,
        last_played_item_id: None,
        last_played_completed: false,
    }
}

pub(in crate::app) fn bootstrap_unified_queue(
    state: &mbv_ctrl::UnifiedQueueStateData,
) -> LocalDaemonBootstrap {
    LocalDaemonBootstrap {
        local_view: QueueView::from_snapshot(state),
        queue_source: state.source.clone(),
        last_played_item_id: None,
        last_played_completed: false,
    }
}

pub(in crate::app) struct LocalDaemonBootstrap {
    pub(in crate::app) local_view: QueueView,
    pub(in crate::app) queue_source: mbv_queue::QueueSource,
    pub(in crate::app) last_played_item_id: Option<String>,
    pub(in crate::app) last_played_completed: bool,
}
