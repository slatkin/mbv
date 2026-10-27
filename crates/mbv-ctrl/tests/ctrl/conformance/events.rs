//! Event (`events/`) corpus: one golden `CtrlEvent` per variant — every
//! `PlayerEvent` variant as a `Player.<variant>` row (including the variants
//! the Client synthesizes itself, plan review note N4), one row per
//! `DisconnectReason`, per `ServiceSetupRejection`, per
//! `PlaybackIntentOutcome` and `PlaybackIntentRejection` case, per
//! `QueueOpOutcome`, and per `OutputState` (design D4).

mod player;

use mbv_queue::ServiceKind;

pub(crate) use super::builders::{
    ABS_BOOK_ITEM_ID, ABS_EPISODE_ID, ABS_LIBRARY_ITEM_ID, CAST_NAME, CAST_RECEIVER_ID,
    EMBY_DEVICE_ID, EMBY_DEVICE_NAME, EMBY_ITEM_ID, EMBY_ITEM_NAME, EMBY_SERIES_ID, GENERATION,
    POSITION_TICKS, REQUEST_ID, REVISION, player_status, unified_queue_state,
};
use super::*;
use player::PlayerShape;

/// Fixed event literals: the corpus pins wire shapes, not runtime state, so
/// every builder below uses these constants and never reads the environment
/// or the clock (design note N2, matching the hello corpus).
const COMMAND_REJECTED_MESSAGE: &str = "conformance command rejected";
const QUEUE_OP_REJECTED_MESSAGE: &str = "conformance queue op rejected";
const SHUTDOWN_REJECTED_REASON: &str = "conformance shutdown rejected";
const OWNER_ACTION_REFUSED_REASON: &str = "conformance owner action refused";
const QUEUE_DESYNCED_DETAIL: &str = "external playlist edit detected";
const REMOTE_DISCONNECTED_MESSAGE: &str = "conformance remote disconnect";
const EMBY_AUTHORITY_TAKEN_MESSAGE: &str = "taken over by another Emby remote";
/// The `PlaybackIntentOutcome::Coalesced` canonical request the fixture pins.
const CANONICAL_REQUEST_ID: u64 = 5;
/// The pid the `ExclusiveOwner` disconnect fixture pins.
const EXCLUSIVE_OWNER_PID: u32 = 4242;
/// The intro window end the `IntroStarted` fixture pins (4 min in ticks).
const INTRO_END_TICKS: i64 = 240_000_000;
/// The buffering remainder the pipe-status fixture pins.
const ESTIMATED_REMAINING_MS: u64 = 1_500;

/// One event shape in the corpus. The exhaustive shape-name match keeps
/// every shape named, and `rows` gives each one a builder and a fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EventShape {
    Hello,
    Player(PlayerShape),
    StatusOnly,
    Disconnected(DisconnectKind),
    CommandRejected,
    PlaybackIntent(IntentOutcome),
    PipePlaybackStatus,
    ShutdownAccepted,
    ShutdownRejected,
    ServiceSetupApplied,
    ServiceSetupRejected(SetupRejection),
    SwapPrepare,
    SwapQuit,
    OwnerActionAccepted,
    OwnerActionRefused,
    UnifiedQueueState,
    QueueOpResult(QueueOutcome),
    AudiobookshelfProgress,
    AudiobookshelfBookProgress,
    PlaybackOutput(commands::OutputChoice),
}

impl EventShape {
    /// Every event shape: each flat variant plus one per nested-enum
    /// variant. Hand-kept, like `commands::CommandShape::all`.
    fn all() -> Vec<Self> {
        let mut shapes = vec![
            EventShape::Hello,
            EventShape::StatusOnly,
            EventShape::CommandRejected,
            EventShape::PipePlaybackStatus,
            EventShape::ShutdownAccepted,
            EventShape::ShutdownRejected,
            EventShape::ServiceSetupApplied,
            EventShape::SwapPrepare,
            EventShape::SwapQuit,
            EventShape::OwnerActionAccepted,
            EventShape::OwnerActionRefused,
            EventShape::UnifiedQueueState,
            EventShape::AudiobookshelfProgress,
            EventShape::AudiobookshelfBookProgress,
        ];
        shapes.extend(PlayerShape::ALL.map(EventShape::Player));
        shapes.extend(DisconnectKind::ALL.map(EventShape::Disconnected));
        shapes.extend(IntentOutcome::ALL.map(EventShape::PlaybackIntent));
        shapes.extend(SetupRejection::ALL.map(EventShape::ServiceSetupRejected));
        shapes.extend(QueueOutcome::ALL.map(EventShape::QueueOpResult));
        shapes.extend(commands::OutputChoice::ALL.map(EventShape::PlaybackOutput));
        shapes
    }

