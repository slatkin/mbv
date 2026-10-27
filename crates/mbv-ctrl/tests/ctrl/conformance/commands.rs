//! Command (`commands/`) corpus: one golden `CtrlCmd` per variant —
//! `PlayerCmd` per `WireCommand` tag, `PlaybackIntent` per
//! `PlaybackIntentAction`, `SelectPlaybackOutput` per `OutputSelection`,
//! `RunOwnerAction` per `OwnerAction`, and `ApplyServiceSetup` per
//! `ServiceKind` (design D4).

use mbv_emby_model::{EmbyImageTags, EmbyItem};
use mbv_queue::{
    FeedEntry, FeedKind, QueueItem, QueueItemContentId, QueueLineage, QueueSource, ServiceKind,
};

use super::*;

/// Fixed payload literals: the corpus pins wire shapes, not runtime state,
/// so every builder below uses these constants and never reads the
/// environment or the clock (design note N2, matching the hello corpus).
const EMBY_ITEM_ID: &str = "emby-conformance-item";
const EMBY_ITEM_NAME: &str = "Conformance Episode";
const FEED_GUID: &str = "conformance-feed-guid";
const FEED_TITLE: &str = "Conformance Feed Entry";
const FEED_URL: &str = "https://feeds.example/conformance/episode-1.mp3";
const FEED_ID: &str = "https://feeds.example/conformance.xml";
const PLAYLIST_NAME: &str = "Conformance Playlist";
const REQUEST_ID: u64 = 7;
const GENERATION: u64 = 3;
const REVISION: u64 = 4;
const CAST_RECEIVER_ID: &str = "cast-receiver-1";
const CAST_NAME: &str = "Conformance Display";
const CAST_HOST: &str = "192.168.1.50";
const CAST_PORT: u16 = 8009;
const EMBY_DEVICE_ID: &str = "emby-device-1";
const EMBY_DEVICE_NAME: &str = "Conformance Session";

