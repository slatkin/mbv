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
pub mod playback;
pub mod queue;
pub mod theme;

use mbv::App;
use mbv_surfaces::FocusStack;
use mbv_surfaces::breakpoint::BreakpointClass;
use mbv_surfaces::queue::QueueSurface;
use serde::Serialize;

use self::chrome::{GuiQueueFooter, GuiStatusBar};
use self::focus::GuiFocus;
use self::playback::GuiQueuePlayback;
use self::queue::GuiQueue;
use self::theme::GuiTheme;

/// One frame (design D4), sent to the window as the `mbv://frame` event.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiFrame {
    /// The focused surface and the Panel that holds focus.
    pub focus: GuiFocus,
    /// The Queue rows inside the viewport, with the cursor, marks, and active slot.
    pub queue: GuiQueue,
    /// The Queue playback projection: the transport band.
    pub playback: GuiQueuePlayback,
    /// The Queue footer's status pills.
    pub footer: GuiQueueFooter,
    /// The Status bar projection.
    pub status: GuiStatusBar,
    /// The breakpoint class of the window's width.
    pub breakpoint: GuiBreakpoint,
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
    /// The whole Queue rows that the viewport fits (`ListRows::whole_rows`).
    pub viewport_rows: usize,
    /// The breakpoint class of the window's width (design D10).
    pub breakpoint: BreakpointClass,
    /// Whether the window needs the theme in this frame: the first frame, and
    /// the first frame after the window reloads its content.
    pub theme_changed: bool,
}

impl GuiFrame {
    /// Builds the frame from the core's current state.
    #[must_use]
    pub fn build(sources: &FrameSources<'_>) -> Self {
        Self {
            focus: GuiFocus::from(sources.focus),
            queue: GuiQueue::from_surface(sources.queue, sources.viewport_rows),
            playback: GuiQueuePlayback::from(&sources.app.queue_playback_projection()),
            footer: GuiQueueFooter::from(&sources.app.queue_footer_projection()),
            status: GuiStatusBar::from(&sources.app.status_bar_projection()),
            breakpoint: GuiBreakpoint::from(sources.breakpoint),
            theme: sources.theme_changed.then(GuiTheme::current),
        }
    }
}

/// The layout breakpoint class of the window (design D10), mirroring
/// `BreakpointClass` variant for variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiBreakpoint {
    /// Only one of Library or Queue shows.
    Mini,
    /// Both panels show. The hero opens as an overlay on demand.
    Narrow,
    /// Both panels show. The hero may sit beside its browser.
    Wide,
}

impl From<BreakpointClass> for GuiBreakpoint {
    fn from(class: BreakpointClass) -> Self {
        match class {
            BreakpointClass::Mini => Self::Mini,
            BreakpointClass::Narrow => Self::Narrow,
            BreakpointClass::Wide => Self::Wide,
        }
    }
}
