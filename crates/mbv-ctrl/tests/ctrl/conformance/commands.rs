//! Command (`commands/`) corpus: one golden `CtrlCmd` per variant —
//! `PlayerCmd` per `WireCommand` tag, `PlaybackIntent` per
//! `PlaybackIntentAction`, `SelectPlaybackOutput` and `SwitchPlaybackOutput`
//! per `OutputSelection`, `RunOwnerAction` per `OwnerAction`, and
//! `ApplyServiceSetup` per `ServiceKind` (design D4).

use mbv_queue::{QueueLineage, ServiceKind};
use strum::IntoEnumIterator;

use super::builders::{
    CAST_HOST, CAST_NAME, CAST_PORT, CAST_RECEIVER_ID, EMBY_DEVICE_ID, EMBY_DEVICE_NAME,
    EMBY_ITEM_ID, GENERATION, NAMED_QUEUE_NAME, REQUEST_ID, REVISION, START_POSITION_SECS,
    emby_queue_item, feed_queue_item, named_queue_id, progress_update, queue_source,
    resting_queue_id, unified_queue_replace_slots,
};
use super::*;

/// One command shape in the corpus. Coverage is compile-forced: every shape
/// enum derives `EnumIter`, `all()` iterates the top-level shapes and
/// expands each payload-carrying variant over its nested enum, and the
/// expansion match names every variant — a new shape fails this build until
/// its arm, builder, fixture row, and fixture exist. `ServiceKind` and
/// `OwnerAction` are production enums the iterator cannot construct, so
/// their shapes are flattened to one variant per case: a new production
/// variant still fails the exhaustive `shape_of` match. The unified queue and
/// Named queue commands are grouped into their own shape enums so no builder
/// outgrows the `too_many_lines` threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::EnumIter)]
enum CommandShape {
    Hello,
    PlayerCmd(WireTag),
    Stop,
    PlaybackIntent(IntentAction),
    RequestShutdown,
    ApplyServiceSetupEmby,
    ApplyServiceSetupAudiobookshelf,
    SwapPrepared,
    RunOwnerActionSwapPanel,
    RunOwnerActionOpen,
    UnifiedQueue(UnifiedShape),
    NamedQueue(NamedQueueShape),
    UnifiedAdoptQueue,
    SelectPlaybackOutput(OutputChoice),
    SwitchPlaybackOutput(OutputChoice),
}

impl CommandShape {
    /// Every command shape: iterate the top-level variants, expanding each
    /// payload-carrying variant over its nested enum's iteration. The flat
    /// or-pattern is exhaustive, so a new shape fails this build until it
    /// lands in an arm — it can never be missing from the table silently.
    fn all() -> Vec<Self> {
        let mut shapes = Vec::new();
        for shape in Self::iter() {
            match shape {
                CommandShape::PlayerCmd(_) => {
                    shapes.extend(WireTag::iter().map(CommandShape::PlayerCmd));
                }
                CommandShape::PlaybackIntent(_) => {
                    shapes.extend(IntentAction::iter().map(CommandShape::PlaybackIntent));
                }
                CommandShape::UnifiedQueue(_) => {
                    shapes.extend(UnifiedShape::iter().map(CommandShape::UnifiedQueue));
                }
                CommandShape::NamedQueue(_) => {
                    shapes.extend(NamedQueueShape::iter().map(CommandShape::NamedQueue));
                }
                CommandShape::SelectPlaybackOutput(_) => {
                    shapes.extend(OutputChoice::iter().map(CommandShape::SelectPlaybackOutput));
                }
                CommandShape::SwitchPlaybackOutput(_) => {
                    shapes.extend(OutputChoice::iter().map(CommandShape::SwitchPlaybackOutput));
                }
                CommandShape::Hello
                | CommandShape::Stop
                | CommandShape::RequestShutdown
                | CommandShape::ApplyServiceSetupEmby
                | CommandShape::ApplyServiceSetupAudiobookshelf
                | CommandShape::SwapPrepared
                | CommandShape::RunOwnerActionSwapPanel
                | CommandShape::RunOwnerActionOpen
                | CommandShape::UnifiedAdoptQueue => shapes.push(shape),
            }
        }
        shapes
    }