    /// The fixture stem for this shape. Exhaustive: a new shape fails this
    /// build until its arm and fixture row exist.
    fn name(self) -> String {
        match self {
            EventShape::Hello => "Hello".into(),
            EventShape::Player(shape) => format!("Player.{}", shape.name()),
            EventShape::StatusOnly => "StatusOnly".into(),
            EventShape::Disconnected(kind) => format!("Disconnected.{}", kind.name()),
            EventShape::CommandRejected => "CommandRejected".into(),
            EventShape::PlaybackIntent(outcome) => format!("PlaybackIntent.{}", outcome.name()),
            EventShape::PipePlaybackStatus => "PipePlaybackStatus".into(),
            EventShape::ShutdownAccepted => "ShutdownAccepted".into(),
            EventShape::ShutdownRejected => "ShutdownRejected".into(),
            EventShape::ServiceSetupApplied => "ServiceSetupApplied".into(),
            EventShape::ServiceSetupRejected(reason) => {
                format!("ServiceSetupRejected.{}", reason.name())
            }
            EventShape::SwapPrepare => "SwapPrepare".into(),
            EventShape::SwapQuit => "SwapQuit".into(),
            EventShape::OwnerActionAccepted => "OwnerActionAccepted".into(),
            EventShape::OwnerActionRefused => "OwnerActionRefused".into(),
            EventShape::UnifiedQueueState => "UnifiedQueueState".into(),
            EventShape::QueueOpResult(outcome) => format!("QueueOpResult.{}", outcome.name()),
            EventShape::AudiobookshelfProgress => "AudiobookshelfProgress".into(),
            EventShape::AudiobookshelfBookProgress => "AudiobookshelfBookProgress".into(),
            EventShape::PlaybackOutput(choice) => format!("PlaybackOutput.{}", choice.name()),
        }
    }

    /// The canonical `CtrlEvent` for one shape.
    fn build(self) -> CtrlEvent {
        match self {
            EventShape::Hello => CtrlEvent::Hello(hello::base()),
            EventShape::Player(shape) => CtrlEvent::Player(shape.build()),
            EventShape::StatusOnly => CtrlEvent::StatusOnly(player_status()),
            EventShape::Disconnected(kind) => CtrlEvent::Disconnected {
                reason: kind.build(),
            },
            EventShape::CommandRejected => {
                CtrlEvent::CommandRejected(COMMAND_REJECTED_MESSAGE.to_owned())
            }
            EventShape::PlaybackIntent(outcome) => CtrlEvent::PlaybackIntent(PlaybackIntentEvent {
                request_id: REQUEST_ID,
                generation: GENERATION,
                outcome: outcome.build(),
            }),
            EventShape::PipePlaybackStatus => CtrlEvent::PipePlaybackStatus(pipe_status()),
            EventShape::ShutdownAccepted => CtrlEvent::ShutdownAccepted,
            EventShape::ShutdownRejected => CtrlEvent::ShutdownRejected {
                reason: SHUTDOWN_REJECTED_REASON.to_owned(),
            },
            EventShape::ServiceSetupApplied => CtrlEvent::ServiceSetupApplied {
                kind: ServiceKind::Emby,
                revision: REVISION,
            },
            EventShape::ServiceSetupRejected(reason) => CtrlEvent::ServiceSetupRejected {
                kind: ServiceKind::Audiobookshelf,
                revision: REVISION,
                reason: reason.build(),
            },
            EventShape::SwapPrepare => CtrlEvent::SwapPrepare,
            EventShape::SwapQuit => CtrlEvent::SwapQuit,
            EventShape::OwnerActionAccepted => CtrlEvent::OwnerActionAccepted,
            EventShape::OwnerActionRefused => CtrlEvent::OwnerActionRefused {
                reason: OWNER_ACTION_REFUSED_REASON.to_owned(),
            },
            EventShape::UnifiedQueueState => CtrlEvent::UnifiedQueueState(unified_queue_state()),
            EventShape::QueueOpResult(outcome) => CtrlEvent::QueueOpResult {
                op: QueueOpId(41),
                outcome: outcome.build(),
            },
            EventShape::AudiobookshelfProgress => {
                CtrlEvent::AudiobookshelfProgress(abs_progress_event())
            }
            EventShape::AudiobookshelfBookProgress => {
                CtrlEvent::AudiobookshelfBookProgress(abs_book_progress_event())
            }
            EventShape::PlaybackOutput(choice) => CtrlEvent::PlaybackOutput(output_state(choice)),
        }
    }
}

