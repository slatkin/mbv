use super::{IntroState, NextUp};
use crate::playback_queue::{AudiobookshelfItem, QueueItem, QueueSlotId};
use mbv_ids::ItemId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::player) struct ActiveItemState {
    pub(in crate::player) osd_title: String,
    pub(in crate::player) last_valid_pos: i64,
    pub(in crate::player) series_id: ItemId,
    pub(in crate::player) season: i64,
    pub(in crate::player) episode: i64,
    pub(in crate::player) intro_state: IntroState,
}

pub(in crate::player) fn active_item_state(item: Option<&QueueItem>) -> ActiveItemState {
    let mut state = ActiveItemState {
        osd_title: String::new(),
        last_valid_pos: 0,
        series_id: ItemId::empty(),
        season: 0,
        episode: 0,
        intro_state: IntroState::Pending,
    };
    match item {
        Some(QueueItem::Emby(item)) => {
            state.osd_title = item.display_name();
            state.last_valid_pos = if item.is_audio() {
                0
            } else {
                item.playback_position_ticks
            };
            if item.item_type == "Episode" {
                state.series_id = ItemId::new(item.series_id.clone());
                state.season = item.parent_index_number;
                state.episode = item.index_number;
            }
        }
        Some(QueueItem::Feed(item)) => {
            state.osd_title.clone_from(&item.title);
            let runtime = i64::try_from(item.duration_ticks.unwrap_or(0)).unwrap_or(i64::MAX);
            state.last_valid_pos = if crate::api::should_resume(item.position_ticks, runtime) {
                item.position_ticks
            } else {
                0
            };
        }
        Some(QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(item))) => {
            state.osd_title.clone_from(&item.title);
            let runtime = i64::try_from(item.duration_ticks.unwrap_or(0)).unwrap_or(i64::MAX);
            state.last_valid_pos = if crate::api::should_resume(item.position_ticks, runtime) {
                item.position_ticks
            } else {
                0
            };
        }
        Some(QueueItem::Audiobookshelf(AudiobookshelfItem::Book(item))) => {
            state.osd_title.clone_from(&item.title);
            let runtime = i64::try_from(item.duration_ticks.unwrap_or(0)).unwrap_or(i64::MAX);
            state.last_valid_pos = if crate::api::should_resume(item.position_ticks, runtime) {
                item.position_ticks
            } else {
                0
            };
        }
        None => {}
    }
    state
}

pub(in crate::player) fn resolve_jump_target(
    slots: &[QueueSlotId],
    target: QueueSlotId,
) -> Option<usize> {
    slots.iter().position(|slot| *slot == target)
}

pub(in crate::player) fn seek_decision(seconds: f64, absolute: bool) -> (&'static str, String) {
    (
        if absolute { "absolute" } else { "relative" },
        seconds.to_string(),
    )
}

pub(in crate::player) fn volume_decision(requested: i64, maximum: i64) -> (i64, i64) {
    let volume = requested.clamp(0, maximum);
    #[expect(
        clippy::cast_precision_loss,
        reason = "player volume (i64) → f64 for the mpv cube-root volume curve; the curve has no integer path (approved, issue #804)"
    )]
    let raw = super::super::saturating_i64_from_f64((10.0 * (volume as f64).sqrt()).round());
    (volume, raw)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::player) enum NextUpFire {
    Queue(usize),
    Standalone,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::player) struct NextUpDecision {
    pub(in crate::player) reset: bool,
    pub(in crate::player) arm: bool,
    pub(in crate::player) fire: Option<NextUpFire>,
}

pub(in crate::player) fn queue_next_up_decision(
    state: NextUp,
    current_idx: usize,
    queue_len: usize,
    current_is_episode: bool,
    next_is_episode: bool,
    runtime: i64,
    ticks: i64,
) -> NextUpDecision {
    const MIN_RUNTIME: i64 = 600 * crate::api::TICKS_PER_SECOND;
    const MIN_REMAIN: i64 = 20 * crate::api::TICKS_PER_SECOND;
    const WINDOW: i64 = 60 * crate::api::TICKS_PER_SECOND;
    const ARM_WINDOW: i64 = 5 * crate::api::TICKS_PER_SECOND;
    if current_idx + 1 >= queue_len || !current_is_episode || !next_is_episode || runtime <= 0 {
        return NextUpDecision::default();
    }
    let show_at = runtime - WINDOW;
    let reset = state.is_fired() && ticks < show_at;
    let state = if reset { NextUp::Idle } else { state };
    if state.is_fired() || runtime < MIN_RUNTIME {
        return NextUpDecision {
            reset,
            ..NextUpDecision::default()
        };
    }
    if runtime - ticks >= MIN_REMAIN && ticks >= show_at {
        return NextUpDecision {
            reset,
            fire: Some(NextUpFire::Queue(current_idx + 1)),
            ..NextUpDecision::default()
        };
    }
    let arm = state == NextUp::Idle && ticks > 0 && ticks < ARM_WINDOW;
    NextUpDecision {
        reset,
        arm,
        fire: None,
    }
}

pub(in crate::player) fn standalone_next_up_decision(
    state: NextUp,
    has_series: bool,
    runtime: i64,
    ticks: i64,
) -> NextUpDecision {
    const WINDOW: i64 = 60 * crate::api::TICKS_PER_SECOND;
    const ARM_WINDOW: i64 = 5 * crate::api::TICKS_PER_SECOND;
    if state.is_fired() {
        return NextUpDecision::default();
    }
    if !has_series {
        return NextUpDecision {
            arm: state == NextUp::Idle && ticks > 0 && ticks < ARM_WINDOW,
            ..NextUpDecision::default()
        };
    }
    if runtime > WINDOW && ticks > runtime - WINDOW {
        return NextUpDecision {
            fire: Some(NextUpFire::Standalone),
            ..NextUpDecision::default()
        };
    }
    NextUpDecision {
        arm: state == NextUp::Idle && ticks > 0 && ticks < ARM_WINDOW,
        ..NextUpDecision::default()
    }
}

pub(in crate::player) struct AdvanceDecisionInput {
    pub(in crate::player) media: CompletedMedia,
    pub(in crate::player) finish: FinishReason,
    pub(in crate::player) last_valid_pos: i64,
}

#[derive(Copy, Clone)]
pub(in crate::player) enum CompletedMedia {
    Audio,
    Video,
}

#[derive(Copy, Clone)]
pub(in crate::player) enum FinishReason {
    Natural,
    NearEnd,
    NextUp,
    Unfinished,
}

pub(in crate::player) fn advance_decision(input: &AdvanceDecisionInput) -> (bool, bool, bool, i64) {
    let finished = !matches!(input.finish, FinishReason::Unfinished);
    let played = finished && matches!(input.media, CompletedMedia::Video);
    let (natural, near_end) = match input.finish {
        FinishReason::Natural => (true, false),
        FinishReason::NearEnd => (false, true),
        FinishReason::NextUp | FinishReason::Unfinished => (false, false),
    };
    let position = crate::player::queue_completed_pos(
        matches!(input.media, CompletedMedia::Audio),
        natural,
        near_end,
        input.last_valid_pos,
    );
    (finished, played, finished, position)
}