    /// The fixture stem for this shape. Exhaustive: a new shape fails this
    /// build until its arm and fixture row exist.
    fn name(self) -> String {
        match self {
            CommandShape::Hello => "Hello".into(),
            CommandShape::PlayerCmd(tag) => format!("PlayerCmd.{}", tag.name()),
            CommandShape::Stop => "Stop".into(),
            CommandShape::PlaybackIntent(action) => {
                format!("PlaybackIntent.{}", action.name())
            }
            CommandShape::RequestShutdown => "RequestShutdown".into(),
            CommandShape::ApplyServiceSetupEmby => "ApplyServiceSetup.Emby".into(),
            CommandShape::ApplyServiceSetupAudiobookshelf => {
                "ApplyServiceSetup.Audiobookshelf".into()
            }
            CommandShape::SwapPrepared => "SwapPrepared".into(),
            CommandShape::RunOwnerActionSwapPanel => "RunOwnerAction.SwapPanel".into(),
            CommandShape::RunOwnerActionOpen => "RunOwnerAction.Open".into(),
            CommandShape::UnifiedQueue(shape) => shape.name().into(),
            CommandShape::NamedQueue(shape) => shape.name().into(),
            CommandShape::UnifiedAdoptQueue => "UnifiedAdoptQueue".into(),
            CommandShape::SelectPlaybackOutput(choice) => {
                format!("SelectPlaybackOutput.{}", choice.name())
            }
            CommandShape::SwitchPlaybackOutput(choice) => {
                format!("SwitchPlaybackOutput.{}", choice.name())
            }
        }
    }

    /// The canonical `CtrlCmd` for one shape.
    fn build(self) -> CtrlCmd {
        match self {
            CommandShape::Hello => CtrlCmd::Hello(hello::base()),
            CommandShape::PlayerCmd(tag) => CtrlCmd::PlayerCmd(tag.build()),
            CommandShape::Stop => CtrlCmd::Stop,
            CommandShape::PlaybackIntent(action) => CtrlCmd::PlaybackIntent(PlaybackIntent {
                request_id: REQUEST_ID,
                generation: GENERATION,
                action: action.build(),
            }),
            CommandShape::RequestShutdown => CtrlCmd::RequestShutdown,
            CommandShape::ApplyServiceSetupEmby => CtrlCmd::ApplyServiceSetup {
                kind: ServiceKind::Emby,
                revision: REVISION,
            },
            CommandShape::ApplyServiceSetupAudiobookshelf => CtrlCmd::ApplyServiceSetup {
                kind: ServiceKind::Audiobookshelf,
                revision: REVISION,
            },
            CommandShape::SwapPrepared => CtrlCmd::SwapPrepared,
            CommandShape::RunOwnerActionSwapPanel => {
                CtrlCmd::RunOwnerAction(OwnerAction::SwapPanel)
            }
            CommandShape::RunOwnerActionOpen => CtrlCmd::RunOwnerAction(OwnerAction::Open),
            CommandShape::UnifiedQueue(shape) => shape.build(),
            CommandShape::NamedQueue(shape) => shape.build(),
            CommandShape::UnifiedAdoptQueue => CtrlCmd::UnifiedAdoptQueue {
                items: vec![emby_queue_item()],
                cursor: 1,
                source: queue_source(),
            },
            CommandShape::SelectPlaybackOutput(choice) => {
                CtrlCmd::SelectPlaybackOutput(choice.build())
            }
            CommandShape::SwitchPlaybackOutput(choice) => {
                CtrlCmd::SwitchPlaybackOutput(choice.build())
            }
        }
    }
}

