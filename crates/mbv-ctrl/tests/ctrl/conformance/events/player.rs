//! `PlayerEvent` payload corpus: every `PlayerEvent` variant gets a
//! `Player.<variant>` row — including the variants `RemotePlayer`
//! synthesizes itself instead of relaying a ctrl line (plan review note N4:
//! `UnifiedQueueUpdated`, `QueueOpResult`, `RemoteDisconnected`,
//! `EmbyAuthorityTaken`, `DaemonShutdownAnnounced`).

use mbv_ctrl::player::PlayerEvent;
use mbv_ids::ItemId;
use mbv_queue::QueueSlotId;

use super::*;

/// One `PlayerEvent` variant. Every variant gets a `Player.<variant>`
/// fixture row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlayerShape {
    Stopped,
    TrackChanged,
    PausedChanged,
    OutputStarted,
    TrackCompleted,
    NextUpThreshold,
    NextUpPlay,
    QueueNextUp,
    UnifiedQueueUpdated,
    QueueOpResult,
    IntroStarted,
    IntroEnded,
    SkipIntroPlay,
    MpvQuit,
    CommandRejected,
    PlaybackIntent,
    PipePlaybackStatus,
    RemoteDisconnected,
    EmbyAuthorityTaken,
    DaemonShutdownAnnounced,
    QueueDesynced,
    SwapPrepare,
    SwapQuit,
    AudiobookshelfProgress,
    AudiobookshelfBookProgress,
    PlaybackOutput,
}

impl PlayerShape {
    /// Every `PlayerEvent` variant, in declaration order.
    pub(crate) const ALL: [PlayerShape; 26] = [
        PlayerShape::Stopped,
        PlayerShape::TrackChanged,
        PlayerShape::PausedChanged,
        PlayerShape::OutputStarted,
        PlayerShape::TrackCompleted,
        PlayerShape::NextUpThreshold,
        PlayerShape::NextUpPlay,
        PlayerShape::QueueNextUp,
        PlayerShape::UnifiedQueueUpdated,
        PlayerShape::QueueOpResult,
        PlayerShape::IntroStarted,
        PlayerShape::IntroEnded,
        PlayerShape::SkipIntroPlay,
        PlayerShape::MpvQuit,
        PlayerShape::CommandRejected,
        PlayerShape::PlaybackIntent,
        PlayerShape::PipePlaybackStatus,
        PlayerShape::RemoteDisconnected,
        PlayerShape::EmbyAuthorityTaken,
        PlayerShape::DaemonShutdownAnnounced,
        PlayerShape::QueueDesynced,
        PlayerShape::SwapPrepare,
        PlayerShape::SwapQuit,
        PlayerShape::AudiobookshelfProgress,
        PlayerShape::AudiobookshelfBookProgress,
        PlayerShape::PlaybackOutput,
    ];

