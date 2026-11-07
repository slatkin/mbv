use crate::library::LibraryKey;
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;

/// A bounded percentage used by active canonical media-list rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveProgress(u8);

impl ActiveProgress {
    /// Clamps a percentage into the permitted `0..=100` range.
    #[must_use]
    pub fn new(percent: u16) -> Self {
        Self(percent.min(100) as u8)
    }

    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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

/// Whether the media-list row at `row_index` paints the zebra fill. The one
/// alternation rule both Clients' painters share (`mbv-render`'s wide media
/// list and the GUI queue frame): headings and spacers never stripe and
/// never flip the running fill; an ungrouped list's item takes the running
/// `striped` state and flips it; a grouped list's member alternates within
/// its group from the group's first member — the row below its heading, the
/// list's first row when it has none above the row. `grouped` is whether the
/// list carries any heading row; `rows` reaches the list's source rows by
/// index, `stop_row` inclusive; callers seed `striped` from their viewport
/// offset, so the parity stays stable while the viewport moves.
#[must_use]
pub fn zebra_striped<'a, Target: 'a>(
    row: &'a MediaListRow<Target>,
    row_index: usize,
    rows: impl Fn(usize) -> &'a MediaListRow<Target>,
    grouped: bool,
    striped: &mut bool,
) -> bool {
    match row {
        MediaListRow::Heading { .. } | MediaListRow::Spacer => false,
        MediaListRow::Item { .. } if grouped => {
            zebra_group_member_striped(row_index, zebra_group_start(&rows, row_index))
        }
        MediaListRow::Item { .. } => {
            let state = *striped;
            *striped = !*striped;
            state
        }
    }
}

/// Whether one item row of a grouped list takes the zebra fill: the group's
/// members alternate from the fill at their first member, so every second
/// member reverts to the surface fill.
#[must_use]
pub fn zebra_group_member_striped(row_index: usize, group_start: usize) -> bool {
    (row_index - group_start).is_multiple_of(2)
}

/// One item row's group start in its list's source rows: the index of the
/// row below the group's heading — the list's first row when no heading sits
/// above the row. `rows` reaches the source rows up to and including
/// `row_index` (exclusive uses above are harmless: only a heading row marks
/// the start, and the wanted row is an item).
#[must_use]
pub fn zebra_group_start<'a, Target: 'a>(
    rows: &impl Fn(usize) -> &'a MediaListRow<Target>,
    row_index: usize,
) -> usize {
    (0..=row_index)
        .rev()
        .find(|&index| matches!(rows(index), MediaListRow::Heading { .. }))
        .map_or(0, |header| header + 1)
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

impl From<LibraryKey> for LibrarySelectionOrigin {
    fn from(key: LibraryKey) -> Self {
        match key {
            LibraryKey::Home => Self::Home,
            LibraryKey::Feeds => Self::Feeds,
            key @ LibraryKey::Service { .. } => Self::Service(key),
        }
    }
}

impl From<LibrarySelectionOrigin> for LibraryKey {
    fn from(origin: LibrarySelectionOrigin) -> Self {
        match origin {
            LibrarySelectionOrigin::Home => Self::Home,
            LibrarySelectionOrigin::Feeds => Self::Feeds,
            LibrarySelectionOrigin::Service(key) => key,
        }
    }
}
