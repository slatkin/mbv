use crate::app::components::library_panel::LibraryKey;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

/// A bounded percentage used by active canonical media-list rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveProgress(u8);

impl ActiveProgress {
    /// Clamps a percentage into the permitted `0..=100` range.
    pub fn new(percent: u16) -> Self {
        Self(percent.min(100) as u8)
    }

    pub fn percent(self) -> u8 {
        self.0
    }
}

/// Whether a canonical media-list item drills into a container or plays a leaf.
///
/// `Collection` rows are navigable containers (movie/series folder, album,
/// podcast show, book title); `Media` rows are playable leaves (queue entries,
/// episodes, tracks, chapters, feed items). The painter suppresses the duration
/// slot for `Collection` rows even when one is projected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Collection,
    Media,
}

/// Provider-neutral semantic state used by canonical media-list rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaSemanticState {
    Ordinary,
    Played,
    Active { progress: Option<ActiveProgress> },
    NowPlaying { progress: Option<ActiveProgress> },
}

/// The closed title-reveal policy of one list: whether every row paints its
/// full title, or only the list's selected row does. Declared once by the
/// destination that composes the list (`set_title_reveal`) and applied by the
/// shared row painter, so a destination never paints or branches its own rows
/// to express it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MediaListTitleReveal {
    /// Every row paints its full title; a split row paints both parts.
    #[default]
    Always,
    /// A row outside the list's selection paints only its primary text; the
    /// selected row paints the full title (marqueed per the selected-row
    /// marquee rule while the list is focused).
    OnSelection,
}

impl MediaSemanticState {
    /// Constructs active state, clamping prepared progress to the permitted range.
    pub fn active(progress: Option<u16>) -> Self {
        Self::Active {
            progress: progress.map(ActiveProgress::new),
        }
    }

    /// The canonical state/colour policy, so no destination decides a row
    /// colour: a positive resume position yields `Active` with the bounded
    /// percentage (no percentage when the runtime is unknown); `played`
    /// without a position yields `Played`; otherwise the row is `Ordinary`.
    /// Lists derive from an item through [`Self::from_emby`] /
    /// [`Self::from_queue_item`], which add the music rule on top of this.
    pub fn from_progress(played: bool, position_ticks: i64, runtime_ticks: i64) -> Self {
        if position_ticks > 0 {
            let progress = (runtime_ticks > 0).then(|| {
                (u128::try_from(position_ticks)
                    .unwrap_or(u128::MAX)
                    .saturating_mul(100)
                    / u128::try_from(runtime_ticks).unwrap_or(u128::MAX))
                .min(100) as u16
            });
            Self::active(progress)
        } else if played {
            Self::Played
        } else {
            Self::Ordinary
        }
    }

    /// The one item-level derivation for an Emby row: a music item (track,
    /// album, artist) is fire-and-forget, so its stored played/resume facts
    /// are ignored and its row is always `Ordinary`. Only the row that is
    /// actually playing shows state, and the Queue paints that one as
    /// `NowPlaying` itself.
    pub fn from_emby(item: &EmbyItem) -> Self {
        Self::music_or_progress(
            item.is_music(),
            item.played,
            item.playback_position_ticks,
            item.runtime_ticks,
        )
    }

    /// [`Self::from_emby`] for a queue item: the same rule over the item's
    /// own stored facts.
    pub fn from_queue_item(item: &QueueItem) -> Self {
        Self::music_or_progress(
            item.is_music(),
            item.played(),
            item.playback_position_ticks(),
            item.runtime_ticks(),
        )
    }

    fn music_or_progress(
        is_music: bool,
        played: bool,
        position_ticks: i64,
        runtime_ticks: i64,
    ) -> Self {
        if is_music {
            Self::Ordinary
        } else {
            Self::from_progress(played, position_ticks, runtime_ticks)
        }
    }
}

/// Which semantic surface should receive the selected-row treatment.
///
/// The policy is deliberately closed: callers choose a named surface identity,
/// never a raw Ratatui style or colour. The painter retains the identity for
/// composition policy while every selected row resolves to the canonical
/// selected-row bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedRowSurface {
    ListBackdrop,
    OwningQueueColumn,
    /// The library Workspace (Wide Hero pane and Library Hero overlay — one
    /// unified look), retained as a closed owning-surface identity.
    OwningLibraryPane,
}

/// Semantic paint policy for one `WideMediaList` view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZebraStripe {
    pub focused: Color,
    pub unfocused: Color,
}