    /// The fixture-stem name: the Rust variant name. Exhaustive: a new
    /// `PlayerEvent` variant fails this build until its arm and fixture row
    /// exist. (`QueueNextUp`'s wire tag stays `PlaylistNextUp`; fixtures name
    /// Rust variants, design D4.)
    pub(crate) fn name(self) -> &'static str {
        match self {
            PlayerShape::Stopped => "Stopped",
            PlayerShape::TrackChanged => "TrackChanged",
            PlayerShape::PausedChanged => "PausedChanged",
            PlayerShape::OutputStarted => "OutputStarted",
            PlayerShape::TrackCompleted => "TrackCompleted",
            PlayerShape::NextUpThreshold => "NextUpThreshold",
            PlayerShape::NextUpPlay => "NextUpPlay",
            PlayerShape::QueueNextUp => "QueueNextUp",
            PlayerShape::UnifiedQueueUpdated => "UnifiedQueueUpdated",
            PlayerShape::QueueOpResult => "QueueOpResult",
            PlayerShape::IntroStarted => "IntroStarted",
            PlayerShape::IntroEnded => "IntroEnded",
            PlayerShape::SkipIntroPlay => "SkipIntroPlay",
            PlayerShape::MpvQuit => "MpvQuit",
            PlayerShape::CommandRejected => "CommandRejected",
            PlayerShape::PlaybackIntent => "PlaybackIntent",
            PlayerShape::PipePlaybackStatus => "PipePlaybackStatus",
            PlayerShape::RemoteDisconnected => "RemoteDisconnected",
            PlayerShape::EmbyAuthorityTaken => "EmbyAuthorityTaken",
            PlayerShape::DaemonShutdownAnnounced => "DaemonShutdownAnnounced",
            PlayerShape::QueueDesynced => "QueueDesynced",
            PlayerShape::SwapPrepare => "SwapPrepare",
            PlayerShape::SwapQuit => "SwapQuit",
            PlayerShape::AudiobookshelfProgress => "AudiobookshelfProgress",
            PlayerShape::AudiobookshelfBookProgress => "AudiobookshelfBookProgress",
            PlayerShape::PlaybackOutput => "PlaybackOutput",
        }
    }

    /// The canonical `PlayerEvent` for one variant, with fixed literals.
    pub(crate) fn build(self) -> PlayerEvent {
        match self {
            PlayerShape::Stopped => PlayerEvent::Stopped {
                slot_id: Some(QueueSlotId::from_raw(1)),
                run_identity: GENERATION,
                position_ticks: POSITION_TICKS,
                played: false,
                consume: false,
                error: None,
            },
            PlayerShape::TrackChanged => PlayerEvent::TrackChanged {
                slot_id: QueueSlotId::from_raw(2),
                transition: None,
            },
            PlayerShape::PausedChanged => PlayerEvent::PausedChanged(true),
            PlayerShape::OutputStarted => PlayerEvent::OutputStarted,
            PlayerShape::TrackCompleted => PlayerEvent::TrackCompleted {
                slot_id: QueueSlotId::from_raw(1),
                run_identity: GENERATION,
                position_ticks: POSITION_TICKS,
                played: true,
                consume: false,
            },
            PlayerShape::NextUpThreshold => PlayerEvent::NextUpThreshold {
                series_id: ItemId::new(EMBY_SERIES_ID),
                season: 1,
                episode: 2,
            },
            PlayerShape::NextUpPlay => PlayerEvent::NextUpPlay,
            PlayerShape::QueueNextUp => PlayerEvent::QueueNextUp { next_idx: 2 },
            PlayerShape::UnifiedQueueUpdated => {
                PlayerEvent::UnifiedQueueUpdated(Box::new(unified_queue_state()))
            }
            PlayerShape::QueueOpResult => PlayerEvent::QueueOpResult {
                op: QueueOpId(41),
                outcome: QueueOpOutcome::Rejected(QUEUE_OP_REJECTED_MESSAGE.to_owned()),
            },
            PlayerShape::IntroStarted => PlayerEvent::IntroStarted {
                intro_end_ticks: INTRO_END_TICKS,
            },
            PlayerShape::IntroEnded => PlayerEvent::IntroEnded,
            PlayerShape::SkipIntroPlay => PlayerEvent::SkipIntroPlay,
            PlayerShape::MpvQuit => PlayerEvent::MpvQuit,
            PlayerShape::CommandRejected => {
                PlayerEvent::CommandRejected(COMMAND_REJECTED_MESSAGE.to_owned())
            }
            PlayerShape::PlaybackIntent => PlayerEvent::PlaybackIntent(PlaybackIntentEvent {
                request_id: REQUEST_ID,
                generation: GENERATION,
                outcome: PlaybackIntentOutcome::Accepted,
            }),
            PlayerShape::PipePlaybackStatus => PlayerEvent::PipePlaybackStatus(pipe_status()),
            PlayerShape::RemoteDisconnected => {
                PlayerEvent::RemoteDisconnected(REMOTE_DISCONNECTED_MESSAGE.to_owned())
            }
            PlayerShape::EmbyAuthorityTaken => {
                PlayerEvent::EmbyAuthorityTaken(EMBY_AUTHORITY_TAKEN_MESSAGE.to_owned())
            }
            PlayerShape::DaemonShutdownAnnounced => PlayerEvent::DaemonShutdownAnnounced,
            PlayerShape::QueueDesynced => {
                PlayerEvent::QueueDesynced(QUEUE_DESYNCED_DETAIL.to_owned())
            }
            PlayerShape::SwapPrepare => PlayerEvent::SwapPrepare,
            PlayerShape::SwapQuit => PlayerEvent::SwapQuit,
            PlayerShape::AudiobookshelfProgress => {
                PlayerEvent::AudiobookshelfProgress(abs_progress_event())
            }
            PlayerShape::AudiobookshelfBookProgress => {
                PlayerEvent::AudiobookshelfBookProgress(abs_book_progress_event())
            }
            PlayerShape::PlaybackOutput => PlayerEvent::PlaybackOutput(OutputState::Local),
        }
    }
}

