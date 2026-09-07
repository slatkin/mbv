#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum PlaybackOrigin {
    Standalone,
    Queue,
}

struct PlaybackRun {
    origin: PlaybackOrigin,
    config: MpvRunConfig,
    reporter: SessionReporter,
    event_tx: mpsc::Sender<PlayerEvent>,
    status: Arc<Mutex<PlayerStatus>>,
    subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    server_url: String,
    token: String,
    queue: ExecutionSequence,
    audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
    active_lifecycle: ActiveItemLifecycle,
    active_file: bool,
    prepared_source: Option<PreparedSource>,
    active_file_starting: bool,
    ext_sub_urls: Vec<String>,
    // loop state
    current_idx: usize,
    forced_slot_id: Option<QueueSlotId>,
    /// Request identity of the in-flight explicit jump that set
    /// `forced_slot_id`, so the settling `TrackChanged` observation can be
    /// tagged with the `(request_id, generation)` it satisfies (design D4).
    /// Cleared in lockstep with `forced_slot_id`.
    forced_transition: Option<crate::playback_transition::Transition>,
    /// Slot identity captured at the moment a stop/quit is first observed, so a
    /// `QueueMove`/`QueueRemove` applied before the deferred `Stopped` emit
    /// (shutdown / quit-timeout paths) cannot change which occurrence the event
    /// names (design D2). `None` once taken or when no stop is pending.
    stop_slot: Option<QueueSlotId>,
    quit_at: Option<Instant>,
    last_seek_at: Option<Instant>,
    last_valid_pos: i64,
    tracks_initialized: bool,
    load_state: LoadState,
    pending_initial_playlist_layout: bool,
    stop_report: StopReport,
    stopped_event_sent: bool,
    mark_played_id: Option<ItemId>,
    osd_title: String,
    last_mouse_osd: Option<Instant>,
    series_id: ItemId,
    season: i64,
    episode: i64,
    next_up: NextUp,
    queue_next_up: NextUp,
    next_up_jump: bool,
    stopped_near_end: bool,
    shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    startup_pause: StartupPause,
    // intro
    intro_start: i64,
    intro_end: i64,
    intro_state: IntroState,
}

impl Drop for PlaybackRun {
    fn drop(&mut self) {
        // This is the last-resort path for delayed quit, FIFO shutdown, and
        // panic unwinding. Keep the owner-known position for the lifecycle's
        // own Drop instead of falling back to zero.
        self.close_prepared_source();
    }
}
