//! The frame that the core thread emits to the front end after a tick that
//! changed state, and after each layout report (design D4). The front end keeps
//! only the last frame, so a frame carries everything the window paints.
//!
//! The frame reads the `mbv` library's read-only projections and the GUI's own
//! state: the focus stack and the Queue Surface state. The Queue section reads
//! the Surface state, not `App::queue_surface_projection`. That projection is a
//! push which the Surface state adopts once per tick, so a frame that called it
//! would consume the push the tick already delivered.

pub mod chrome;
pub mod focus;
pub mod library;
pub mod overlay;
pub mod playback;
pub mod queue;
pub mod theme;

use mbv::{App, PlaybackChapter, QueuePlaybackProjection};
use mbv_surfaces::FocusStack;
use mbv_surfaces::queue::QueueSurface;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use serde::Serialize;

use self::chrome::{GuiQueueFooter, GuiStatusBar};
use self::focus::GuiFocus;
use self::library::GuiLibraryFrame;
use self::overlay::GuiOverlayFrame;
use self::playback::{GuiCardArtwork, GuiQueuePlayback, GuiScrubberHover, GuiScrubberTile};
use self::queue::{GuiQueue, GuiQueueSummary};
use self::theme::GuiTheme;

/// One frame (design D4), sent to the window as the `mbv://frame` event. The
/// front end keeps only the last frame, so each frame carries everything the
/// window paints: the attached slice's frame, or the state a window whose
/// Owner attach has not finished shows (design D9).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiWindowFrame {
    /// The attached slice: the focus, Queue, playback, footer, and Status
    /// bar sections the window paints. The payload is boxed: the
    /// attach state's frame is far smaller, and a frame the window never
    /// sees while unattached should not size the whole enum for it.
    Slice(Box<GuiFrame>),
    /// The Owner attach's state (design D9): the window shows it while the
    /// core thread resolves the attach, and paints no slice.
    Attach(GuiAttachFrame),
}

/// The attach state's frame: the state the window shows, and the theme it
/// paints it in (design D7). The theme follows the slice frame's arming rule:
/// the first attach frame carries it, and the first frame after a
/// `report_layout` re-arms it — the front end sends the report when it
/// mounts, so a reload re-shows the state with its theme.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiAttachFrame {
    /// The attach state the window shows.
    pub state: GuiAttachState,
    /// The role-to-colour map (design D7). `None` unless the theme changed
    /// for this window.
    pub theme: Option<GuiTheme>,
}

/// Why the window has no slice yet (design D9): one variant per Owner attach
/// outcome, with the text `startup::run_local_instance` gives the TUI Client.
/// The mismatch state's window paints Restart and Quit buttons and answers
/// key R and key Q; every other state answers nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum GuiAttachState {
    /// The Owner holds the lock but does not accept connections
    /// (`OwnerState::Refuse`).
    Refusal(String),
    /// Stay-alive is off and another Client holds the Owner exclusively.
    ExclusiveRefusal(String),
    /// An Owner is shutting down; the window waits while the core thread
    /// polls for its exit, then starts an Owner and attaches.
    Waiting,
    /// A build mismatch. The text names the mismatch and the restart's cost;
    /// the window paints the Restart and Quit buttons.
    Mismatch(String),
    /// The attach failed with no recovery path; the text says why.
    Failed(String),
}

/// The attached slice's frame (design D4): the focus, Queue, playback,
/// footer, and Status bar sections, carried in
/// [`GuiWindowFrame::Slice`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiFrame {
    /// The focused surface and the Panel that holds focus.
    pub focus: GuiFocus,
    /// The Queue rows inside the viewport, with the cursor, marks, and active slot.
    pub queue: GuiQueue,
    /// The queue summary: the remaining count, the queue finish clock, the
    /// right column's mode, and the filter text with its caret fact.
    pub summary: GuiQueueSummary,
    /// The panel toggles the front end picks the layout with (design D10):
    /// the collapse and queue-fold states.
    pub panel: GuiPanelToggles,
    /// The open menu or popover (design D4). `None` while none is open.
    pub overlay: Option<GuiOverlayFrame>,
    /// The Queue playback projection: the transport band.
    pub playback: GuiQueuePlayback,
    /// The Queue footer's status pills.
    pub footer: GuiQueueFooter,
    /// The Status bar projection.
    pub status: GuiStatusBar,
    /// The Library panel's section (add-gui-library-panel design D2): the
    /// active destination's content under its revision. `None` while no
    /// Library panel shows.
    pub library: Option<GuiLibraryFrame>,
    /// The role-to-colour map (design D7). `None` unless the theme changed for
    /// this window.
    pub theme: Option<GuiTheme>,
}