/// The `CtrlCmd::UnifiedQueue*` shapes: one fixture row per variant, plus the
/// defaulted `queue` field's populated case for the edit commands (design D3
/// of `gui-named-queues`). `Default` exists only so `CommandShape::iter()`
/// can construct the payload variant; the expansion discards it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, strum::EnumIter)]
enum UnifiedShape {
    #[default]
    Replace,
    ReplaceWithStartPosition,
    SourceUpdate,
    Append,
    /// The one representative row for the defaulted `queue` field every edit
    /// command carries. The field and its type are shared across the nine
    /// edit commands, so a copy per command would pin the same encode nine
    /// times; each base row already pins the field's `null` case.
    AppendToNamedQueue,
    RemoveSlot,
    RemoveSlots,
    MoveSlot,
    Shuffle,
    PlaySlot,
    Clear,
    ClearOp,
    Refresh,
    ApplyProgress,
}

impl UnifiedShape {
    /// The fixture stem. Exhaustive: a new shape fails this build until its
    /// arm and fixture row exist.
    fn name(self) -> &'static str {
        match self {
            UnifiedShape::Replace => "UnifiedQueueReplace",
            UnifiedShape::ReplaceWithStartPosition => "UnifiedQueueReplace.WithStartPosition",
            UnifiedShape::SourceUpdate => "UnifiedQueueSourceUpdate",
            UnifiedShape::Append => "UnifiedQueueAppend",
            UnifiedShape::AppendToNamedQueue => "UnifiedQueueAppend.NamedQueue",
            UnifiedShape::RemoveSlot => "UnifiedQueueRemoveSlot",
            UnifiedShape::RemoveSlots => "UnifiedQueueRemoveSlots",
            UnifiedShape::MoveSlot => "UnifiedQueueMoveSlot",
            UnifiedShape::Shuffle => "UnifiedQueueShuffle",
            UnifiedShape::PlaySlot => "UnifiedQueuePlaySlot",
            UnifiedShape::Clear => "UnifiedQueueClear",
            UnifiedShape::ClearOp => "UnifiedQueueClearOp",
            UnifiedShape::Refresh => "UnifiedQueueRefresh",
            UnifiedShape::ApplyProgress => "UnifiedQueueApplyProgress",
        }
    }

    /// The canonical `CtrlCmd` for one shape.
    fn build(self) -> CtrlCmd {
        match self {
            UnifiedShape::Replace => unified_queue_replace(None),
            UnifiedShape::ReplaceWithStartPosition => {
                unified_queue_replace(Some(START_POSITION_SECS))
            }
            UnifiedShape::SourceUpdate => CtrlCmd::UnifiedQueueSourceUpdate {
                source: queue_source(),
                lineage: QueueLineage(2),
                op: Some(QueueOpId(42)),
                queue: None,
            },
            UnifiedShape::Append => unified_queue_append(None),
            UnifiedShape::AppendToNamedQueue => unified_queue_append(Some(resting_queue_id())),
            UnifiedShape::RemoveSlot => CtrlCmd::UnifiedQueueRemoveSlot {
                slot_id: 3,
                op: Some(QueueOpId(44)),
                queue: None,
            },
            UnifiedShape::RemoveSlots => CtrlCmd::UnifiedQueueRemoveSlots {
                slot_ids: vec![3, 4],
                op: Some(QueueOpId(45)),
                queue: None,
            },
            UnifiedShape::MoveSlot => CtrlCmd::UnifiedQueueMoveSlot {
                slot_id: 4,
                to_index: 1,
                op: Some(QueueOpId(46)),
                queue: None,
            },
            UnifiedShape::Shuffle => CtrlCmd::UnifiedQueueShuffle {
                op: Some(QueueOpId(47)),
                queue: None,
            },
            UnifiedShape::PlaySlot => CtrlCmd::UnifiedQueuePlaySlot {
                slot_id: 2,
                op: Some(QueueOpId(48)),
                queue: None,
            },
            UnifiedShape::Clear => CtrlCmd::UnifiedQueueClear,
            UnifiedShape::ClearOp => CtrlCmd::UnifiedQueueClearOp {
                op: QueueOpId(49),
                queue: None,
            },
            UnifiedShape::Refresh => CtrlCmd::UnifiedQueueRefresh { op: QueueOpId(50) },
            UnifiedShape::ApplyProgress => CtrlCmd::UnifiedQueueApplyProgress {
                op: QueueOpId(51),
                updates: vec![progress_update()],
            },
        }
    }
}

