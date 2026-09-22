use crate::app::components::library_panel::LibraryKey;
use crate::app::palette;
use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::Position;
use ratatui::style::Color;

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
            let progress = (runtime_ticks > 0)
                .then(|| ((position_ticks as u128 * 100) / runtime_ticks as u128).min(100) as u16);
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

/// The Queue's row palette, shared by the Queue and Grouped Music tree so
/// their base and zebra fills cannot drift apart. The base fill is the
/// recessed QueuePanel surface; the stripe is the QueueColumn surface.
pub(crate) fn queue_row_background(focused: bool) -> Color {
    palette::surface_colors(palette::Surface::QueuePanel, focused).fill
}

pub(crate) fn queue_row_zebra(focused: bool) -> Color {
    palette::surface_colors(palette::Surface::QueueColumn, focused).fill
}

pub(crate) fn queue_row_zebra_stripe() -> ZebraStripe {
    ZebraStripe {
        focused: queue_row_zebra(true),
        unfocused: queue_row_zebra(false),
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

impl MediaListSurfaceInput {
    pub fn into_operation<Target>(
        self,
        target: Option<Target>,
    ) -> Option<MediaListOperation<Target>> {
        Some(match self {
            Self::Move(delta) => MediaListOperation::Move(delta),
            Self::Page(delta) => MediaListOperation::Page(delta),
            Self::First => MediaListOperation::First,
            Self::Last => MediaListOperation::Last,
            Self::Activate => MediaListOperation::ActivateCurrent,
            Self::Context => MediaListOperation::ContextCurrent,
            Self::Wheel { delta, .. } => MediaListOperation::Move(delta),
            Self::Click(_) => MediaListOperation::Select(target?),
            Self::ToggleClick(_) => MediaListOperation::Toggle(target?),
            Self::RangeClick(_) => MediaListOperation::Range(target?),
            Self::DoubleClick(_) => MediaListOperation::Activate(target?),
            Self::ContextClick(_) => MediaListOperation::Context(target?),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaListDisposition {
    Unhandled,
    Consumed,
}

/// Stable coordination identity for a MediaList selection. This identifies
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

/// Read-only presentation projection of a MediaList selection. Membership is
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

/// The text a list's marquee clock keys on for one row: the full title text the
/// painter marquees — a split row's context text and item title, one space
/// apart — or `None` for a row with no title. One formula for both sides: the
/// presenter keys its clock with this text and hands the painter the same
/// string, so the two cannot drift. A drift makes the clock restart every frame
/// and the marquee hold at the start forever.
pub(crate) fn row_marquee_key<Target>(row: &MediaListRow<Target>) -> Option<String> {
    match row {
        MediaListRow::Item {
            primary, secondary, ..
        } => Some(match secondary.as_deref().filter(|sec| !sec.is_empty()) {
            Some(sec) => format!("{primary} {sec}"),
            None => primary.clone(),
        }),
        MediaListRow::Heading { .. } | MediaListRow::Spacer => None,
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

impl WideViewport {}
