use mbv_ui_msg::SelectionSummary;
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

/// Target-resolved operation accepted by the canonical media-list owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaListOperation<Target> {
    Move(i64),
    Page(i64),
    First,
    Last,
    ActivateCurrent,
    ContextCurrent,
    Select(Target),
    Toggle(Target),
    Range(Target),
    Activate(Target),
    Context(Target),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaListDisposition {
    Unhandled,
    Consumed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaListTransition<Target> {
    pub disposition: MediaListDisposition,
    pub selected_target: Option<Target>,
    pub selection_summary: Option<SelectionSummary>,
    pub external_intent: Option<RowIntent<Target>>,
}

impl<Target> MediaListTransition<Target> {
    pub fn unhandled() -> Self {
        Self {
            disposition: MediaListDisposition::Unhandled,
            selected_target: None,
            selection_summary: None,
            external_intent: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowIntent<Target> {
    Activate(Target),
    Context(Target),
    ContextSelection(Vec<Target>),
}

/// The clamped one-column viewport of a [`MediaList`] for a given painted
/// height: `offset` is the display-row index at the viewport top, so display
/// row `i` paints at screen row `i - offset`. `total_rows` counts every
/// display row (items, headings, spacers alike).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WideViewport {
    pub offset: usize,
    pub height: usize,
    pub total_rows: usize,
}