/// Exhaustive variant match over `PlayerEvent` with NO wildcard arm: a new
/// variant fails this build until its arm and fixture row exist.
pub(crate) fn shape_of(event: &PlayerEvent) -> PlayerShape {
    match event {
        PlayerEvent::Stopped { .. } => PlayerShape::Stopped,
        PlayerEvent::TrackChanged { .. } => PlayerShape::TrackChanged,
        PlayerEvent::PausedChanged(_) => PlayerShape::PausedChanged,
        PlayerEvent::OutputStarted => PlayerShape::OutputStarted,
        PlayerEvent::TrackCompleted { .. } => PlayerShape::TrackCompleted,
        PlayerEvent::NextUpThreshold { .. } => PlayerShape::NextUpThreshold,
        PlayerEvent::NextUpPlay => PlayerShape::NextUpPlay,
        PlayerEvent::QueueNextUp { .. } => PlayerShape::QueueNextUp,
        PlayerEvent::UnifiedQueueUpdated(_) => PlayerShape::UnifiedQueueUpdated,
        PlayerEvent::QueueOpResult { .. } => PlayerShape::QueueOpResult,
        PlayerEvent::IntroStarted { .. } => PlayerShape::IntroStarted,
        PlayerEvent::IntroEnded => PlayerShape::IntroEnded,
        PlayerEvent::SkipIntroPlay => PlayerShape::SkipIntroPlay,
        PlayerEvent::MpvQuit => PlayerShape::MpvQuit,
        PlayerEvent::CommandRejected(_) => PlayerShape::CommandRejected,
        PlayerEvent::PlaybackIntent(_) => PlayerShape::PlaybackIntent,
        PlayerEvent::PipePlaybackStatus(_) => PlayerShape::PipePlaybackStatus,
        PlayerEvent::RemoteDisconnected(_) => PlayerShape::RemoteDisconnected,
        PlayerEvent::EmbyAuthorityTaken(_) => PlayerShape::EmbyAuthorityTaken,
        PlayerEvent::DaemonShutdownAnnounced => PlayerShape::DaemonShutdownAnnounced,
        PlayerEvent::QueueDesynced(_) => PlayerShape::QueueDesynced,
        PlayerEvent::SwapPrepare => PlayerShape::SwapPrepare,
        PlayerEvent::SwapQuit => PlayerShape::SwapQuit,
        PlayerEvent::AudiobookshelfProgress(_) => PlayerShape::AudiobookshelfProgress,
        PlayerEvent::AudiobookshelfBookProgress(_) => PlayerShape::AudiobookshelfBookProgress,
        PlayerEvent::PlaybackOutput(_) => PlayerShape::PlaybackOutput,
    }
}