/// The `start_position_secs` case of the replace command: `None` for the
/// plain shape, the fixed position for the `WithStartPosition` row.
fn unified_queue_replace(start_position_secs: Option<f64>) -> CtrlCmd {
    let slots = unified_queue_replace_slots();
    // The legacy `items` payload mirrors the slots, as
    // `CtrlCmd::unified_queue_replace` derives it.
    let items = slots.iter().map(|slot| slot.item.clone()).collect();
    CtrlCmd::UnifiedQueueReplace {
        items,
        slots,
        start_idx: Some(0),
        source: queue_source(),
        start_position_secs,
        op: Some(QueueOpId(41)),
        queue: None,
    }
}

/// The append command, with or without the Named queue it names.
fn unified_queue_append(queue: Option<mbv_queue::NamedQueueId>) -> CtrlCmd {
    CtrlCmd::UnifiedQueueAppend {
        items: vec![feed_queue_item()],
        before: Some(2),
        op: Some(QueueOpId(43)),
        queue,
    }
}

/// The `CtrlCmd::NamedQueue*` shapes (design D3 of `gui-named-queues`), with
/// the no-slot and slot cases of `NamedQueuePlay`. `Default` exists only so
/// `CommandShape::iter()` can construct the payload variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, strum::EnumIter)]
enum NamedQueueShape {
    #[default]
    Create,
    Rename,
    Close,
    Play,
    PlaySlot,
    Show,
}

impl NamedQueueShape {
    /// The fixture stem. Exhaustive: a new shape fails this build until its
    /// arm and fixture row exist.
    fn name(self) -> &'static str {
        match self {
            NamedQueueShape::Create => "NamedQueueCreate",
            NamedQueueShape::Rename => "NamedQueueRename",
            NamedQueueShape::Close => "NamedQueueClose",
            NamedQueueShape::Play => "NamedQueuePlay",
            NamedQueueShape::PlaySlot => "NamedQueuePlay.Slot",
            NamedQueueShape::Show => "NamedQueueShow",
        }
    }

    /// The canonical `CtrlCmd` for one shape.
    fn build(self) -> CtrlCmd {
        match self {
            NamedQueueShape::Create => CtrlCmd::NamedQueueCreate { op: QueueOpId(52) },
            NamedQueueShape::Rename => CtrlCmd::NamedQueueRename {
                queue: named_queue_id(),
                name: NAMED_QUEUE_NAME.to_owned(),
                op: QueueOpId(53),
            },
            NamedQueueShape::Close => CtrlCmd::NamedQueueClose {
                queue: named_queue_id(),
                op: QueueOpId(54),
            },
            NamedQueueShape::Play => CtrlCmd::NamedQueuePlay {
                queue: named_queue_id(),
                slot: None,
                op: QueueOpId(55),
            },
            NamedQueueShape::PlaySlot => CtrlCmd::NamedQueuePlay {
                queue: named_queue_id(),
                slot: Some(6),
                op: QueueOpId(56),
            },
            NamedQueueShape::Show => CtrlCmd::NamedQueueShow {
                queue: named_queue_id(),
            },
        }
    }
}

