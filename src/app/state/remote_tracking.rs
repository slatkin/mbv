use std::time::{Duration, Instant};

pub(in crate::app) struct RemoteTracking {
    pub(in crate::app) direct_remote_connected: bool,
    pub(in crate::app) direct_remote_label: Option<String>,
    /// Emby session id of a remote owner under Direct remote control, so the
    /// Sessions sidebar can still mark that row after the control socket
    /// takes over. Never set for the Stay-alive process: that process is not
    /// a remote session.
    pub(in crate::app) direct_remote_session_id: Option<String>,
    pub(in crate::app) last_session_poll: Instant,
    pub(in crate::app) session_miss_count: u8,
    pub(in crate::app) remote_pos_s: i64,
    pub(in crate::app) remote_pos_at: Instant,
    pub(in crate::app) remote_api_pos_advanced_at: Instant,
    pub(in crate::app) remote_stalled_while_paused: bool,
    pub(in crate::app) remote_seek_pending_until: Instant,
    pub(in crate::app) runtime_zero_since: Option<Instant>,
}

impl RemoteTracking {
    pub(in crate::app) fn new() -> Self {
        Self {
            direct_remote_connected: false,
            direct_remote_label: None,
            direct_remote_session_id: None,
            last_session_poll: Instant::now()
                .checked_sub(Duration::from_secs(60))
                .unwrap_or_else(Instant::now),
            session_miss_count: 0,
            remote_pos_s: 0,
            remote_pos_at: Instant::now(),
            remote_api_pos_advanced_at: Instant::now()
                .checked_sub(Duration::from_secs(60))
                .unwrap_or_else(Instant::now),
            remote_stalled_while_paused: false,
            remote_seek_pending_until: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or_else(Instant::now),
            runtime_zero_since: None,
        }
    }
}
