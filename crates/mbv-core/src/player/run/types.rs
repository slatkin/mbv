use super::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum PlaybackOrigin {
    Standalone,
    Queue,
}

pub(super) struct PlaybackRun {
    pub(super) origin: PlaybackOrigin,
    pub(super) run_identity: (crate::ctrl::PlaybackRequestId, crate::ctrl::PlaybackGeneration),
    pub(super) config: MpvRunConfig,
    pub(super) reporter: SessionReporter,
    pub(super) event_tx: mpsc::Sender<PlayerEvent>,
    pub(super) status: Arc<Mutex<PlayerStatus>>,
    pub(super) subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    pub(super) server_url: String,
    pub(super) token: String,
    pub(super) queue: ExecutionSequence,
    pub(super) audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
    pub(super) active_lifecycle: ActiveItemLifecycle,
    pub(super) active_file: bool,
    pub(super) prepared_source: Option<PreparedSource>,
    pub(super) active_file_starting: bool,
    pub(super) ext_sub_urls: Vec<String>,
    // loop state
    pub(super) current_idx: usize,
    pub(super) forced_slot_id: Option<QueueSlotId>,
    /// Whether the current playlist jump began with no active playback. Such a
    /// jump has no outgoing EndFile to settle it; PlaybackRestart must do so.
    pub(super) forced_jump_from_idle: bool,
    /// Request identity of the in-flight explicit jump that set
    /// `forced_slot_id`, so the settling `TrackChanged` observation can be
    /// tagged with the `(request_id, generation)` it satisfies (design D4).
    /// Cleared in lockstep with `forced_slot_id`.
    pub(super) forced_transition: Option<crate::playback_transition::Transition>,
    /// Resume position for the target of an in-flight explicit jump on the
    /// playlist (non-active-file) path. mpv only honors a playlist entry's
    /// baked `start=` option the first time that entry loads; navigating back
    /// to it via `playlist-pos` reopens it from `start=` again, discarding
    /// whatever was watched in this session. Taken and applied as an absolute
    /// seek on the target's first `PlaybackRestart` (see `on_playback_restart`).
    pub(super) forced_resume_ticks: Option<i64>,
    /// Slot identity captured at the moment a stop/quit is first observed, so a
    /// `QueueMove`/`QueueRemove` applied before the deferred `Stopped` emit
    /// (shutdown / quit-timeout paths) cannot change which occurrence the event
    /// names (design D2). `None` once taken or when no stop is pending.
    pub(super) stop_slot: Option<QueueSlotId>,
    /// Runtime captured with `stop_slot`, so deferred quit/shutdown decisions
    /// use the completed occurrence rather than a later status update.
    pub(super) stop_runtime: Option<i64>,
    pub(super) quit_at: Option<Instant>,
    pub(super) last_seek_at: Option<Instant>,
    pub(super) last_valid_pos: i64,
    pub(super) tracks_initialized: bool,
    pub(super) load_state: LoadState,
    pub(super) pending_initial_playlist_layout: bool,
    pub(super) stop_report: StopReport,
    pub(super) stopped_event_sent: bool,
    pub(super) mark_played_id: Option<ItemId>,
    pub(super) osd_title: String,
    pub(super) last_mouse_osd: Option<Instant>,
    pub(super) series_id: ItemId,
    pub(super) season: i64,
    pub(super) episode: i64,
    pub(super) next_up: NextUp,
    pub(super) queue_next_up: NextUp,
    pub(super) next_up_jump: bool,
    pub(super) stopped_near_end: bool,
    pub(super) shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    pub(super) startup_pause: StartupPause,
    // intro
    pub(super) intro_start: i64,
    pub(super) intro_end: i64,
    pub(super) intro_state: IntroState,
}

