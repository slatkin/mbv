use mbv_surfaces::media_list::MediaListOperation;
use ratatui::layout::Position;

/// Pointer surface input resolved by a mounted presentation. Convert this to
/// a target-bearing operation before delegating to the canonical owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaListSurfaceInput {
    Move(i64),
    Page(i64),
    First,
    Last,
    Activate,
    Context,
    Click(Position),
    ToggleClick(Position),
    RangeClick(Position),
    DoubleClick(Position),
    ContextClick(Position),
    Wheel { at: Position, delta: i64 },
}

impl MediaListSurfaceInput {
    pub fn into_operation<Target>(
        self,
        target: Option<Target>,
    ) -> Option<MediaListOperation<Target>> {
        Some(match self {
            Self::Move(delta) => MediaListOperation::Move(delta),
            Self::Wheel { delta, .. } => MediaListOperation::Scroll(delta),
            Self::Page(delta) => MediaListOperation::Page(delta),
            Self::First => MediaListOperation::First,
            Self::Last => MediaListOperation::Last,
            Self::Activate => MediaListOperation::ActivateCurrent,
            Self::Context => MediaListOperation::ContextCurrent,
            Self::Click(_) => MediaListOperation::Select(target?),
            Self::ToggleClick(_) => MediaListOperation::Toggle(target?),
            Self::RangeClick(_) => MediaListOperation::Range(target?),
            Self::DoubleClick(_) => MediaListOperation::Activate(target?),
            Self::ContextClick(_) => MediaListOperation::Context(target?),
        })
    }
}
