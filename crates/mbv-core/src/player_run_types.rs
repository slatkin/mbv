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
    queue: PlaybackQueue,
    audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
    projection: QueueProjection,
    prepared_source: Option<PreparedSource>,
    active_file_starting: bool,
    ext_sub_urls: Vec<String>,
    // loop state
    current_idx: usize,
    forced_slot_id: Option<QueueSlotId>,
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
