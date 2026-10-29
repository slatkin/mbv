//! Command (`commands/`) corpus: one golden `CtrlCmd` per variant —
//! `PlayerCmd` per `WireCommand` tag, `PlaybackIntent` per
//! `PlaybackIntentAction`, `SelectPlaybackOutput` per `OutputSelection`,
//! `RunOwnerAction` per `OwnerAction`, and `ApplyServiceSetup` per
//! `ServiceKind` (design D4).

use mbv_queue::{QueueLineage, ServiceKind};
use strum::IntoEnumIterator;

use super::builders::{
    CAST_HOST, CAST_NAME, CAST_PORT, CAST_RECEIVER_ID, EMBY_DEVICE_ID, EMBY_DEVICE_NAME,
    EMBY_ITEM_ID, GENERATION, REQUEST_ID, REVISION, START_POSITION_SECS, emby_queue_item,
    feed_queue_item, progress_update, queue_source, unified_queue_replace_slots,
};
use super::*;

/// One command shape in the corpus. Coverage is compile-forced: every shape
/// enum derives `EnumIter`, `all()` iterates the top-level shapes and
/// expands each payload-carrying variant over its nested enum, and the
/// expansion match names every variant — a new shape fails this build until
/// its arm, builder, fixture row, and fixture exist. `ServiceKind` and
/// `OwnerAction` are production enums the iterator cannot construct, so
/// their shapes are flattened to one variant per case: a new production
/// variant still fails the exhaustive `shape_of` match.
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
    RunOwnerActionOpenTerminal,
    UnifiedQueueReplace,
    UnifiedQueueReplaceWithStartPosition,
    UnifiedQueueSourceUpdate,
    UnifiedQueueAppend,
    UnifiedQueueRemoveSlot,
    UnifiedQueueRemoveSlots,
    UnifiedQueueMoveSlot,
    UnifiedQueueShuffle,
    UnifiedQueuePlaySlot,
    UnifiedQueueClear,
    UnifiedQueueClearOp,
    UnifiedQueueRefresh,
    UnifiedQueueApplyProgress,
    UnifiedAdoptQueue,
    SelectPlaybackOutput(OutputChoice),
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
                CommandShape::SelectPlaybackOutput(_) => {
                    shapes.extend(OutputChoice::iter().map(CommandShape::SelectPlaybackOutput));
                }
                CommandShape::Hello
                | CommandShape::Stop
                | CommandShape::RequestShutdown
                | CommandShape::ApplyServiceSetupEmby
                | CommandShape::ApplyServiceSetupAudiobookshelf
                | CommandShape::SwapPrepared
                | CommandShape::RunOwnerActionSwapPanel
                | CommandShape::RunOwnerActionOpenTerminal
                | CommandShape::UnifiedQueueReplace
                | CommandShape::UnifiedQueueReplaceWithStartPosition
                | CommandShape::UnifiedQueueSourceUpdate
                | CommandShape::UnifiedQueueAppend
                | CommandShape::UnifiedQueueRemoveSlot
                | CommandShape::UnifiedQueueRemoveSlots
                | CommandShape::UnifiedQueueMoveSlot
                | CommandShape::UnifiedQueueShuffle
                | CommandShape::UnifiedQueuePlaySlot
                | CommandShape::UnifiedQueueClear
                | CommandShape::UnifiedQueueClearOp
                | CommandShape::UnifiedQueueRefresh
                | CommandShape::UnifiedQueueApplyProgress
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
            CommandShape::RunOwnerActionOpenTerminal => "RunOwnerAction.OpenTerminal".into(),
            CommandShape::UnifiedQueueReplace => "UnifiedQueueReplace".into(),
            CommandShape::UnifiedQueueReplaceWithStartPosition => {
                "UnifiedQueueReplace.WithStartPosition".into()
            }
            CommandShape::UnifiedQueueSourceUpdate => "UnifiedQueueSourceUpdate".into(),
            CommandShape::UnifiedQueueAppend => "UnifiedQueueAppend".into(),
            CommandShape::UnifiedQueueRemoveSlot => "UnifiedQueueRemoveSlot".into(),
            CommandShape::UnifiedQueueRemoveSlots => "UnifiedQueueRemoveSlots".into(),
            CommandShape::UnifiedQueueMoveSlot => "UnifiedQueueMoveSlot".into(),
            CommandShape::UnifiedQueueShuffle => "UnifiedQueueShuffle".into(),
            CommandShape::UnifiedQueuePlaySlot => "UnifiedQueuePlaySlot".into(),
            CommandShape::UnifiedQueueClear => "UnifiedQueueClear".into(),
            CommandShape::UnifiedQueueClearOp => "UnifiedQueueClearOp".into(),
            CommandShape::UnifiedQueueRefresh => "UnifiedQueueRefresh".into(),
            CommandShape::UnifiedQueueApplyProgress => "UnifiedQueueApplyProgress".into(),
            CommandShape::UnifiedAdoptQueue => "UnifiedAdoptQueue".into(),
            CommandShape::SelectPlaybackOutput(choice) => {
                format!("SelectPlaybackOutput.{}", choice.name())
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
            CommandShape::RunOwnerActionOpenTerminal => {
                CtrlCmd::RunOwnerAction(OwnerAction::OpenTerminal)
            }
            CommandShape::UnifiedQueueReplace => {
                let slots = unified_queue_replace_slots();
                // The legacy `items` payload mirrors the slots, as
                // `CtrlCmd::unified_queue_replace` derives it.
                let items = slots.iter().map(|slot| slot.item.clone()).collect();
                CtrlCmd::UnifiedQueueReplace {
                    items,
                    slots,
                    start_idx: Some(0),
                    source: queue_source(),
                    start_position_secs: None,
                    op: Some(QueueOpId(41)),
                }
            }
            CommandShape::UnifiedQueueReplaceWithStartPosition => {
                let slots = unified_queue_replace_slots();
                let items = slots.iter().map(|slot| slot.item.clone()).collect();
                CtrlCmd::UnifiedQueueReplace {
                    items,
                    slots,
                    start_idx: Some(0),
                    source: queue_source(),
                    start_position_secs: Some(START_POSITION_SECS),
                    op: Some(QueueOpId(41)),
                }
            }
            CommandShape::UnifiedQueueSourceUpdate => CtrlCmd::UnifiedQueueSourceUpdate {
                source: queue_source(),
                lineage: QueueLineage(2),
                op: Some(QueueOpId(42)),
            },
            CommandShape::UnifiedQueueAppend => CtrlCmd::UnifiedQueueAppend {
                items: vec![feed_queue_item()],
                before: Some(2),
                op: Some(QueueOpId(43)),
            },
            CommandShape::UnifiedQueueRemoveSlot => CtrlCmd::UnifiedQueueRemoveSlot {
                slot_id: 3,
                op: Some(QueueOpId(44)),
            },
            CommandShape::UnifiedQueueRemoveSlots => CtrlCmd::UnifiedQueueRemoveSlots {
                slot_ids: vec![3, 4],
                op: Some(QueueOpId(45)),
            },
            CommandShape::UnifiedQueueMoveSlot => CtrlCmd::UnifiedQueueMoveSlot {
                slot_id: 4,
                to_index: 1,
                op: Some(QueueOpId(46)),
            },
            CommandShape::UnifiedQueueShuffle => CtrlCmd::UnifiedQueueShuffle {
                op: Some(QueueOpId(47)),
            },
            CommandShape::UnifiedQueuePlaySlot => CtrlCmd::UnifiedQueuePlaySlot {
                slot_id: 2,
                op: Some(QueueOpId(48)),
            },
            CommandShape::UnifiedQueueClear => CtrlCmd::UnifiedQueueClear,
            CommandShape::UnifiedQueueClearOp => CtrlCmd::UnifiedQueueClearOp { op: QueueOpId(49) },
            CommandShape::UnifiedQueueRefresh => CtrlCmd::UnifiedQueueRefresh { op: QueueOpId(50) },
            CommandShape::UnifiedQueueApplyProgress => CtrlCmd::UnifiedQueueApplyProgress {
                op: QueueOpId(51),
                updates: vec![progress_update()],
            },
            CommandShape::UnifiedAdoptQueue => CtrlCmd::UnifiedAdoptQueue {
                items: vec![emby_queue_item()],
                cursor: 1,
                source: queue_source(),
            },
            CommandShape::SelectPlaybackOutput(choice) => {
                CtrlCmd::SelectPlaybackOutput(choice.build())
            }
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
            OwnerAction::OpenTerminal => CommandShape::RunOwnerActionOpenTerminal,
        },
        CtrlCmd::UnifiedQueueReplace {
            start_position_secs: Some(_),
            ..
        } => CommandShape::UnifiedQueueReplaceWithStartPosition,
        CtrlCmd::UnifiedQueueReplace { .. } => CommandShape::UnifiedQueueReplace,
        CtrlCmd::UnifiedQueueSourceUpdate { .. } => CommandShape::UnifiedQueueSourceUpdate,
        CtrlCmd::UnifiedQueueAppend { .. } => CommandShape::UnifiedQueueAppend,
        CtrlCmd::UnifiedQueueRemoveSlot { .. } => CommandShape::UnifiedQueueRemoveSlot,
        CtrlCmd::UnifiedQueueRemoveSlots { .. } => CommandShape::UnifiedQueueRemoveSlots,
        CtrlCmd::UnifiedQueueMoveSlot { .. } => CommandShape::UnifiedQueueMoveSlot,
        CtrlCmd::UnifiedQueueShuffle { .. } => CommandShape::UnifiedQueueShuffle,
        CtrlCmd::UnifiedQueuePlaySlot { .. } => CommandShape::UnifiedQueuePlaySlot,
        CtrlCmd::UnifiedQueueClear => CommandShape::UnifiedQueueClear,
        CtrlCmd::UnifiedQueueClearOp { .. } => CommandShape::UnifiedQueueClearOp,
        CtrlCmd::UnifiedQueueRefresh { .. } => CommandShape::UnifiedQueueRefresh,
        CtrlCmd::UnifiedQueueApplyProgress { .. } => CommandShape::UnifiedQueueApplyProgress,
        CtrlCmd::UnifiedAdoptQueue { .. } => CommandShape::UnifiedAdoptQueue,
        CtrlCmd::SelectPlaybackOutput(selection) => {
            CommandShape::SelectPlaybackOutput(output_of(selection))
        }
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
