use super::{
    mpsc, ActiveItemLifecycle, Arc, AudiobookshelfPlayerContext, Duration, ExecutionSequence,
    Instant, IntroState, ItemId, LoadState, MpvRunConfig, Mutex, NextUp, PlayerEvent, PlayerStatus,
    PreparedSource, QueueSlotId, SessionReporter, StartupPause, StopReport, SubtitlePrefs,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(in crate::player) enum PlaybackOrigin {
    Standalone,
    Queue,
}

/// Arguments shared by the `PlaybackRun` constructors: everything except the
/// queue itself (`Vec<ExecSlot>` vs `ExecutionSequence`) and the subtitle
/// URLs only `init_from_queue` takes. One struct so same-type neighbours
/// (`server_url`/`token`, the `Arc<Mutex<..>>` trio) can't be swapped
/// silently at a call site.
pub(in crate::player) struct RunInit {
    pub(in crate::player) start_idx: usize,
    pub(in crate::player) origin: PlaybackOrigin,
    pub(in crate::player) reporter: SessionReporter,
    pub(in crate::player) config: MpvRunConfig,
    pub(in crate::player) startup_pause_for_pipe: bool,
    pub(in crate::player) status: Arc<Mutex<PlayerStatus>>,
    pub(in crate::player) event_tx: mpsc::Sender<PlayerEvent>,
    pub(in crate::player) subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    pub(in crate::player) shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    pub(in crate::player) server_url: String,
    pub(in crate::player) token: String,
    pub(in crate::player) audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
    pub(in crate::player) prepared_source: Option<PreparedSource>,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "runtime flags represent independent playback state, not alternatives (design analysis, issue #804)"
)]
pub(in crate::player) struct PlaybackRun {
    pub(in crate::player) origin: PlaybackOrigin,
    pub(in crate::player) run_identity: crate::ctrl::PlaybackGeneration,
    pub(in crate::player) config: MpvRunConfig,
    pub(in crate::player) reporter: SessionReporter,
    pub(in crate::player) event_tx: mpsc::Sender<PlayerEvent>,
    pub(in crate::player) status: Arc<Mutex<PlayerStatus>>,
    pub(in crate::player) subtitle_prefs: Arc<Mutex<SubtitlePrefs>>,
    pub(in crate::player) server_url: String,
    pub(in crate::player) token: String,
    pub(in crate::player) queue: ExecutionSequence,
    pub(in crate::player) audiobookshelf_context: Option<AudiobookshelfPlayerContext>,
    pub(in crate::player) active_lifecycle: ActiveItemLifecycle,
    pub(in crate::player) active_file: bool,
    pub(in crate::player) prepared_source: Option<PreparedSource>,
    pub(in crate::player) active_file_starting: bool,
    pub(in crate::player) ext_sub_urls: Vec<String>,
    // loop state
    pub(in crate::player) current_idx: usize,
    pub(in crate::player) forced_slot_id: Option<QueueSlotId>,
    /// Whether the current playlist jump began with no active playback. Such a
    /// jump has no outgoing `EndFile` to settle it; `PlaybackRestart` must do so.
    pub(in crate::player) forced_jump_from_idle: bool,
    /// Request identity of the in-flight explicit jump that set
    /// `forced_slot_id`, so the settling `TrackChanged` observation can be
    /// tagged with the `(request_id, generation)` it satisfies (design D4).
    /// Cleared in lockstep with `forced_slot_id`.
    pub(in crate::player) forced_transition: Option<crate::playback_transition::Transition>,
    /// Resume position for the target of an in-flight explicit jump on the
    /// playlist (non-active-file) path. mpv only honors a playlist entry's
    /// baked `start=` option the first time that entry loads; navigating back
    /// to it via `playlist-pos` reopens it from `start=` again, discarding
    /// whatever was watched in this session. Taken and applied as an absolute
    /// seek on the target's first `PlaybackRestart` (see `on_playback_restart`).
    pub(in crate::player) forced_resume_ticks: Option<i64>,
    /// Slot identity captured at the moment a stop/quit is first observed, so a
    /// `QueueMove`/`QueueRemove` applied before the deferred `Stopped` emit
    /// (shutdown / quit-timeout paths) cannot change which occurrence the event
    /// names (design D2). `None` once taken or when no stop is pending.
    pub(in crate::player) stop_slot: Option<QueueSlotId>,
    /// Runtime captured with `stop_slot`, so deferred quit/shutdown decisions
    /// use the completed occurrence rather than a later status update.
    pub(in crate::player) stop_runtime: Option<i64>,
    pub(in crate::player) quit_at: Option<Instant>,
    pub(in crate::player) last_seek_at: Option<Instant>,
    pub(in crate::player) last_valid_pos: i64,
    pub(in crate::player) tracks_initialized: bool,
    pub(in crate::player) load_state: LoadState,
    pub(in crate::player) pending_initial_playlist_layout: bool,
    pub(in crate::player) stop_report: StopReport,
    pub(in crate::player) stopped_event_sent: bool,
    pub(in crate::player) mark_played_id: Option<ItemId>,
    pub(in crate::player) osd_title: String,
    pub(in crate::player) last_mouse_osd: Option<Instant>,
    pub(in crate::player) series_id: ItemId,
    pub(in crate::player) season: i64,
    pub(in crate::player) episode: i64,
    pub(in crate::player) next_up: NextUp,
    pub(in crate::player) queue_next_up: NextUp,
    pub(in crate::player) next_up_jump: bool,
    pub(in crate::player) stopped_near_end: bool,
    pub(in crate::player) shutdown_report_timeout: Arc<Mutex<Option<Duration>>>,
    pub(in crate::player) startup_pause: StartupPause,
    // intro
    pub(in crate::player) intro_start: i64,
    pub(in crate::player) intro_end: i64,
    pub(in crate::player) intro_state: IntroState,
}