/// Exhaustive shape-name match over `CtrlEvent` with NO wildcard arm: a new
/// variant fails this build until its arm, its fixture row, and its fixture
/// exist.
fn shape_of(event: &CtrlEvent) -> EventShape {
    match event {
        CtrlEvent::Hello(_) => EventShape::Hello,
        CtrlEvent::Player(event) => EventShape::Player(player::shape_of(event)),
        CtrlEvent::StatusOnly(_) => EventShape::StatusOnly,
        CtrlEvent::Disconnected { reason } => EventShape::Disconnected(disconnect_kind_of(*reason)),
        CtrlEvent::CommandRejected(_) => EventShape::CommandRejected,
        CtrlEvent::PlaybackIntent(event) => {
            EventShape::PlaybackIntent(intent_outcome_of(&event.outcome))
        }
        CtrlEvent::PipePlaybackStatus(_) => EventShape::PipePlaybackStatus,
        CtrlEvent::ShutdownAccepted => EventShape::ShutdownAccepted,
        CtrlEvent::ShutdownRejected { .. } => EventShape::ShutdownRejected,
        CtrlEvent::ServiceSetupApplied { .. } => EventShape::ServiceSetupApplied,
        CtrlEvent::ServiceSetupRejected { reason, .. } => {
            EventShape::ServiceSetupRejected(setup_rejection_of(*reason))
        }
        CtrlEvent::SwapPrepare => EventShape::SwapPrepare,
        CtrlEvent::SwapQuit => EventShape::SwapQuit,
        CtrlEvent::OwnerActionAccepted => EventShape::OwnerActionAccepted,
        CtrlEvent::OwnerActionRefused { .. } => EventShape::OwnerActionRefused,
        CtrlEvent::UnifiedQueueState(_) => EventShape::UnifiedQueueState,
        CtrlEvent::QueueOpResult { outcome, .. } => {
            EventShape::QueueOpResult(queue_outcome_of(outcome))
        }
        CtrlEvent::AudiobookshelfProgress(_) => EventShape::AudiobookshelfProgress,
        CtrlEvent::AudiobookshelfBookProgress(_) => EventShape::AudiobookshelfBookProgress,
        CtrlEvent::PlaybackOutput(state) => EventShape::PlaybackOutput(output_state_of(state)),
    }
}

// ── Nested wire enums ────────────────────────────────────────────────────

/// One `DisconnectReason`. Every reason gets a `Disconnected.<reason>`
/// fixture row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DisconnectKind {
    TakenOverByEmbyRemote,
    DaemonShutdown,
    ExclusiveOwner,
    OwnerShuttingDown,
}

impl DisconnectKind {
    const ALL: [DisconnectKind; 4] = [
        DisconnectKind::TakenOverByEmbyRemote,
        DisconnectKind::DaemonShutdown,
        DisconnectKind::ExclusiveOwner,
        DisconnectKind::OwnerShuttingDown,
    ];

    /// Exhaustive: a new `DisconnectReason` variant fails this build until
    /// its arm and fixture row exist.
    fn name(self) -> &'static str {
        match self {
            DisconnectKind::TakenOverByEmbyRemote => "TakenOverByEmbyRemote",
            DisconnectKind::DaemonShutdown => "DaemonShutdown",
            DisconnectKind::ExclusiveOwner => "ExclusiveOwner",
            DisconnectKind::OwnerShuttingDown => "OwnerShuttingDown",
        }
    }

    /// The canonical `DisconnectReason` for one kind.
    fn build(self) -> DisconnectReason {
        match self {
            DisconnectKind::TakenOverByEmbyRemote => DisconnectReason::TakenOverByEmbyRemote,
            DisconnectKind::DaemonShutdown => DisconnectReason::DaemonShutdown,
            DisconnectKind::ExclusiveOwner => DisconnectReason::ExclusiveOwner {
                pid: EXCLUSIVE_OWNER_PID,
            },
            DisconnectKind::OwnerShuttingDown => DisconnectReason::OwnerShuttingDown,
        }
    }
}

