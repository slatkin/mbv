use super::{
    mpsc, ActiveItemLifecycle, Arc, AudiobookshelfPlayerContext, Duration, ExecutionSequence,
    Instant, IntroState, ItemId, LoadState, MpvRunConfig, Mutex, NextUp, PlayerEvent, PlayerStatus,
    PreparedSource, QueueSlotId, SessionReporter, StartupPause, StopReport, SubtitlePrefs,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlaybackOrigin {
    Standalone,
    Queue,
}

/// Arguments shared by the `PlaybackRun` constructors: everything except the
/// queue itself (`Vec<ExecSlot>` vs `ExecutionSequence`) and the subtitle
/// URLs only `init_from_queue` takes. One struct so same-type neighbours
/// (`server_url`/`token`, the `Arc<Mutex<..>>` trio) can't be swapped
/// silently at a call site.
pub(crate) struct RunInit {
    pub(crate) start_idx: usize,
    pub(crate) origin: PlaybackOrigin,
    pub(crate) reporter: SessionReporter,
    pub(crate) config: MpvRunConfig,
    pub(crate) startup_pause_for_pipe: bool,
    pub(crate) status: Arc<Mutex<PlayerStatus>>,
    pub(crate) event_tx: mpsc::Sender<PlayerEvent>,
    pub(crate) subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    pub(crate) shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    pub(crate) server_url: String,
    pub(crate) token: String,
    pub(crate) audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
    pub(crate) prepared_source: Option<PreparedSource>,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "runtime flags represent independent playback state, not alternatives (design analysis, issue #804)"
)]
pub(crate) struct PlaybackRun {
    pub(crate) origin: PlaybackOrigin,
    pub(crate) run_identity: mbv_ctrl::PlaybackGeneration,
    pub(crate) config: MpvRunConfig,
    pub(crate) reporter: SessionReporter,
    pub(crate) event_tx: mpsc::Sender<PlayerEvent>,
    pub(crate) status: Arc<Mutex<PlayerStatus>>,
    pub(crate) subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    pub(crate) server_url: String,
    pub(crate) token: String,
    pub(crate) queue: ExecutionSequence,
    pub(crate) audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
    pub(crate) active_lifecycle: ActiveItemLifecycle,
    pub(crate) active_file: bool,
    pub(crate) prepared_source: Option<PreparedSource>,
    pub(crate) active_file_starting: bool,
    pub(crate) ext_sub_urls: Vec<String>,
    // loop state
    pub(crate) current_idx: usize,
    pub(crate) forced_slot_id: Option<QueueSlotId>,
    /// Whether the current playlist jump began with no active playback. Such a
    /// jump has no outgoing `EndFile` to settle it; `PlaybackRestart` must do so.
    pub(crate) forced_jump_from_idle: bool,
    /// Request identity of the in-flight explicit jump that set
    /// `forced_slot_id`, so the settling `TrackChanged` observation can be
    /// tagged with the `(request_id, generation)` it satisfies (design D4).
    /// Cleared in lockstep with `forced_slot_id`.
    pub(crate) forced_transition: Option<crate::transition::Transition>,
    /// Resume position for the target of an in-flight explicit jump on the
    /// playlist (non-active-file) path. mpv only honors a playlist entry's
    /// baked `start=` option the first time that entry loads; navigating back
    /// to it via `playlist-pos` reopens it from `start=` again, discarding
    /// whatever was watched in this session. Taken and applied as an absolute
    /// seek on the target's first `PlaybackRestart` (see `on_playback_restart`).
    pub(crate) forced_resume_ticks: Option<i64>,
    /// Slot identity captured at the moment a stop/quit is first observed, so a
    /// `QueueMove`/`QueueRemove` applied before the deferred `Stopped` emit
    /// (shutdown / quit-timeout paths) cannot change which occurrence the event
    /// names (design D2). `None` once taken or when no stop is pending.
    pub(crate) stop_slot: Option<QueueSlotId>,
    /// Runtime captured with `stop_slot`, so deferred quit/shutdown decisions
    /// use the completed occurrence rather than a later status update.
    pub(crate) stop_runtime: Option<i64>,
    pub(crate) quit_at: Option<Instant>,
    pub(crate) last_seek_at: Option<Instant>,
    pub(crate) last_valid_pos: i64,
    pub(crate) tracks_initialized: bool,
    pub(crate) load_state: LoadState,
    pub(crate) pending_initial_playlist_layout: bool,
    pub(crate) stop_report: StopReport,
    pub(crate) stopped_event_sent: bool,
    pub(crate) mark_played_id: Option<ItemId>,
    pub(crate) osd_title: String,
    pub(crate) last_mouse_osd: Option<Instant>,
    pub(crate) series_id: ItemId,
    pub(crate) season: i64,
    pub(crate) episode: i64,
    pub(crate) next_up: NextUp,
    pub(crate) queue_next_up: NextUp,
    pub(crate) next_up_jump: bool,
    pub(crate) stopped_near_end: bool,
    pub(crate) shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    pub(crate) startup_pause: StartupPause,
    // intro
    pub(crate) intro_start: i64,
    pub(crate) intro_end: i64,
    pub(crate) intro_state: IntroState,
}