impl ZebraStripe {
    /// One fill in both focus states.
    pub const fn fixed(fill: Color) -> Self {
        Self {
            focused: fill,
            unfocused: fill,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WideMediaListPaintPolicy {
    focused: bool,
    selected_surface: SelectedRowSurface,
    zebra: Option<ZebraStripe>,
}

impl WideMediaListPaintPolicy {
    pub const fn new(focused: bool) -> Self {
        Self {
            focused,
            selected_surface: SelectedRowSurface::ListBackdrop,
            zebra: None,
        }
    }

    pub const fn for_queue(focused: bool) -> Self {
        Self {
            focused,
            selected_surface: SelectedRowSurface::OwningQueueColumn,
            zebra: None,
        }
    }

    pub const fn for_library_workspace(focused: bool) -> Self {
        Self {
            focused,
            selected_surface: SelectedRowSurface::OwningLibraryPane,
            zebra: None,
        }
    }

    pub const fn with_zebra(mut self, zebra: ZebraStripe) -> Self {
        self.zebra = Some(zebra);
        self
    }

    pub(crate) fn zebra_bg(self) -> Option<Color> {
        self.zebra.map(|zebra| {
            if self.focused {
                zebra.focused
            } else {
                zebra.unfocused
            }
        })
    }

    pub(crate) const fn focused(self) -> bool {
        self.focused
    }

    pub(crate) const fn selected_surface(self) -> SelectedRowSurface {
        self.selected_surface
    }
}

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

/// Stable coordination identity for a `MediaList` selection. This identifies
/// the list that produced a projection or delayed action; it never carries
/// selection membership.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SelectionOrigin {
    Library(LibrarySelectionOrigin),
    Queue,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LibrarySelectionOrigin {
    Home,
    Feeds,
    Service(LibraryKey),
}

/// Read-only presentation projection of a `MediaList` selection. Membership is
/// deliberately private to the owner and cannot be reconstructed here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionSummary {
    pub count: usize,
    pub origin: SelectionOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaListTransition<Target> {
    pub disposition: MediaListDisposition,
    pub selected_target: Option<Target>,
    pub selection_summary: Option<SelectionSummary>,
    pub external_intent: Option<RowIntent<Target>>,
}

impl<Target> MediaListTransition<Target> {
    pub(crate) fn unhandled() -> Self {
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

/// Metadata a row paints in the fixed-width right-aligned gutter, with the
/// green metadata role baked in (one closed vocabulary: rows never carry raw
/// colours). This carries release dates/years and minutes-precision durations;
/// resume/live progress is not trailing metadata: it comes from the row's
/// [`MediaSemanticState`], so `Active`/`NowPlaying` render it inline. A row
/// without gutter metadata reserves no gutter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaListTrailing {
    /// The item's release year, publish date, or minutes-precision runtime,
    /// painted right-aligned in the fixed-width gutter in the green
    /// (`STATUS_AVAILABLE`) metadata role.
    Gutter(String),
}

/// A closed, provider-neutral row vocabulary for embedded media lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaListRow<Target> {
    Item {
        target: Target,
        primary: String,
        /// Optional secondary title of a split row (e.g. an episode title)
        /// painted after `primary` with one space: `primary` is the
        /// container/context name in gold (`PLAYBACK_CONTEXT_FG`), secondary
        /// in aqua (`PLAYBACK_TITLE_FG`); a played row mutes only the
        /// secondary to (`TEXT_MUTED`).
        secondary: Option<String>,
        /// Gutter metadata rendered in the fixed right-aligned green column.
        /// Inline progress percentages are derived from `semantic_state` in
        /// the orange (`PROGRESS_PERCENT`) role. Distinct from `duration`, the
        /// right-aligned deep gold time slot.
        trailing: Option<MediaListTrailing>,
        /// A duration/time string. Rendered as a distinct right-aligned
        /// deep gold (`DURATION`) element, never as `trailing`.
        duration: Option<String>,
        /// Drill-in container vs. playable leaf; gates the duration slot.
        kind: MediaKind,
        semantic_state: MediaSemanticState,
    },
    Heading {
        text: String,
    },
    Spacer,
}

impl<Target> MediaListRow<Target> {
    /// Returns the stable identity only for selectable item rows.
    pub fn selectable_target(&self) -> Option<&Target> {
        match self {
            Self::Item { target, .. } => Some(target),
            Self::Heading { .. } | Self::Spacer => None,
        }
    }
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

/// Flow-space geometry for a painted media-list control.
///
/// Rows contain the source-row lookup used by painters and an optional stable
/// target for hit maps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowGeometry<Target> {
    offset: usize,
    rows: Vec<Option<Target>>,
    selected_row: Option<usize>,
}

impl<Target> RowGeometry<Target> {
    /// Display-row index at the viewport top.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Number of rows in the complete painted flow.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Display-row index of the selected row in flow space.
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row
    }

    fn row_rect(&self, area: Rect, flow_row: usize) -> Rect {
        Rect {
            y: area.y
                + u16::try_from(flow_row - self.offset)
                    .expect("visible row offset is bounded by the painted rect height"),
            height: 1,
            ..area
        }
    }

    /// The selected row's absolute one-line rectangle when it is visible.
    pub fn selected_row_rect(&self, area: Rect) -> Option<Rect> {
        let row = self.selected_row?;
        (self.offset..self.offset.saturating_add(area.height as usize))
            .contains(&row)
            .then(|| self.row_rect(area, row))
    }

    /// Produce target-bearing rectangles for the visible portion of the
    /// completed fixed-row paint. Structural rows remain in the flow but do
    /// not enter retained hit geometry.
    pub(crate) fn target_rects(&self, claim_rect: Rect, content_rect: Rect) -> Vec<(Rect, Target)>
    where
        Target: Clone,
    {
        let end = self
            .offset
            .saturating_add(content_rect.height as usize)
            .min(self.rows.len());
        (self.offset..end)
            .filter_map(|flow_row| {
                let target = self.rows.get(flow_row)?.clone()?;
                // Rows anchor at `content_rect.y` (where the list paints)
                // but inherit x/width from the claim rectangle.
                let rect = Rect {
                    y: content_rect.y
                        + u16::try_from(flow_row - self.offset)
                            .expect("visible row offset is bounded by the painted rect height"),
                    height: 1,
                    ..claim_rect
                };
                Some((rect, target))
            })
            .collect()
    }
}

impl<Target: Clone> RowGeometry<Target> {
    pub(crate) fn source(
        rows: &[MediaListRow<Target>],
        offset: usize,
        selected_row: Option<usize>,
    ) -> Self {
        Self {
            offset,
            rows: rows
                .iter()
                .map(|row| row.selectable_target().cloned())
                .collect(),
            selected_row,
        }
    }
}
