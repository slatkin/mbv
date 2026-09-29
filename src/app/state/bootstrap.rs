use crate::app::state::queue_view::QueueView;

pub(in crate::app) fn bootstrap_legacy_queue() -> LocalDaemonBootstrap {
    LocalDaemonBootstrap {
        local_view: QueueView::empty(),
        last_played_item_id: None,
        last_played_completed: false,
    }
}

pub(in crate::app) fn bootstrap_unified_queue(
    state: &mbv_ctrl::UnifiedQueueStateData,
) -> LocalDaemonBootstrap {
    LocalDaemonBootstrap {
        local_view: QueueView::from_snapshot(state),
        last_played_item_id: None,
        last_played_completed: false,
    }
}

pub(in crate::app) struct LocalDaemonBootstrap {
    pub(in crate::app) local_view: QueueView,
    pub(in crate::app) last_played_item_id: Option<String>,
    pub(in crate::app) last_played_completed: bool,
}
