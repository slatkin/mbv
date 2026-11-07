//! The Queue playback Surface state (extract-client-core design D5, row 4.5):
//! the transport focus and the seek state the Queue playback panel owns,
//! front-end-neutral. The TUI and GUI Clients hold this state and paint from
//! it; neither reimplements a rule.

use std::time::Instant;

use crate::seekbar::{scrub_fraction, seek_fraction};
use crate::{HitMap, HitRegion};
use mbv_ui_msg::Position;

/// The semantic target of one painted Queue playback region (design D6):
/// what a pointer press on the region resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuePlaybackHitTarget {
    /// The play/pause glyph: a press toggles playback.
    PlayPause,
    /// The stop glyph: a press stops playback.
    Stop,
    /// The previous glyph: a press steps to the previous item.
    Previous,
    /// The next glyph: a press steps to the next item.
    Next,
    /// The seekbar: a press seeks to the column's fraction of the painted
    /// bar span (the seek-target rule, [`QueuePlaybackSurface::seek_target`]).
    Seekbar,
    /// The playback-target indicator in the header row: a press opens the
    /// Sessions sidebar (the F3 shortcut's click target).
    PlaybackTarget,
    /// The visual slot's image rectangle: a press there anchors the slot's
    /// context menu.
    VisualSlot,
    /// The back control (add-tauri-gui-client row 6.4): a press does the
    /// `seek_back` chord's work. The GUI's canvas transport paints it; the
    /// TUI band has no such control and never pushes it.
    SeekBack,
    /// The forward control: a press does the `seek_forward` chord's work.
    /// GUI-painted only, as `SeekBack` is.
    SeekForward,
    /// The header's collapse control (the GUI canvas's `z` toggle). GUI-
    /// painted only: the TUI's panel has no collapse.
    Collapse,
    /// The header's queue-fold control (the GUI canvas's `q` toggle).
    /// GUI-painted only, as `Collapse` is.
    QueueFold,
    /// One row of the open playback-target chooser (add-tauri-gui-client
    /// row 6.4): the choice's index in the chooser's painted list.
    /// GUI-painted only: the TUI's counterpart is the Sessions sidebar,
    /// not a region of this band.
    TargetChoice(usize),
}

/// One frame's Queue playback interaction report (design D6): the painted
/// regions a pointer press resolves against. A Client derives it from its own
/// painted geometry and hands it to the Client core; the core reads it and
/// never computes geometry itself. The Queue playback panel has no shared
/// list, so the report carries no capacity.
#[derive(Debug, Clone, PartialEq)]
pub struct QueuePlaybackFrameReport {
    /// The frame's painted regions and their semantic targets. The transport
    /// glyphs resolve to their controls, the seekbar to the seek rule's
    /// target, the header's playback-target indicator to the Sessions
    /// shortcut, and the visual slot to its context-menu target.
    pub hit_map: HitMap<QueuePlaybackHitTarget>,
}

/// The Queue playback panel's Surface state: the transport band's focus
/// presentation and the seek target's throttle state.
///
/// The transport focus is the focus-derived presentation of the panel's
/// transport band: the Client projects the site's focus fact and the painter
/// resolves the band's fill through it. The queue column forces the band
/// unfocused today (the queue list owns the column's focus), so the projected
/// value is `false`; the seam exists so both Clients paint the same
/// focus-derived fill from the same state.
///
/// The seek state is the last admitted scrub's timestamp: the throttle that
/// admits at most one scrub seek per [`SEEKBAR_SCRUB_INTERVAL`]. It must be
/// shared, or the two Clients would admit scrubs on different cadences.
#[derive(Debug, Default)]
pub struct QueuePlaybackSurface {
    transport_focused: bool,
    last_scrub: Option<Instant>,
    /// The pointer's hover fraction over the seekbar (add-tauri-gui-client
    /// row 6.4): the scrubber preview's position, `None` while the pointer
    /// is away from the bar. A move over the painted bar sets it, a move
    /// away clears it.
    scrubber_hover: Option<f64>,
}

impl QueuePlaybackSurface {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Project the transport band's focus presentation.
    pub fn set_transport_focused(&mut self, focused: bool) {
        self.transport_focused = focused;
    }

    /// The transport band's focus presentation the painter resolves.
    #[must_use]
    pub const fn transport_focused(&self) -> bool {
        self.transport_focused
    }

    /// The seek target a pointer column resolves against the painted bar:
    /// its fraction of the bar span (the shared seek rule). `None` when the
    /// Client painted no bar.
    #[must_use]
    pub fn seek_target(&self, bar: HitRegion, column: f32) -> Option<f64> {
        seek_fraction(bar, column)
    }

    /// Record the pointer's hover fraction over the seekbar; `None` clears
    /// it (the pointer left the bar, or the painted regions changed under
    /// it).
    pub fn set_scrubber_hover(&mut self, hover: Option<f64>) {
        self.scrubber_hover = hover;
    }

    /// The pointer's hover fraction over the seekbar, `None` while the
    /// pointer is away from the bar.
    #[must_use]
    pub const fn scrubber_hover(&self) -> Option<f64> {
        self.scrubber_hover
    }

    /// Admit one scrub of the painted bar for a left-drag from `from` to
    /// `to`, throttled to one admitted scrub per [`SEEKBAR_SCRUB_INTERVAL`]
    /// of the caller's clock. Returns the seek fraction the drag's pointer
    /// end resolves to; `None` when the drag is not a scrub of this bar or
    /// the interval is still running.
    pub fn admit_scrub(
        &mut self,
        bar: HitRegion,
        from: Position,
        to: Position,
        now: Instant,
    ) -> Option<f64> {
        scrub_fraction(bar, &mut self.last_scrub, from, to, now)
    }
}
