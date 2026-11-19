//! The window's section facts that the section ledger builds (design D1 of
//! `gui-change-driven-sections`): the attach state the window shows before the
//! attach succeeds, the panel toggles, and the scrubber hover's conversions.
//!
//! The full-frame snapshot (`GuiFrame`, `GuiWindowFrame`, `GuiFrame::build`)
//! is gone: each section is built and sent on its own (`crate::sections`).

pub mod chrome;
pub mod library;
pub mod overlay;
pub mod playback;
pub mod queue;
pub mod settings;
pub mod theme;

use mbv::QueuePlaybackProjection;

use self::playback::GuiScrubberHover;

/// Why the window has no slice yet (design D9): one variant per Owner attach
/// outcome, with the text `startup::run_local_instance` gives the TUI Client.
/// The mismatch state's window paints Restart and Quit buttons and answers
/// key R and key Q; every other state answers nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// The scrubber's hover preview the playback section carries (design D4):
/// the Surface state's hover fraction resolved to the time it names against
/// the playback projection the tick reads. `None` while the pointer is away
/// from the bar.
pub(crate) fn scrubber_hover(
    fraction: Option<f64>,
    playback: &QueuePlaybackProjection,
) -> Option<GuiScrubberHover> {
    let fraction = fraction?;
    let ticks = scrubber_ticks(playback, fraction);
    Some(GuiScrubberHover {
        fraction,
        time_label: mbv_ui_model::ui_util::fmt_duration_short(
            ticks / mbv_emby_model::TICKS_PER_SECOND,
        ),
    })
}

/// The tick the pointer's `fraction` along the painted bar names, so the
/// hover's time label reads the same conversion the painted bar does.
fn scrubber_ticks(playback: &QueuePlaybackProjection, fraction: f64) -> i64 {
    mbv_emby_model::seconds_to_ticks(
        fraction * mbv_emby_model::ticks_to_seconds(playback.playback.runtime_ticks),
    )
}

/// The canvas panel's collapse and queue-fold facts (design D10), mirroring
/// `PanelTogglesProjection`, with the UI-reset counter the window resets its
/// interaction stores and panel widths on (docs/architecture/gui-client.md).
/// The counter rides this section so a reset moves the section's revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
