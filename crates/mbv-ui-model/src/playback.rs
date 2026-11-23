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
/// `Peer` is the queue owned by a directly-controlled mbv daemon or remote
/// instance. A stale `Peer` UI preference is ignored unless a direct remote
/// queue is actually present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueLink {
    Local,
    Peer,
}

/// Volume change applied by one volume key press or status-bar pill notch.
pub const VOLUME_STEP: i64 = 5;

/// Where the now-playing title is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NowPlayingTitleSite {
    #[default]
    Header,
    Artwork,
}

/// Per-slot render knobs for a queue card, owned by the shell and pushed to
/// the render card painter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueueCardProjection {
    pub cache_key: Option<String>,
    /// Plain artwork used if a projected title-overlay protocol is unavailable.
    pub plain_cache_key: Option<String>,
    pub images_enabled: bool,
    pub visualizer: bool,
    pub title_site: NowPlayingTitleSite,
}

/// The projection's image state for one hero (task 5.10, design D9): the
/// fetch/encode runs in the shell projection; painting reads this state only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeroImageState {
    /// No image will arrive (no artwork source, images disabled, or a fetch
    /// that resolved empty): the shared placeholder is final.
    None,
    /// A fetch/encode for the artwork source is in flight: the painters
    /// reserve the box with the shared placeholder.
    Loading,
    /// The image is cached under `cache_key` (cover-fit keyed by the Wide
    /// header's projected box); the painters reserve the box and the shell
    /// paints the protocol into it after view, showing the placeholder at
    /// most one frame while an encode completes. `decoded` is the cached
    /// cached source image's pixel size for aspect-aware artwork placement.
    Ready {
        cache_key: String,
        decoded: Option<(u32, u32)>,
    },
}

/// The transport-availability bundle shared between the playback projection
/// and the painters (branch D10): the painter and the panels' hit-testing read
/// the same three bits, never duplicate fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransportAvailability {
    pub stop: bool,
    pub next: bool,
    pub previous: bool,
}

/// Configured music albums need the image worker's child-audio lookup; their
/// album containers do not reliably expose usable Primary images.
pub const MUSIC_ALBUM_IMAGE_TYPES: &[&str] = &["AudioChild"];