/// Exhaustive kind match over `DisconnectReason` with NO wildcard arm.
fn disconnect_kind_of(reason: DisconnectReason) -> DisconnectKind {
    match reason {
        DisconnectReason::TakenOverByEmbyRemote => DisconnectKind::TakenOverByEmbyRemote,
        DisconnectReason::DaemonShutdown => DisconnectKind::DaemonShutdown,
        DisconnectReason::ExclusiveOwner { .. } => DisconnectKind::ExclusiveOwner,
        DisconnectReason::OwnerShuttingDown => DisconnectKind::OwnerShuttingDown,
    }
}

/// One `PlaybackIntentOutcome`. Every flat outcome gets a
/// `PlaybackIntent.<outcome>` fixture row, and every `PlaybackIntentRejection`
/// gets a `PlaybackIntent.Rejected.<reason>` row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IntentOutcome {
    Accepted,
    Applied,
    Coalesced,
    Superseded,
    Rejected(IntentRejection),
}

impl IntentOutcome {
    const ALL: [IntentOutcome; 9] = [
        IntentOutcome::Accepted,
        IntentOutcome::Applied,
        IntentOutcome::Coalesced,
        IntentOutcome::Superseded,
        IntentOutcome::Rejected(IntentRejection::EmptyTarget),
        IntentOutcome::Rejected(IntentRejection::InvalidTarget),
        IntentOutcome::Rejected(IntentRejection::ResolutionFailed),
        IntentOutcome::Rejected(IntentRejection::AudioOnly),
        IntentOutcome::Rejected(IntentRejection::Unavailable),
    ];

    /// The fixture-stem suffix for this outcome. Exhaustive: a new
    /// `PlaybackIntentOutcome` or `PlaybackIntentRejection` variant fails
    /// this build until its arm and fixture row exist.
    fn name(self) -> String {
        match self {
            IntentOutcome::Accepted => "Accepted".into(),
            IntentOutcome::Applied => "Applied".into(),
            IntentOutcome::Coalesced => "Coalesced".into(),
            IntentOutcome::Superseded => "Superseded".into(),
            IntentOutcome::Rejected(reason) => format!("Rejected.{}", reason.name()),
        }
    }

    /// The canonical `PlaybackIntentOutcome` for one case.
    fn build(self) -> PlaybackIntentOutcome {
        match self {
            IntentOutcome::Accepted => PlaybackIntentOutcome::Accepted,
            IntentOutcome::Applied => PlaybackIntentOutcome::Applied,
            IntentOutcome::Coalesced => PlaybackIntentOutcome::Coalesced {
                canonical_request_id: CANONICAL_REQUEST_ID,
            },
            IntentOutcome::Superseded => PlaybackIntentOutcome::Superseded,
            IntentOutcome::Rejected(reason) => PlaybackIntentOutcome::Rejected {
                reason: reason.build(),
            },
        }
    }
}

/// Exhaustive outcome match over `PlaybackIntentOutcome` with NO wildcard
/// arm.
fn intent_outcome_of(outcome: &PlaybackIntentOutcome) -> IntentOutcome {
    match outcome {
        PlaybackIntentOutcome::Accepted => IntentOutcome::Accepted,
        PlaybackIntentOutcome::Applied => IntentOutcome::Applied,
        PlaybackIntentOutcome::Coalesced { .. } => IntentOutcome::Coalesced,
        PlaybackIntentOutcome::Superseded => IntentOutcome::Superseded,
        PlaybackIntentOutcome::Rejected { reason } => {
            IntentOutcome::Rejected(intent_rejection_of(reason))
        }
    }
}

/// One `PlaybackIntentRejection`. Exhaustive: a new variant fails this build
/// until its arm and fixture row exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IntentRejection {
    EmptyTarget,
    InvalidTarget,
    ResolutionFailed,
    AudioOnly,
    Unavailable,
}