/// Exhaustive shape-name match over `CtrlCmd` with NO wildcard arm: a new
/// variant fails this build until its arm, its fixture row, and its fixture
/// exist (unit-B deviation, applied to `CtrlCmd`).
fn shape_of(cmd: &CtrlCmd) -> CommandShape {
    match cmd {
        CtrlCmd::Hello(_) => CommandShape::Hello,
        CtrlCmd::PlayerCmd(cmd) => CommandShape::PlayerCmd(wire_tag_of(cmd)),
        CtrlCmd::Stop => CommandShape::Stop,
        CtrlCmd::PlaybackIntent(intent) => {
            CommandShape::PlaybackIntent(intent_action_of(&intent.action))
        }
        CtrlCmd::RequestShutdown => CommandShape::RequestShutdown,
        CtrlCmd::ApplyServiceSetup { kind, .. } => match kind {
            ServiceKind::Emby => CommandShape::ApplyServiceSetupEmby,
            ServiceKind::Audiobookshelf => CommandShape::ApplyServiceSetupAudiobookshelf,
        },
        CtrlCmd::SwapPrepared => CommandShape::SwapPrepared,
        CtrlCmd::RunOwnerAction(action) => match action {
            OwnerAction::SwapPanel => CommandShape::RunOwnerActionSwapPanel,
            OwnerAction::Open => CommandShape::RunOwnerActionOpen,
        },
        CtrlCmd::UnifiedQueueReplace {
            start_position_secs: Some(_),
            ..
        } => CommandShape::UnifiedQueue(UnifiedShape::ReplaceWithStartPosition),
        CtrlCmd::UnifiedQueueReplace { .. } => CommandShape::UnifiedQueue(UnifiedShape::Replace),
        CtrlCmd::UnifiedQueueSourceUpdate { .. } => {
            CommandShape::UnifiedQueue(UnifiedShape::SourceUpdate)
        }
        CtrlCmd::UnifiedQueueAppend { queue: Some(_), .. } => {
            CommandShape::UnifiedQueue(UnifiedShape::AppendToNamedQueue)
        }
        CtrlCmd::UnifiedQueueAppend { .. } => CommandShape::UnifiedQueue(UnifiedShape::Append),
        CtrlCmd::UnifiedQueueRemoveSlot { .. } => {
            CommandShape::UnifiedQueue(UnifiedShape::RemoveSlot)
        }
        CtrlCmd::UnifiedQueueRemoveSlots { .. } => {
            CommandShape::UnifiedQueue(UnifiedShape::RemoveSlots)
        }
        CtrlCmd::UnifiedQueueMoveSlot { .. } => CommandShape::UnifiedQueue(UnifiedShape::MoveSlot),
        CtrlCmd::UnifiedQueueShuffle { .. } => CommandShape::UnifiedQueue(UnifiedShape::Shuffle),
        CtrlCmd::UnifiedQueuePlaySlot { .. } => CommandShape::UnifiedQueue(UnifiedShape::PlaySlot),
        CtrlCmd::UnifiedQueueClear => CommandShape::UnifiedQueue(UnifiedShape::Clear),
        CtrlCmd::UnifiedQueueClearOp { .. } => CommandShape::UnifiedQueue(UnifiedShape::ClearOp),
        CtrlCmd::UnifiedQueueRefresh { .. } => CommandShape::UnifiedQueue(UnifiedShape::Refresh),
        CtrlCmd::UnifiedQueueApplyProgress { .. } => {
            CommandShape::UnifiedQueue(UnifiedShape::ApplyProgress)
        }
        CtrlCmd::UnifiedAdoptQueue { .. } => CommandShape::UnifiedAdoptQueue,
        CtrlCmd::SelectPlaybackOutput(selection) => {
            CommandShape::SelectPlaybackOutput(output_of(selection))
        }
        CtrlCmd::SwitchPlaybackOutput(selection) => {
            CommandShape::SwitchPlaybackOutput(output_of(selection))
        }
        CtrlCmd::NamedQueueCreate { .. } => CommandShape::NamedQueue(NamedQueueShape::Create),
        CtrlCmd::NamedQueueRename { .. } => CommandShape::NamedQueue(NamedQueueShape::Rename),
        CtrlCmd::NamedQueueClose { .. } => CommandShape::NamedQueue(NamedQueueShape::Close),
        CtrlCmd::NamedQueuePlay { slot: Some(_), .. } => {
            CommandShape::NamedQueue(NamedQueueShape::PlaySlot)
        }
        CtrlCmd::NamedQueuePlay { slot: None, .. } => {
            CommandShape::NamedQueue(NamedQueueShape::Play)
        }
        CtrlCmd::NamedQueueShow { .. } => CommandShape::NamedQueue(NamedQueueShape::Show),
    }
}

// ── Nested wire enums ────────────────────────────────────────────────────

