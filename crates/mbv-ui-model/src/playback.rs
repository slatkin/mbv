#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaybackState {
    /// Whether a transport is active, on any target. A watched remote
    /// Session playing foreign content is active without a local slot.
    pub active: bool,
    /// The playhead's slot in the queue this state was read from. `None`
    /// while a watched remote Session plays something the local queue does
    /// not hold: the transport is active, but no row backs it.
    pub active_idx: Option<usize>,
    pub position_ticks: i64,
    pub runtime_ticks: i64,
    pub paused: bool,
}

/// Which queue an operation refers to.
///
/// `Local` is this TUI instance's own queue and carries local-only metadata:
/// dirty state, undo history, saved-playlist source, and on-disk persistence.
/// `Remote` is the queue owned by a directly-controlled mbv daemon or remote
/// instance. A stale `Remote` UI preference is ignored unless a direct remote
/// queue is actually present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueScope {
    Local,
    Remote,
}

/// Volume change applied by one volume key press or status-bar pill notch.
pub const VOLUME_STEP: i64 = 5;

/// Per-slot render knobs for a queue card, owned by the shell and pushed to
/// the render card painter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueueCardProjection {
    pub cache_key: Option<String>,
    /// Plain artwork used if a projected title-overlay protocol is unavailable.
    pub plain_cache_key: Option<String>,
    pub images_enabled: bool,
    pub visualizer: bool,
    /// Whether the Queue playback panel's header must be painted because the
    /// artwork title overlay is unreachable (design D1): the header carries
    /// the now-playing title whenever the overlay cannot. `false` while the
    /// overlay is merely pending — including a capable setup whose overlay has
    /// composed but not yet painted — and false while idle, which the shell
    /// ORs in (the idle header is governed by the idle rule). Derived from the
    /// decomposed title-site inputs, never from the painted overlay key.
    pub header_visible: bool,
}