impl IntentRejection {
    fn name(self) -> &'static str {
        match self {
            IntentRejection::EmptyTarget => "EmptyTarget",
            IntentRejection::InvalidTarget => "InvalidTarget",
            IntentRejection::ResolutionFailed => "ResolutionFailed",
            IntentRejection::AudioOnly => "AudioOnly",
            IntentRejection::Unavailable => "Unavailable",
        }
    }

    fn build(self) -> PlaybackIntentRejection {
        match self {
            IntentRejection::EmptyTarget => PlaybackIntentRejection::EmptyTarget,
            IntentRejection::InvalidTarget => PlaybackIntentRejection::InvalidTarget,
            IntentRejection::ResolutionFailed => PlaybackIntentRejection::ResolutionFailed,
            IntentRejection::AudioOnly => PlaybackIntentRejection::AudioOnly,
            IntentRejection::Unavailable => PlaybackIntentRejection::Unavailable,
        }
    }
}

/// Exhaustive rejection match over `PlaybackIntentRejection` with NO
/// wildcard arm.
fn intent_rejection_of(reason: &PlaybackIntentRejection) -> IntentRejection {
    match reason {
        PlaybackIntentRejection::EmptyTarget => IntentRejection::EmptyTarget,
        PlaybackIntentRejection::InvalidTarget => IntentRejection::InvalidTarget,
        PlaybackIntentRejection::ResolutionFailed => IntentRejection::ResolutionFailed,
        PlaybackIntentRejection::AudioOnly => IntentRejection::AudioOnly,
        PlaybackIntentRejection::Unavailable => IntentRejection::Unavailable,
    }
}

/// One `ServiceSetupRejection`. Every rejection gets a
/// `ServiceSetupRejected.<reason>` fixture row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SetupRejection {
    UnsupportedService,
    RevisionMismatch,
    StorageUnavailable,
    TransitionRejected,
}

impl SetupRejection {
    const ALL: [SetupRejection; 4] = [
        SetupRejection::UnsupportedService,
        SetupRejection::RevisionMismatch,
        SetupRejection::StorageUnavailable,
        SetupRejection::TransitionRejected,
    ];

    /// Exhaustive: a new `ServiceSetupRejection` variant fails this build
    /// until its arm and fixture row exist.
    fn name(self) -> &'static str {
        match self {
            SetupRejection::UnsupportedService => "UnsupportedService",
            SetupRejection::RevisionMismatch => "RevisionMismatch",
            SetupRejection::StorageUnavailable => "StorageUnavailable",
            SetupRejection::TransitionRejected => "TransitionRejected",
        }
    }

    fn build(self) -> ServiceSetupRejection {
        match self {
            SetupRejection::UnsupportedService => ServiceSetupRejection::UnsupportedService,
            SetupRejection::RevisionMismatch => ServiceSetupRejection::RevisionMismatch,
            SetupRejection::StorageUnavailable => ServiceSetupRejection::StorageUnavailable,
            SetupRejection::TransitionRejected => ServiceSetupRejection::TransitionRejected,
        }
    }
}

/// Exhaustive rejection match over `ServiceSetupRejection` with NO wildcard
/// arm.
fn setup_rejection_of(reason: ServiceSetupRejection) -> SetupRejection {
    match reason {
        ServiceSetupRejection::UnsupportedService => SetupRejection::UnsupportedService,
        ServiceSetupRejection::RevisionMismatch => SetupRejection::RevisionMismatch,
        ServiceSetupRejection::StorageUnavailable => SetupRejection::StorageUnavailable,
        ServiceSetupRejection::TransitionRejected => SetupRejection::TransitionRejected,
    }
}

/// One `QueueOpOutcome`. Both outcomes get a `QueueOpResult.<outcome>`
/// fixture row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueueOutcome {
    Applied,
    Rejected,
}

impl QueueOutcome {
    const ALL: [QueueOutcome; 2] = [QueueOutcome::Applied, QueueOutcome::Rejected];

    /// Exhaustive: a new `QueueOpOutcome` variant fails this build until its
    /// arm and fixture row exist.
    fn name(self) -> &'static str {
        match self {
            QueueOutcome::Applied => "Applied",
            QueueOutcome::Rejected => "Rejected",
        }
    }

    /// The canonical `QueueOpOutcome` for one case: `Applied` carries the
    /// canonical post-op snapshot, `Rejected` the owner-computed reason.
    fn build(self) -> QueueOpOutcome {
        match self {
            QueueOutcome::Applied => QueueOpOutcome::Applied(Box::new(unified_queue_state())),
            QueueOutcome::Rejected => {
                QueueOpOutcome::Rejected(QUEUE_OP_REJECTED_MESSAGE.to_owned())
            }
        }
    }
}