/// One `WireCommand` tag. Every tag gets a `PlayerCmd.<tag>` fixture row.
/// `Default` exists only so `CommandShape::iter()` can construct the
/// payload variant; the expansion discards it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, strum::EnumIter)]
enum WireTag {
    #[default]
    TogglePause,
    JumpTo,
    SetVolume,
    Seek,
    SeekAbsolute,
    SetAudio,
    SetSub,
    SetSubtitlePrefs,
    SetMute,
    NextUpShow,
    NextUpDismiss,
    SkipIntroDismiss,
}

impl WireTag {
    /// Exhaustive: a new `WireCommand` variant fails this build until its
    /// arm and fixture row exist.
    fn name(self) -> &'static str {
        match self {
            WireTag::TogglePause => "TogglePause",
            WireTag::JumpTo => "JumpTo",
            WireTag::SetVolume => "SetVolume",
            WireTag::Seek => "Seek",
            WireTag::SeekAbsolute => "SeekAbsolute",
            WireTag::SetAudio => "SetAudio",
            WireTag::SetSub => "SetSub",
            WireTag::SetSubtitlePrefs => "SetSubtitlePrefs",
            WireTag::SetMute => "SetMute",
            WireTag::NextUpShow => "NextUpShow",
            WireTag::NextUpDismiss => "NextUpDismiss",
            WireTag::SkipIntroDismiss => "SkipIntroDismiss",
        }
    }

    /// The canonical `WireCommand` for one tag, with fixed literals.
    fn build(self) -> WireCommand {
        match self {
            WireTag::TogglePause => WireCommand::TogglePause,
            WireTag::JumpTo => WireCommand::JumpTo(2),
            WireTag::SetVolume => WireCommand::SetVolume(55),
            WireTag::Seek => WireCommand::Seek(12.5),
            WireTag::SeekAbsolute => WireCommand::SeekAbsolute(90.25),
            WireTag::SetAudio => WireCommand::SetAudio(1),
            WireTag::SetSub => WireCommand::SetSub(0),
            WireTag::SetSubtitlePrefs => WireCommand::SetSubtitlePrefs {
                mode: "on".to_owned(),
                subtitle_lang: "en".to_owned(),
                audio_lang: "en".to_owned(),
            },
            WireTag::SetMute => WireCommand::SetMute(true),
            WireTag::NextUpShow => WireCommand::NextUpShow {
                item_id: EMBY_ITEM_ID.to_owned(),
                show_title: "Conformance Show".to_owned(),
                ep_title: "Conformance Episode".to_owned(),
                artist: "Conformance Artist".to_owned(),
            },
            WireTag::NextUpDismiss => WireCommand::NextUpDismiss,
            WireTag::SkipIntroDismiss => WireCommand::SkipIntroDismiss,
        }
    }
}

/// Exhaustive tag match over `WireCommand` with NO wildcard arm: a new
/// variant fails this build until its arm and fixture row exist.
fn wire_tag_of(cmd: &WireCommand) -> WireTag {
    match cmd {
        WireCommand::TogglePause => WireTag::TogglePause,
        WireCommand::JumpTo(_) => WireTag::JumpTo,
        WireCommand::SetVolume(_) => WireTag::SetVolume,
        WireCommand::Seek(_) => WireTag::Seek,
        WireCommand::SeekAbsolute(_) => WireTag::SeekAbsolute,
        WireCommand::SetAudio(_) => WireTag::SetAudio,
        WireCommand::SetSub(_) => WireTag::SetSub,
        WireCommand::SetSubtitlePrefs { .. } => WireTag::SetSubtitlePrefs,
        WireCommand::SetMute(_) => WireTag::SetMute,
        WireCommand::NextUpShow { .. } => WireTag::NextUpShow,
        WireCommand::NextUpDismiss => WireTag::NextUpDismiss,
        WireCommand::SkipIntroDismiss => WireTag::SkipIntroDismiss,
    }
}

/// One `PlaybackIntentAction`. Every action gets a `PlaybackIntent.<action>`
/// fixture row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, strum::EnumIter)]
enum IntentAction {
    #[default]
    Play,
    Stop,
    SetPaused,
    Next,
    Previous,
}

