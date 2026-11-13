//! The window's section facts that the section ledger builds (design D1 of
//! `gui-change-driven-sections`): the attach state the window shows before the
//! attach succeeds, the panel toggles, and the scrubber hover's conversions.
//!
//! The full-frame snapshot (`GuiFrame`, `GuiWindowFrame`, `GuiFrame::build`)
//! is gone: each section is built and sent on its own (`crate::sections`).

pub mod chrome;
pub mod focus;
pub mod library;
pub mod overlay;
pub mod playback;
pub mod queue;
pub mod settings;
pub mod theme;

use mbv::{PlaybackChapter, QueuePlaybackProjection};
use serde::Serialize;

use self::playback::{GuiScrubberHover, GuiScrubberTile};

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

/// The scrubber's hover preview the playback section carries (design D4; task
/// 6.7 for the tile): the Surface state's hover fraction resolved to the time
/// and chapter it names against the playback projection the tick reads, plus
/// the tile the core thread resolved for the same position. `None` while the
/// pointer is away from the bar.
pub(crate) fn scrubber_hover(
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
/// `PanelTogglesProjection`, with the UI-reset counter the window resets its
/// interaction stores and panel widths on (docs/architecture/gui-client.md).
/// The counter rides this section so a reset moves the section's revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiPanelToggles {
    /// The `z` toggle: the panel collapsed to the rail.
    pub collapsed: bool,
    /// The `q` toggle: the expanded panel's queue folded to its one-line
    /// summary.
    pub queue_folded: bool,
    /// The UI-reset counter (design D10): it rises on each reset, so the front
    /// end resets its interaction stores when the value changes.
    pub ui_reset_epoch: u64,
}

impl GuiPanelToggles {
    /// The panel section's value: the projection's toggles and the App's
    /// UI-reset counter, read in the same tick.
    #[must_use]
    pub fn new(toggles: &mbv::PanelTogglesProjection, ui_reset_epoch: u64) -> Self {
        Self {
            collapsed: toggles.collapsed,
            queue_folded: toggles.queue_folded,
            ui_reset_epoch,
        }
    }
}