/// One command shape in the corpus. The exhaustive shape-name match keeps
/// every shape named, and `rows` gives each one a builder and a fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandShape {
    Hello,
    PlayerCmd(WireTag),
    Stop,
    PlaybackIntent(IntentAction),
    RequestShutdown,
    ApplyServiceSetup(ServiceKind),
    SwapPrepared,
    RunOwnerAction(OwnerAction),
    UnifiedQueueReplace,
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
    /// Every command shape: each flat variant plus one per nested-enum
    /// variant. Hand-kept, like `HelloShape::ALL`.
    fn all() -> Vec<Self> {
        let mut shapes = vec![
            CommandShape::Hello,
            CommandShape::Stop,
            CommandShape::RequestShutdown,
            CommandShape::SwapPrepared,
            CommandShape::UnifiedQueueReplace,
            CommandShape::UnifiedQueueSourceUpdate,
            CommandShape::UnifiedQueueAppend,
            CommandShape::UnifiedQueueRemoveSlot,
            CommandShape::UnifiedQueueRemoveSlots,
            CommandShape::UnifiedQueueMoveSlot,
            CommandShape::UnifiedQueueShuffle,
            CommandShape::UnifiedQueuePlaySlot,
            CommandShape::UnifiedQueueClear,
            CommandShape::UnifiedQueueClearOp,
            CommandShape::UnifiedQueueRefresh,
            CommandShape::UnifiedQueueApplyProgress,
            CommandShape::UnifiedAdoptQueue,
        ];
        shapes.extend(WireTag::ALL.map(CommandShape::PlayerCmd));
        shapes.extend(IntentAction::ALL.map(CommandShape::PlaybackIntent));
        shapes.extend(SERVICE_KINDS.map(CommandShape::ApplyServiceSetup));
        shapes.extend(
            OwnerAction::ALL
                .iter()
                .map(|action| CommandShape::RunOwnerAction(*action)),
        );
        shapes.extend(OutputChoice::ALL.map(CommandShape::SelectPlaybackOutput));
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
            CommandShape::ApplyServiceSetup(kind) => {
                format!("ApplyServiceSetup.{}", service_kind_name(kind))
            }
            CommandShape::SwapPrepared => "SwapPrepared".into(),
            CommandShape::RunOwnerAction(action) => {
                format!("RunOwnerAction.{}", owner_action_name(action))
            }
            CommandShape::UnifiedQueueReplace => "UnifiedQueueReplace".into(),
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
            CommandShape::ApplyServiceSetup(kind) => CtrlCmd::ApplyServiceSetup {
                kind,
                revision: REVISION,
            },
            CommandShape::SwapPrepared => CtrlCmd::SwapPrepared,
            CommandShape::RunOwnerAction(action) => CtrlCmd::RunOwnerAction(action),
            CommandShape::UnifiedQueueReplace => {
                let slots = vec![
                    UnifiedQueueSlot {
                        slot_id: 1,
                        item: emby_queue_item(),
                    },
                    UnifiedQueueSlot {
                        slot_id: 2,
                        item: feed_queue_item(),
                    },
                ];
                // The legacy `items` payload mirrors the slots, as
                // `CtrlCmd::unified_queue_replace` derives it.
                let items = slots.iter().map(|slot| slot.item.clone()).collect();
                CtrlCmd::UnifiedQueueReplace {
                    items,
                    slots,
                    start_idx: Some(0),
                    source: queue_source(),
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
        CtrlCmd::ApplyServiceSetup { kind, .. } => CommandShape::ApplyServiceSetup(*kind),
        CtrlCmd::SwapPrepared => CommandShape::SwapPrepared,
        CtrlCmd::RunOwnerAction(action) => CommandShape::RunOwnerAction(*action),
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WireTag {
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
    const ALL: [WireTag; 12] = [
        WireTag::TogglePause,
        WireTag::JumpTo,
        WireTag::SetVolume,
        WireTag::Seek,
        WireTag::SeekAbsolute,
        WireTag::SetAudio,
        WireTag::SetSub,
        WireTag::SetSubtitlePrefs,
        WireTag::SetMute,
        WireTag::NextUpShow,
        WireTag::NextUpDismiss,
        WireTag::SkipIntroDismiss,
    ];

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IntentAction {
    Play,
    Stop,
    SetPaused,
    Next,
    Previous,
}

impl IntentAction {
    const ALL: [IntentAction; 5] = [
        IntentAction::Play,
        IntentAction::Stop,
        IntentAction::SetPaused,
        IntentAction::Next,
        IntentAction::Previous,
    ];

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
/// `SelectPlaybackOutput.<selection>` fixture row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputChoice {
    Local,
    Cast,
    EmbySession,
}

impl OutputChoice {
    const ALL: [OutputChoice; 3] = [
        OutputChoice::Local,
        OutputChoice::Cast,
        OutputChoice::EmbySession,
    ];

    /// Exhaustive: a new `OutputSelection` variant fails this build until
    /// its arm and fixture row exist.
    fn name(self) -> &'static str {
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
fn output_of(selection: &OutputSelection) -> OutputChoice {
    match selection {
        OutputSelection::Local => OutputChoice::Local,
        OutputSelection::Cast { .. } => OutputChoice::Cast,
        OutputSelection::EmbySession { .. } => OutputChoice::EmbySession,
    }
}

/// Exhaustive name match over `OwnerAction` with NO wildcard arm: a new
/// action fails this build until its arm and fixture row exist.
fn owner_action_name(action: OwnerAction) -> &'static str {
    match action {
        OwnerAction::SwapPanel => "SwapPanel",
        OwnerAction::OpenTerminal => "OpenTerminal",
    }
}

/// Both `ServiceKind`s, each with an `ApplyServiceSetup.<kind>` fixture row.
const SERVICE_KINDS: [ServiceKind; 2] = [ServiceKind::Emby, ServiceKind::Audiobookshelf];

/// Exhaustive name match over `ServiceKind` with NO wildcard arm.
fn service_kind_name(kind: ServiceKind) -> &'static str {
    match kind {
        ServiceKind::Emby => "Emby",
        ServiceKind::Audiobookshelf => "Audiobookshelf",
    }
}

// ── Fixed-literal payload builders ───────────────────────────────────────

/// The playlist `QueueSource` the queue-command fixtures carry.
fn queue_source() -> QueueSource {
    QueueSource::Playlist {
        id: Some("conformance-playlist".to_owned()),
        name: PLAYLIST_NAME.to_owned(),
    }
}

/// The Emby queue item the queue-command fixtures carry.
fn emby_queue_item() -> QueueItem {
    QueueItem::Emby(Box::new(EmbyItem {
        id: EMBY_ITEM_ID.to_owned(),
        name: EMBY_ITEM_NAME.to_owned(),
        item_type: "Episode".to_owned(),
        is_folder: false,
        child_count: None,
        studios: Vec::new(),
        media_type: "Video".to_owned(),
        collection_type: String::new(),
        runtime_ticks: 1_800_000_000,
        played: false,
        playback_position_ticks: 0,
        series_id: "emby-series-1".to_owned(),
        series_name: "Conformance Series".to_owned(),
        album_id: String::new(),
        album: String::new(),
        index_number: 1,
        parent_index_number: 1,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        track_artist: String::new(),
        artist_items: Vec::new(),
        sort_name: "conformance episode".to_owned(),
        production_year: 2026,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: "2026-01-01T00:00:00Z".to_owned(),
        total_count: 0,
        container: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genres: Vec::new(),
        people: Vec::new(),
        external_urls: Vec::new(),
        playlist_item_id: String::new(),
        image_tags: EmbyImageTags::default(),
    }))
}

/// The feed queue item the queue-command fixtures carry.
fn feed_queue_item() -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: FEED_GUID.to_owned(),
        title: FEED_TITLE.to_owned(),
        enclosure_url: Some(FEED_URL.to_owned()),
        link: None,
        mime_type: Some("audio/mpeg".to_owned()),
        duration_ticks: Some(1_800_000_000),
        pub_date_secs: Some(1_700_000_000),
        feed_kind: Some(FeedKind::Audio),
        feed_id: Some(FEED_ID.to_owned()),
        position_ticks: 0,
        played: false,
    })
}

/// The provider-qualified progress update the progress-relay fixture
/// carries: an Audiobookshelf episode position, redacted of everything but
/// identity and progress.
fn progress_update() -> ProgressUpdate {
    ProgressUpdate {
        content_id: QueueItemContentId::Audiobookshelf {
            library_item_id: "abs-library-item-1".to_owned(),
            episode_id: "abs-episode-1".to_owned(),
        },
        position_ticks: 30_000_000,
        finished: false,
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
            schema: Some(schema::Root::Cmd),
            value: fixture_value(value),
        });
    }
}