/// Exhaustive outcome match over `QueueOpOutcome` with NO wildcard arm.
fn queue_outcome_of(outcome: &QueueOpOutcome) -> QueueOutcome {
    match outcome {
        QueueOpOutcome::Applied(_) => QueueOutcome::Applied,
        QueueOpOutcome::Rejected(_) => QueueOutcome::Rejected,
    }
}

/// Exhaustive choice match over `OutputState` with NO wildcard arm: the
/// state enum shares its variant names (and fixture naming) with
/// `OutputSelection`.
fn output_state_of(state: &OutputState) -> commands::OutputChoice {
    match state {
        OutputState::Local => commands::OutputChoice::Local,
        OutputState::Cast { .. } => commands::OutputChoice::Cast,
        OutputState::EmbySession { .. } => commands::OutputChoice::EmbySession,
    }
}

/// The canonical `OutputState` for one choice: the Cast fixture pins the
/// connected state, the Emby Session fixture pins a connecting audio-only
/// session that is playing the device's own item.
fn output_state(choice: commands::OutputChoice) -> OutputState {
    match choice {
        commands::OutputChoice::Local => OutputState::Local,
        commands::OutputChoice::Cast => OutputState::Cast {
            receiver_id: CAST_RECEIVER_ID.to_owned(),
            name: CAST_NAME.to_owned(),
            connection: OutputConnection::Connected,
        },
        commands::OutputChoice::EmbySession => OutputState::EmbySession {
            device_id: EMBY_DEVICE_ID.to_owned(),
            name: EMBY_DEVICE_NAME.to_owned(),
            connection: OutputConnection::Connecting,
            audio_only: true,
            now_playing: Some(session_now_playing()),
        },
    }
}

/// The device-owned item the `EmbySession` output fixture pins: an episode
/// with its series context and a logo tag.
fn session_now_playing() -> SessionNowPlaying {
    SessionNowPlaying {
        item_id: EMBY_ITEM_ID.to_owned(),
        item_type: "Episode".to_owned(),
        name: EMBY_ITEM_NAME.to_owned(),
        series_id: Some(EMBY_SERIES_ID.to_owned()),
        series_name: Some("Conformance Series".to_owned()),
        logo_tag: Some("conformance-logo-tag".to_owned()),
    }
}

// ── Shared event payload builders ────────────────────────────────────────

/// The pipe-startup status the `PipePlaybackStatus` fixtures carry: a
/// buffering phase with the estimated remainder present, so the `Option`
/// shape is pinned.
pub(crate) fn pipe_status() -> PipePlaybackStatus {
    PipePlaybackStatus {
        request_id: REQUEST_ID,
        generation: GENERATION,
        phase: PipePlaybackPhase::OutputBuffering,
        estimated_remaining_ms: Some(ESTIMATED_REMAINING_MS),
    }
}

/// The redacted Audiobookshelf episode progress the progress fixtures carry.
pub(crate) fn abs_progress_event() -> AudiobookshelfProgressEvent {
    AudiobookshelfProgressEvent {
        library_item_id: ABS_LIBRARY_ITEM_ID.to_owned(),
        episode_id: ABS_EPISODE_ID.to_owned(),
        position_ticks: builders::POSITION_TICKS,
        is_finished: false,
        setup_generation: GENERATION,
    }
}

/// The redacted Audiobookshelf book progress the book-progress fixtures
/// carry: keyed by `library_item_id` only.
pub(crate) fn abs_book_progress_event() -> AudiobookshelfBookProgressEvent {
    AudiobookshelfBookProgressEvent {
        library_item_id: ABS_BOOK_ITEM_ID.to_owned(),
        position_ticks: builders::POSITION_TICKS,
        is_finished: false,
        setup_generation: GENERATION,
    }
}

/// The `events/` rows of the fixture table. Every row asserts that the
/// no-wildcard `CtrlEvent` shape-name match maps its built value back to the
/// shape the row names, so builder and match cannot drift apart.
pub(crate) fn rows(rows: &mut Vec<FixtureRow>) {
    for shape in EventShape::all() {
        let value = shape.build();
        assert_eq!(
            shape_of(&value),
            shape,
            "{}: the CtrlEvent shape-name match disagrees with the fixture row",
            shape.name()
        );
        rows.push(FixtureRow {
            path: format!("events/{}.json", shape.name()),
            direction: Direction::RoundTrip,
            schema: Some(schema::Root::Event),
            value: fixture_value(value),
        });
    }
}