impl IntentAction {
    /// Exhaustive: a new `PlaybackIntentAction` variant fails this build
    /// until its arm and fixture row exist.
    fn name(self) -> &'static str {
        match self {
            IntentAction::Play => "Play",
            IntentAction::Stop => "Stop",
            IntentAction::SetPaused => "SetPaused",
            IntentAction::Next => "Next",
            IntentAction::Previous => "Previous",
        }
    }

    /// The canonical `PlaybackIntentAction` for one action.
    fn build(self) -> PlaybackIntentAction {
        match self {
            IntentAction::Play => PlaybackIntentAction::Play {
                item_ids: vec![EMBY_ITEM_ID.to_owned()],
                start_idx: 0,
                start_ticks: 0,
                source: queue_source(),
            },
            IntentAction::Stop => PlaybackIntentAction::Stop,
            IntentAction::SetPaused => PlaybackIntentAction::SetPaused { paused: true },
            IntentAction::Next => PlaybackIntentAction::Next,
            IntentAction::Previous => PlaybackIntentAction::Previous,
        }
    }
}

/// Exhaustive action match over `PlaybackIntentAction` with NO wildcard arm.
fn intent_action_of(action: &PlaybackIntentAction) -> IntentAction {
    match action {
        PlaybackIntentAction::Play { .. } => IntentAction::Play,
        PlaybackIntentAction::Stop => IntentAction::Stop,
        PlaybackIntentAction::SetPaused { .. } => IntentAction::SetPaused,
        PlaybackIntentAction::Next => IntentAction::Next,
        PlaybackIntentAction::Previous => IntentAction::Previous,
    }
}

/// One `OutputSelection`. Every selection gets a
/// `SelectPlaybackOutput.<selection>` fixture row. The `events/` corpus
/// reuses this enum for `OutputState` naming — the two wire enums have the
/// same three variants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, strum::EnumIter)]
pub(crate) enum OutputChoice {
    #[default]
    Local,
    Cast,
    EmbySession,
}

impl OutputChoice {
    /// Exhaustive: a new `OutputSelection` variant fails this build until
    /// its arm and fixture row exist.
    pub(crate) fn name(self) -> &'static str {
        match self {
            OutputChoice::Local => "Local",
            OutputChoice::Cast => "Cast",
            OutputChoice::EmbySession => "EmbySession",
        }
    }

    /// The canonical `OutputSelection` for one choice.
    fn build(self) -> OutputSelection {
        match self {
            OutputChoice::Local => OutputSelection::Local,
            OutputChoice::Cast => OutputSelection::Cast {
                receiver_id: CAST_RECEIVER_ID.to_owned(),
                name: CAST_NAME.to_owned(),
                host: CAST_HOST.to_owned(),
                port: CAST_PORT,
            },
            OutputChoice::EmbySession => OutputSelection::EmbySession {
                device_id: EMBY_DEVICE_ID.to_owned(),
                name: EMBY_DEVICE_NAME.to_owned(),
            },
        }
    }
}

/// Exhaustive selection match over `OutputSelection` with NO wildcard arm.
pub(crate) fn output_of(selection: &OutputSelection) -> OutputChoice {
    match selection {
        OutputSelection::Local => OutputChoice::Local,
        OutputSelection::Cast { .. } => OutputChoice::Cast,
        OutputSelection::EmbySession { .. } => OutputChoice::EmbySession,
    }
}

/// The `commands/` rows of the fixture table. Every row asserts that the
/// no-wildcard `CtrlCmd` shape-name match maps its built value back to the
/// shape the row names, so builder and match cannot drift apart.
pub(crate) fn rows(rows: &mut Vec<FixtureRow>) {
    for shape in CommandShape::all() {
        let value = shape.build();
        assert_eq!(
            shape_of(&value),
            shape,
            "{}: the CtrlCmd shape-name match disagrees with the fixture row",
            shape.name()
        );
        rows.push(FixtureRow {
            path: format!("commands/{}.json", shape.name()),
            direction: Direction::RoundTrip,
            schema: Some(SchemaAnchor::Root(schema::Root::Cmd)),
            value: fixture_value(value),
        });
    }
}