/// What one frame reads: the `App`, the GUI's focus and Queue Surface state, and
/// the layout and theme facts that the core thread owns.
#[derive(Debug)]
pub struct FrameSources<'a> {
    /// The `App` whose read-only projections the frame carries.
    pub app: &'a App,
    /// The focus stack: the focused surface and the Panel focus.
    pub focus: &'a FocusStack,
    /// The Queue Surface state that adopts the Queue projection each tick.
    pub queue: &'a QueueSurface,
    /// The Queue playback Surface state: the scrubber hover the last
    /// pointer move resolved (design D4).
    pub playback: &'a QueuePlaybackSurface,
    /// The playback projection read once for this frame
    /// (`App::queue_playback_projection`): the playback section, the overlay,
    /// and the hover preview all resolve against this one read, so they
    /// cannot disagree inside one frame.
    pub playback_projection: &'a QueuePlaybackProjection,
    /// The hovered trickplay tile this frame carries (task 6.7): the tile the
    /// core thread's memo resolved and encoded, `None` while the pointer is
    /// away from the bar or the item's sheet is not cached.
    pub hover_tile: Option<GuiScrubberTile>,
    /// The whole Queue rows that the viewport fits (`ListRows::whole_rows`).
    pub viewport_rows: usize,
    /// Whether the window needs the theme in this frame: the first frame, and
    /// the first frame after the window reloads its content.
    pub theme_changed: bool,
    /// The visual slot's artwork this frame arms (design D10): the prepared
    /// value the core thread's memo built, `None` when the last frame already
    /// carried it.
    pub artwork: Option<GuiCardArtwork>,
    /// The Library panel's section (add-gui-library-panel design D2): the
    /// core thread's memo builds it, so a frame carries the content only when
    /// its revision changed. `None` while no Library panel shows.
    pub library: Option<GuiLibraryFrame>,
}

impl GuiFrame {
    /// Builds the frame from the core's current state.
    #[must_use]
    pub fn build(sources: &FrameSources<'_>) -> Self {
        let summary = sources.app.queue_summary_projection();
        // One playback projection read feeds both the playback section and
        // the overlay's popover choices, so the two cannot disagree inside
        // one frame.
        let playback = sources.playback_projection;
        Self {
            focus: GuiFocus::from(sources.focus),
            queue: GuiQueue::from_surface(sources.queue, sources.viewport_rows, &summary),
            summary: GuiQueueSummary::from(&summary),
            panel: GuiPanelToggles::from(&sources.app.panel_toggles_projection()),
            overlay: sources
                .app
                .gui_overlay_projection(playback)
                .as_ref()
                .map(GuiOverlayFrame::from),
            playback: GuiQueuePlayback {
                artwork: sources.artwork.clone(),
                hover: scrubber_hover(
                    sources.playback.scrubber_hover(),
                    playback,
                    sources.hover_tile.clone(),
                ),
                ..GuiQueuePlayback::from(playback)
            },
            footer: GuiQueueFooter::from(&sources.app.queue_footer_projection()),
            status: GuiStatusBar::from(&sources.app.status_bar_projection()),
            library: sources.library.clone(),
            theme: sources.theme_changed.then(GuiTheme::current),
        }
    }
}

/// The scrubber's hover preview one frame carries (design D4; task 6.7 for
/// the tile): the Surface state's hover fraction resolved to the time and
/// chapter it names against the playback projection the frame reads, plus
/// the tile the core thread resolved for the same position. `None` while the
/// pointer is away from the bar.
fn scrubber_hover(
    fraction: Option<f64>,
    playback: &QueuePlaybackProjection,
    tile: Option<GuiScrubberTile>,
) -> Option<GuiScrubberHover> {
    let fraction = fraction?;
    let ticks = scrubber_ticks(playback, fraction);
    let chapter = covering_chapter(playback, ticks);
    Some(GuiScrubberHover {
        fraction,
        time_label: mbv_ui_model::ui_util::fmt_duration_short(
            ticks / mbv_emby_model::TICKS_PER_SECOND,
        ),
        chapter: chapter.map(|(_, chapter)| chapter.name.clone()),
        chapter_number: chapter.map(|(index, _)| index + 1),
        tile,
    })
}

/// The tick the pointer's `fraction` along the painted bar names. The core
/// thread resolves the same tick to pick the preview's trickplay tile, so
/// both read this one conversion.
#[must_use]
pub fn scrubber_ticks(playback: &QueuePlaybackProjection, fraction: f64) -> i64 {
    mbv_emby_model::seconds_to_ticks(
        fraction * mbv_emby_model::ticks_to_seconds(playback.playback.runtime_ticks),
    )
}

/// The chapter that covers `ticks`, with its 1-based ordinal: the last
/// boundary at or before it, the covering rule the TUI's chapter lookups
/// share. The ordinal is the canvas preview's `Ch. 2` form.
fn covering_chapter(
    playback: &QueuePlaybackProjection,
    ticks: i64,
) -> Option<(usize, &PlaybackChapter)> {
    playback
        .chapters
        .iter()
        .enumerate()
        .rev()
        .find(|(_, chapter)| chapter.start_ticks <= ticks)
}

/// The canvas panel's collapse and queue-fold facts (design D10), mirroring
/// `PanelTogglesProjection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPanelToggles {
    /// The `z` toggle: the panel collapsed to the rail.
    pub collapsed: bool,
    /// The `q` toggle: the expanded panel's queue folded to its one-line
    /// summary.
    pub queue_folded: bool,
}

impl From<&mbv::PanelTogglesProjection> for GuiPanelToggles {
    fn from(toggles: &mbv::PanelTogglesProjection) -> Self {
        Self {
            collapsed: toggles.collapsed,
            queue_folded: toggles.queue_folded,
        }
    }
}
